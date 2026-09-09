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
