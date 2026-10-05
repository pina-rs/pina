#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${SBF_OUT_DIR:-$ROOT/target/surfpool/examples}"
TOOLS_VERSION="${SBF_TOOLS_VERSION:-v1.54}"

require_bin() {
	local name="$1"
	if ! command -v "$name" >/dev/null 2>&1; then
		echo "missing required binary on PATH: $name" >&2
		exit 1
	fi
}

require_bin cargo-build-sbf

cargo_build_sbf="$(command -v cargo-build-sbf)"
if [[ "$(uname -s)" == "Linux" ]]; then
	cargo_build_sbf_resolved="$(readlink -f "$cargo_build_sbf")"
	cargo_build_sbf_real="$(dirname "$cargo_build_sbf_resolved")/.cargo-build-sbf-wrapped"
	if [[ -x "$cargo_build_sbf_real" ]]; then
		cargo_build_sbf="$cargo_build_sbf_real"
	fi

	for platform_tools_link in \
		"${HOME:?}/.cache/solana/$TOOLS_VERSION/platform-tools" \
		"${XDG_CACHE_HOME:-$HOME/.cache}/solana/$TOOLS_VERSION/platform-tools"; do
		[[ -L "$platform_tools_link" ]] || continue
		platform_tools_target="$(readlink "$platform_tools_link")"
		case "$platform_tools_target" in
		/nix/store/*/lib/platform-tools) unlink "$platform_tools_link" ;;
		*)
			echo "refusing to replace unexpected platform-tools link: $platform_tools_target" >&2
			exit 1
			;;
		esac
	done
fi

mkdir -p "$OUT_DIR"
tools_install=--force-tools-install

# On macOS the nix agave package that provides cargo-build-sbf re-seeds the
# platform-tools cache symlink from its read-only store bundle before every
# invocation, so a forced install can only download the full asset over that
# bundle — and a truncated download fails the whole run. When the resolved
# binary is that bundled wrapper, trust the pinned bundle from the first
# build on, after confirming its platform-tools version is the one this
# script pins. Linux keeps the forced first install: its bundled sysroot is
# incomplete (no liballoc), so the full pinned toolchain must be fetched.
if [[ "$(uname -s)" != "Linux" ]]; then
	case "$cargo_build_sbf" in
	/nix/store/*/bin/cargo-build-sbf)
		bundled_platform_tools="${cargo_build_sbf%/bin/cargo-build-sbf}/lib/platform-tools"
		bundled_version="$("${cargo_build_sbf%/bin/cargo-build-sbf}/bin/.cargo-build-sbf-wrapped" --version 2>/dev/null |
			sed -n 's/^platform-tools //p')"
		if [[ -d "$bundled_platform_tools" && "$bundled_version" == "$TOOLS_VERSION" ]]; then
			tools_install=--skip-tools-install
		fi
		;;
	esac
fi

# Build each example independently. This deliberately does not use a best-effort
# loop: a missing or non-SBF example is a test failure, not a skipped test.
while IFS= read -r manifest; do
	example_dir="$(dirname "$manifest")"
	example_name="$(basename "$example_dir")"
	direct_artifact="$OUT_DIR/${example_name}.so"
	library_artifact="$OUT_DIR/lib${example_name}.so"
	if [[ -z "$example_name" || "$direct_artifact" != "$OUT_DIR/"*.so || "$library_artifact" != "$OUT_DIR/"*.so ]]; then
		echo "refusing unsafe artifact paths for ${example_name}" >&2
		exit 1
	fi

	echo "Building ${example_name} for Surfpool"
	rm -f -- "$direct_artifact" "$library_artifact"
	features=(bpf-entrypoint)
	if [[ "$example_name" == "pina_bpf_program" ]]; then
		features+=(cpi-runtime-tests)
	fi
	"$cargo_build_sbf" \
		"$tools_install" \
		--tools-version "$TOOLS_VERSION" \
		--manifest-path "$manifest" \
		--features "$(
			IFS=,
			echo "${features[*]}"
		)" \
		--sbf-out-dir "$OUT_DIR"
	tools_install=--skip-tools-install

	if [[ ! -f "$direct_artifact" && ! -f "$library_artifact" ]]; then
		echo "cargo-build-sbf did not produce an artifact for ${example_name}" >&2
		exit 1
	fi
