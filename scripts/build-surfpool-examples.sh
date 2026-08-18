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

mkdir -p "$OUT_DIR"

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
	cargo-build-sbf \
		--skip-tools-install \
		--tools-version "$TOOLS_VERSION" \
		--manifest-path "$manifest" \
		--features bpf-entrypoint \
		--sbf-out-dir "$OUT_DIR"

	if [[ ! -f "$direct_artifact" && ! -f "$library_artifact" ]]; then
		echo "cargo-build-sbf did not produce an artifact for ${example_name}" >&2
		exit 1
	fi
done < <(find "$ROOT/examples" -mindepth 2 -maxdepth 2 -name Cargo.toml -print | sort)
