#!/usr/bin/env bash
# Verifies that every checked-in foreign-IDL fixture renders into a CPI crate
# that compiles for the SBF target.
#
# This is the CI counterpart to the in-crate fixture tests: those prove the
# renderer produces the expected discriminators, accounts, and encoded lengths,
# while this script proves the result is a real `no_std` Solana crate. A
# renderer change that emits valid-looking Rust which still fails to build for
# SBF is exactly what this gate exists to catch.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$ROOT/crates/pina_cpi_renderer/fixtures"
WORK_DIR="${PINA_CPI_SBF_WORK_DIR:-$(mktemp -d)}"
TARGET="${PINA_CPI_SBF_TARGET:-bpfel-unknown-none}"
# The BPF/SBF Rust toolchain pin lives in `devenv.nix` so this gate uses the
# same compiler as the compute-units workflow and the example program builds.
TOOLCHAIN="${PINA_BPF_TOOLCHAIN:-nightly-2025-11-20}"

cleanup() {
	if [ -z "${PINA_CPI_SBF_WORK_DIR:-}" ]; then
		rm -rf "$WORK_DIR"
	fi
}
trap cleanup EXIT

if [ ! -d "$FIXTURE_DIR" ]; then
	echo "no foreign-IDL fixtures at $FIXTURE_DIR" >&2
	exit 1
fi

# `-Z build-std` needs the toolchain's `rust-src` component.
if ! rustup component list --toolchain "$TOOLCHAIN" 2>/dev/null | grep -q '^rust-src (installed)'; then
	rustup component add rust-src --toolchain "$TOOLCHAIN"
fi

mapfile -t FIXTURES < <(find "$FIXTURE_DIR" -maxdepth 1 -type f -name '*.json' | sort)

if [ "${#FIXTURES[@]}" -eq 0 ]; then
	echo "no *.json fixtures found in $FIXTURE_DIR" >&2
	exit 1
fi

echo "Rendering ${#FIXTURES[@]} foreign-IDL fixture(s) into $WORK_DIR"

# Build the renderer once so each fixture only pays the rendering cost.
cargo build --locked --quiet -p pina_cpi_renderer

for fixture in "${FIXTURES[@]}"; do
	name="$(basename "$fixture" .json)"
	crate_dir="$WORK_DIR/$name"
	echo
	echo "=== $name ==="

	# Fail loudly: a renderer that cannot produce the crate is a gate failure,
	# not something to paper over with a different check.
	cargo run --locked --quiet -p pina_cpi_renderer --bin pina_cpi_renderer -- \
		--idl "$fixture" \
		--output "$crate_dir"

	if [ ! -f "$crate_dir/src/generated/mod.rs" ]; then
		echo "$name: renderer produced no crate at $crate_dir" >&2
		exit 1
	fi

	# The generated crate is standalone; point it at the in-repo `pina` so the
	# SBF check exercises the same types a consumer would build against.
	cat >"$crate_dir/Cargo.toml" <<EOF
[package]
name = "${name}_cpi"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
pina = { path = "$ROOT/crates/pina", default-features = false }
EOF

	mkdir -p "$crate_dir/.cargo"
	cat >"$crate_dir/.cargo/config.toml" <<EOF
[target.$TARGET]
rustflags = ["-C", "linker=sbpf-linker", "-C", "panic=abort"]
EOF

	(
		cd "$crate_dir"
		cargo "+$TOOLCHAIN" check \
			--quiet \
			--target "$TARGET" \
			-Z build-std=core,alloc
	)
	echo "$name: SBF check passed"
done

echo
echo "All ${#FIXTURES[@]} foreign-IDL fixture(s) compile for $TARGET."
