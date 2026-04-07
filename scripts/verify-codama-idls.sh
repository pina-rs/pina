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

compare_normalized_file() {
	local relative_path="$1"
	local absolute_path="$ROOT/$relative_path"
	local diff_file

	if [ ! -f "$absolute_path" ]; then
		echo "Missing regenerated file: $relative_path" >&2
		return 1
	fi

	if ! git -C "$ROOT" cat-file -e "HEAD:$relative_path" 2>/dev/null; then
		echo "Committed file not found for comparison: $relative_path" >&2
		return 1
	fi

	diff_file="$(mktemp)"
	if ! diff -u \
		<(git -C "$ROOT" show "HEAD:$relative_path" | dprint fmt --config "$ROOT/dprint.json" --stdin "$relative_path") \
		<(dprint fmt --config "$ROOT/dprint.json" --stdin "$relative_path" <"$absolute_path") \
		>"$diff_file"; then
		echo "Detected non-formatting drift in: $relative_path" >&2
		cat "$diff_file" >&2
		rm -f "$diff_file"
		return 1
	fi

	rm -f "$diff_file"
	return 0
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

if find "$IDL_DIR" -mindepth 1 -maxdepth 1 -type f -name "*.json" | grep -q .; then
	dprint fmt --config "$ROOT/dprint.json" "$IDL_DIR/*.json"
else
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
	has_real_drift=0
	for generated_file in "${GENERATED_FILES[@]}"; do
		if ! compare_normalized_file "$generated_file"; then
			has_real_drift=1
			echo "  [drift] $generated_file" >&2
		fi
	done

	if [ "$has_real_drift" -ne 0 ]; then
		echo "Detected non-formatting Codama output drift after regeneration. Output must be committed and deterministic." >&2
		show_codama_diff
		exit 1
	fi

	echo "Codama output differs only by formatting after regeneration; formatting differences were ignored."
fi

echo "Codama generation and validation checks passed."
