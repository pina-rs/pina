#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODAMA_DIR="$ROOT/codama"
IDL_DIR="$CODAMA_DIR/idls"
RUST_CLIENTS_DIR="$CODAMA_DIR/clients/rust"
JS_CLIENTS_DIR="$CODAMA_DIR/clients/js"

if ! command -v git >/dev/null 2>&1; then
	echo "git is required to verify deterministic Codama output." >&2
	exit 1
fi

if ! command -v dprint >/dev/null 2>&1; then
	echo "dprint is required to perform format-neutral Codama drift checks." >&2
	exit 1
fi

show_codama_diff() {
	echo >&2
	echo "Codama output status:" >&2
	git -C "$ROOT" --no-pager status --short -- "$IDL_DIR" "$RUST_CLIENTS_DIR" "$JS_CLIENTS_DIR" >&2 || true
	echo >&2
	echo "Codama output diff stat:" >&2
	git -C "$ROOT" --no-pager diff --stat -- "$IDL_DIR" "$RUST_CLIENTS_DIR" "$JS_CLIENTS_DIR" >&2 || true
	echo >&2
	echo "Codama output diff:" >&2
	git -C "$ROOT" --no-pager diff -- "$IDL_DIR" "$RUST_CLIENTS_DIR" "$JS_CLIENTS_DIR" >&2 || true
}

format_codama_outputs() {
	(
		cd "$ROOT"

		FORMAT_FILES=()
		while IFS= read -r file; do
			FORMAT_FILES+=("$file")
		done < <(find codama/idls codama/clients/rust codama/clients/js -type f | sort)

		if [ "${#FORMAT_FILES[@]}" -eq 0 ]; then
			echo "No Codama output files found to format." >&2
			exit 1
		fi

		dprint fmt --config "$ROOT/dprint.json" "${FORMAT_FILES[@]}"
	)
}

trap '
	status=$?
	if [ "$status" -ne 0 ]; then
		echo "verify-codama-idls.sh failed with exit code $status" >&2
		show_codama_diff
	fi
' EXIT

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

format_codama_outputs

echo "Verifying Rust IDL fixture drift tests..."
cargo test -p pina_cli --locked --test codama_idls

echo "Running Codama JS IDL validation..."
pnpm --dir "$ROOT" run test:idls

NODES_FROM_PINA_DIR="$ROOT/packages/nodes-from-pina"

echo "Type-checking nodes-from-pina..."
(
	cd "$NODES_FROM_PINA_DIR"
	node "$NODES_FROM_PINA_DIR/node_modules/typescript/bin/tsc" --noEmit
)

echo "Running nodes-from-pina unit tests..."
(
	cd "$NODES_FROM_PINA_DIR"
	node "$NODES_FROM_PINA_DIR/node_modules/vitest/vitest.mjs" run
)

echo "Type-checking generated JS clients..."
node "$ROOT/node_modules/typescript/bin/tsc" --noEmit -p "$ROOT/codama/tsconfig.json"

echo "Compile-checking generated Rust client crates..."
CLIENT_MANIFESTS=()
while IFS= read -r manifest; do
	CLIENT_MANIFESTS+=("$manifest")
done < <(find "$RUST_CLIENTS_DIR" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)

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
GENERATED_FILES=()
while IFS= read -r generated_file; do
	GENERATED_FILES+=("$generated_file")
done < <(
	git -C "$ROOT" diff --name-only HEAD -- "$IDL_DIR" "$RUST_CLIENTS_DIR" "$JS_CLIENTS_DIR"
)

if [ "${#GENERATED_FILES[@]}" -gt 0 ]; then
	echo "Detected Codama output drift after regeneration and formatting. Output must be committed and deterministic." >&2
	show_codama_diff
	exit 1
fi

echo "Codama generation and validation checks passed."
