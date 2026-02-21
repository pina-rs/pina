#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Step 1: Build pina CLI ==="
cargo build -p pina_cli --release --manifest-path "$ROOT_DIR/Cargo.toml"

echo ""
echo "=== Step 2: Generate IDL files ==="
mkdir -p "$SCRIPT_DIR/idls"
for program_dir in "$ROOT_DIR"/examples/*/; do
	program_name=$(basename "$program_dir")
	echo "  Generating IDL for $program_name..."
	"$ROOT_DIR/target/release/pina" idl -p "$program_dir" -o "$SCRIPT_DIR/idls/$program_name.json"
done

echo ""
echo "=== Step 3: Install codama dependencies ==="
(cd "$SCRIPT_DIR" && pnpm install --frozen-lockfile)

echo ""
echo "=== Step 4: Generate Rust and JS clients via codama ==="
(cd "$SCRIPT_DIR" && node generate.mjs)

echo ""
echo "=== Step 5: Check Rust clients compile ==="
cargo check --manifest-path "$SCRIPT_DIR/clients/rust/Cargo.toml"

echo ""
echo "=== Step 6: Check JS clients type-check ==="
(cd "$SCRIPT_DIR" && npx tsc --noEmit)

echo ""
echo "=== All checks passed ==="