done < <(find "$ROOT/examples" -mindepth 2 -maxdepth 2 -name Cargo.toml -print | sort)

# `pina rehearse`'s end-to-end test (crates/pina_cli/tests/rehearse_surfpool.rs)
# needs two standalone copies of the counter example, built outside the
# workspace so they never join the example inventory:
#
# - a variant whose increment adds two, which proves a candidate's
#   account-state differences are detected and decoded. Its artifact keeps the
#   counter's name, so it lands in its own directory.
# - a counter declared at the address of the ed25519 seed [11; 32], whose
#   program keypair the test can therefore write, so `pina deploy --rehearse`
#   can plan an upgrade of it. Its source stays in the output directory as the
#   project the deployment plans from.
counter_dir="$ROOT/examples/counter_program"
variant_source="$(mktemp -d)"
trap 'rm -rf "$variant_source"' EXIT

# Copy the counter to the package directory $1 as package $2, applying the
# sed script $3 to its source and migration manifest.
standalone_counter() {
	local package_dir="$1" package="$2" edit="$3"
	rm -rf -- "$package_dir"
	mkdir -p "$package_dir/src" "$package_dir/migrations"
	cp "$counter_dir/build.rs" "$package_dir/"
	cp "$counter_dir/migrations/publications.json" "$package_dir/migrations/"
	sed "$edit" "$counter_dir/src/lib.rs" >"$package_dir/src/lib.rs"
	sed "$edit" "$counter_dir/migrations/manifest.json" >"$package_dir/migrations/manifest.json"
	cat >"$package_dir/Cargo.toml" <<MANIFEST
[package]
name = "$package"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[features]
bpf-entrypoint = []

[dependencies]
pina = { path = "$ROOT/crates/pina", features = ["account-resize", "logs", "derive"] }

[workspace]
MANIFEST
	cp "$ROOT/Cargo.lock" "$package_dir/Cargo.lock"
}

# Build the standalone package in $1 into the output directory $2. The copies
# have distinct package names, so they share one target directory.
build_standalone() {
	local package_dir="$1" out="$2"
	rm -rf -- "$out"
	CARGO_TARGET_DIR="$ROOT/target/surfpool/rehearse-variant-target" "$cargo_build_sbf" \
		"$tools_install" \
		--tools-version "$TOOLS_VERSION" \
		--manifest-path "$package_dir/Cargo.toml" \
		--features bpf-entrypoint \
		--sbf-out-dir "$out"
}

standalone_counter "$variant_source" counter_program 's/\.checked_add(1)/.checked_add(2)/'
if ! grep -q '\.checked_add(2)' "$variant_source/src/lib.rs"; then
	echo "the counter rehearsal variant no longer patches increment; update this script" >&2
	exit 1
fi
variant_out="$OUT_DIR/rehearse-variant"
echo "Building the counter_program rehearsal variant"
build_standalone "$variant_source" "$variant_out"
if [[ ! -f "$variant_out/counter_program.so" ]]; then
	echo "cargo-build-sbf did not produce the counter rehearsal variant" >&2
	exit 1
fi

counter_id="GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS"
deploy_id="7v54NWdBtkjuAFJrLGsS2SXnuk8nKam81mZJeeYxVFi9"
deploy_out="$OUT_DIR/rehearse-deploy"
deploy_project="$deploy_out/project"
mkdir -p "$deploy_out"
standalone_counter "$deploy_project" counter_deploy_program "s/$counter_id/$deploy_id/g"
for redeclared in "$deploy_project/src/lib.rs" "$deploy_project/migrations/manifest.json"; do
	if ! grep -q "$deploy_id" "$redeclared" || grep -q "$counter_id" "$redeclared"; then
		echo "the counter deploy copy no longer redeclares its program ID; update this script" >&2
		exit 1
	fi
done
echo "Building the counter_program deploy rehearsal copy"
build_standalone "$deploy_project" "$deploy_out/program"
if [[ ! -f "$deploy_out/program/counter_deploy_program.so" ]]; then
	echo "cargo-build-sbf did not produce the counter deploy rehearsal copy" >&2
	exit 1
fi
