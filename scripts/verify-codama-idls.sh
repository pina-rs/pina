#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IDL_DIR="$ROOT/codama/idls"
RUST_CLIENTS_DIR="$ROOT/codama/clients/rust"
JS_CLIENTS_DIR="$ROOT/codama/clients/js"

if ! command -v git >/dev/null 2>&1; then
	echo "git is required to verify deterministic Codama output." >&2
	exit 1
fi

echo "Installing pnpm workspace dependencies..."
pnpm install --frozen-lockfile

echo "Generating Codama IDLs and clients for all examples..."
cargo run -p pina_cli --quiet -- codama generate \
	--examples-dir "$ROOT/examples" \
	--idls-dir "$IDL_DIR" \
	--rust-out "$RUST_CLIENTS_DIR" \
	--js-out "$JS_CLIENTS_DIR" \
	--npx node

if ! find "$IDL_DIR" -mindepth 1 -maxdepth 1 -type f -name "*.json" | grep -q .; then
	echo "No *.json fixtures were generated in $IDL_DIR" >&2
	exit 1
fi

echo "Verifying Rust IDL fixture drift tests..."
cargo test -p pina_cli --locked --test codama_idls

echo "Running Codama JS IDL validation..."
pnpm --dir "$ROOT" run test:idls
pnpm --dir "$ROOT" run test:nodes-from-pina

echo "Type-checking generated JS clients..."
pnpm --dir "$ROOT" run check:js

echo "Compile-checking generated Rust client crates..."
mapfile -t CLIENT_MANIFESTS < <(
	find "$RUST_CLIENTS_DIR" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort
)

if [ "${#CLIENT_MANIFESTS[@]}" -eq 0 ]; then
	echo "No generated Rust client manifests found in $RUST_CLIENTS_DIR" >&2
	exit 1
fi

CLIENT_ARGS=()
for manifest in "${CLIENT_MANIFESTS[@]}"; do
	package_name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1)"
	if [ -z "$package_name" ]; then
		echo "Failed to read package name from $manifest" >&2
		exit 1
	fi
	CLIENT_ARGS+=("-p" "$package_name")
done

cargo check --locked "${CLIENT_ARGS[@]}"

echo "Checking deterministic Codama output regeneration..."
diff_status="$(
	git -C "$ROOT" status --porcelain -- "$IDL_DIR" "$RUST_CLIENTS_DIR" "$JS_CLIENTS_DIR"
)"
if [ -n "$diff_status" ]; then
	echo "Detected Codama output diff after regeneration. Output must be committed and deterministic." >&2
	git -C "$ROOT" --no-pager status --short -- "$IDL_DIR" "$RUST_CLIENTS_DIR" "$JS_CLIENTS_DIR" >&2
	exit 1
fi

echo "Codama generation and validation checks passed."
