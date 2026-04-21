#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
	echo "Usage: $0 <workspace-root> <output-dir> [policy-file]" >&2
	exit 1
fi

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
WORKSPACE_ROOT=$(cd "$1" && pwd)
OUTPUT_DIR=$2
POLICY_FILE=${3:-"$SCRIPT_DIR/compute-unit-policy.json"}
BPF_TOOLCHAIN=${PINA_BPF_TOOLCHAIN:-nightly-2025-11-20}

if [ ! -f "$POLICY_FILE" ]; then
	echo "Missing compute-unit policy file: $POLICY_FILE" >&2
	exit 1
fi

if ! command -v install:sbpf-gallery >/dev/null 2>&1; then
	echo "install:sbpf-gallery must be available in PATH. Run this from devenv shell." >&2
	exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
	echo "python3 is required to read $POLICY_FILE" >&2
	exit 1
fi

mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR=$(cd "$OUTPUT_DIR" && pwd)

mapfile -t TRACKED_PROGRAMS < <(
	python3 - "$POLICY_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    policy = json.load(handle)

for program in policy["trackedPrograms"]:
    print(program)
PY
)

mapfile -t BUILD_GROUPS < <(
	python3 - "$WORKSPACE_ROOT" "$POLICY_FILE" <<'PY'
import json
import sys
import tomllib
from pathlib import Path

workspace_root = Path(sys.argv[1])
policy_path = Path(sys.argv[2])

with policy_path.open(encoding="utf-8") as handle:
    policy = json.load(handle)

groups: dict[str, list[str]] = {}

for program in policy["trackedPrograms"]:
    cargo_toml = workspace_root / "examples" / program / "Cargo.toml"
    group_key = program

    if cargo_toml.exists():
        with cargo_toml.open("rb") as handle:
            manifest = tomllib.load(handle)

        pina_dependency = manifest.get("dependencies", {}).get("pina")
        if isinstance(pina_dependency, dict):
            features = sorted(pina_dependency.get("features", []))
            group_key = ",".join(features)

    groups.setdefault(group_key, []).append(program)

for programs in groups.values():
    print(" ".join(programs))
PY
)

resolve_artifact_path() {
	local program=$1
	local candidates=(
		"$WORKSPACE_ROOT/target/deploy/$program.so"
		"$WORKSPACE_ROOT/target/sbpf-solana-solana/release/$program.so"
		"$WORKSPACE_ROOT/target/bpfel-unknown-none/release/$program.so"
	)

	local candidate
	for candidate in "${candidates[@]}"; do
		if [ -f "$candidate" ]; then
			printf '%s\n' "$candidate"
			return 0
		fi
	done

	return 1
}

install:sbpf-gallery

if ! rustup toolchain list | grep -q "^$BPF_TOOLCHAIN"; then
	rustup toolchain install "$BPF_TOOLCHAIN" --profile minimal --component rust-src
else
	rustup component add rust-src --toolchain "$BPF_TOOLCHAIN"
fi

for build_group in "${BUILD_GROUPS[@]}"; do
	read -r -a group_programs <<<"$build_group"
	build_args=()

	for program in "${group_programs[@]}"; do
		rm -f "$OUTPUT_DIR/$program.json"
		build_args+=(-p "$program")
	done

	echo "Building ${group_programs[*]} with cargo +$BPF_TOOLCHAIN build-bpf ${build_args[*]}"

	set +e
	(
		cd "$WORKSPACE_ROOT"
		cargo +"$BPF_TOOLCHAIN" build-bpf "${build_args[@]}"
	)
	build_status=$?
	set -e

	if [ "$build_status" -ne 0 ]; then
		for program in "${group_programs[@]}"; do
			echo "warning: failed to build tracked program $program for static CU profiling" >&2
		done
		continue
	fi

	for program in "${group_programs[@]}"; do
		artifact=$(resolve_artifact_path "$program") || {
			echo "warning: failed to find built SBF artifact for $program under $WORKSPACE_ROOT/target" >&2
			continue
		}

		echo "Profiling $program from $artifact"

		set +e
		(
			cd "$WORKSPACE_ROOT"
			cargo run --quiet --locked -p pina_cli -- profile "$artifact" --json --output "$OUTPUT_DIR/$program.json"
		)
		profile_status=$?
		set -e

		if [ "$profile_status" -ne 0 ]; then
			echo "warning: failed to profile tracked program $program from $artifact" >&2
			rm -f "$OUTPUT_DIR/$program.json"
			continue
		fi
	done
done

python3 - "$OUTPUT_DIR/manifest.json" "$POLICY_FILE" "$BPF_TOOLCHAIN" <<'PY'
import json
import sys
from pathlib import Path

manifest_path = Path(sys.argv[1])
policy_path = Path(sys.argv[2])
toolchain = sys.argv[3]

with policy_path.open(encoding="utf-8") as handle:
    policy = json.load(handle)

manifest = {
    "toolchain": toolchain,
    "trackedPrograms": policy["trackedPrograms"],
}

manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
