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
SHARED_TARGET="$WORK_DIR/target"
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

# Instructions each fixture is expected to skip. `--skip-unsupported-instructions`
# keeps the gate compiling a crate whose IDL uses a strategy this renderer cannot
# express, but a new skip would mean a previously-rendered instruction silently
# disappeared, so the count is pinned here and compared below.
declare -A EXPECTED_SKIPS=(
	[metaplex_token_metadata]=22
	[meteora_dlmm]=0
	[squads_v4_multisig]=0
	[switchboard_on_demand]=0
)

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
	# not something to paper over with a different check. Instructions whose
	# account lists the fixed-size handle set cannot express (Anchor's
	# `omitted` optional-account strategy) are skipped, and the skip reasons
	# are recorded in the generated `instructions/mod.rs`.
	cargo run --locked --quiet -p pina_cpi_renderer --bin pina_cpi_renderer -- \
		--idl "$fixture" \
		--output "$crate_dir" \
		--skip-unsupported-instructions

	if [ ! -f "$crate_dir/src/generated/mod.rs" ]; then
		echo "$name: renderer produced no crate at $crate_dir" >&2
		exit 1
	fi

	skipped_marker="$crate_dir/src/generated/instructions/mod.rs"
	observed_skips=0
	if [ -f "$skipped_marker" ]; then
		if grep -q "^// Skipped" "$skipped_marker"; then
			echo "$name: skipped instructions:"
			grep "^// Skipped" "$skipped_marker" | sed 's/^/  /'
		fi
		observed_skips="$(grep -c "^// Skipped" "$skipped_marker" || true)"
	fi
	expected_skips="${EXPECTED_SKIPS[$name]:-missing}"
	if [ "$expected_skips" = "missing" ]; then
		echo "$name: no expected-skip baseline; add one to EXPECTED_SKIPS" >&2
		exit 1
	fi
	if [ "$observed_skips" != "$expected_skips" ]; then
		echo "$name: skipped $observed_skips instruction(s), expected $expected_skips" >&2
		exit 1
	fi
	echo "$name: $observed_skips skipped instruction(s), matching the baseline"

	mkdir -p "$crate_dir/.cargo"
	cat >"$crate_dir/.cargo/config.toml" <<EOF
[target.$TARGET]
rustflags = ["-C", "linker=sbpf-linker", "-C", "panic=abort"]
EOF

	# Keep the renderer's own manifest so a regression in it still fails this
	# gate; only the `pina` registry dependency is redirected to the in-repo
	# crate, and the manifest is isolated from any parent workspace.
	generated_manifest="$crate_dir/Cargo.toml"
	if [ ! -f "$generated_manifest" ]; then
		echo "$name: renderer produced no Cargo.toml at $generated_manifest" >&2
		exit 1
	fi
	if ! grep -q '^pina = ' "$generated_manifest"; then
		echo "$name: generated manifest declares no pina dependency" >&2
		exit 1
	fi
	node "$ROOT/scripts/point-pina-dependency.ts" "$generated_manifest" "$ROOT/crates/pina"

	(
		cd "$crate_dir"
		CARGO_TARGET_DIR="$SHARED_TARGET" \
			cargo "+$TOOLCHAIN" check \
			--quiet \
			--target "$TARGET" \
			-Z build-std=core,alloc
	)

	# The generated crate ships a test binding its compiled-in program ID to the
	# address from the IDL. Run it on the host, where \`cargo test\` works.
	CARGO_TARGET_DIR="$SHARED_TARGET" \
		cargo test --quiet --manifest-path "$crate_dir/Cargo.toml"
	echo "$name: SBF check passed"
done

echo
echo "All ${#FIXTURES[@]} foreign-IDL fixture(s) compile for $TARGET."
