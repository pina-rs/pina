#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXAMPLE_NAME="anchor_system_accounts"
EXAMPLE_DIR="$ROOT/examples/$EXAMPLE_NAME"
EXAMPLE_SOURCE="$EXAMPLE_DIR/src/lib.rs"
SURFPOOL_DIR="$ROOT/target/surfpool"
IDL_PATH="$SURFPOOL_DIR/$EXAMPLE_NAME.idl.json"
PAYER_KEYPAIR="$SURFPOOL_DIR/payer.json"
PROGRAM_KEYPAIR="$SURFPOOL_DIR/program.json"
SURFPOOL_LOG="$SURFPOOL_DIR/surfpool.log"
RPC_HOST_PORT="127.0.0.1:8899"
WS_HOST_PORT="127.0.0.1:8900"
RPC_URL="http://$RPC_HOST_PORT"
WS_URL="ws://$WS_HOST_PORT"

require_bin() {
	local name="$1"
	local path
	path="$(command -v "$name" || true)"
	if [[ -z "$path" ]]; then
		echo "missing required binary on PATH: $name" >&2
		exit 1
	fi
	printf '%s\n' "$path"
}

find_sbf_sdk_template_dir() {
	local cargo_build_sbf_bin="$1"
	local cargo_bin_dir agave_root candidate
	cargo_bin_dir="$(cd "$(dirname "$cargo_build_sbf_bin")" && pwd)"
	agave_root="$(cd "$cargo_bin_dir/.." && pwd)"

	for candidate in \
		"$cargo_bin_dir/platform-tools-sdk/sbf" \
		"$agave_root/bin/platform-tools-sdk/sbf" \
		"$agave_root/lib/platform-tools-sdk/sbf"; do
		if [[ -d "$candidate" ]]; then
			printf '%s\n' "$candidate"
			return 0
		fi
	done

	echo "missing agave SBF SDK next to cargo-build-sbf under $cargo_bin_dir or $agave_root" >&2
	exit 1
}

SURFPOOL_BIN="$(require_bin surfpool)"
WAIT_FOR_THEM_BIN="$(require_bin wait-for-them)"
SOLANA_BIN="$(require_bin solana)"
SOLANA_KEYGEN_BIN="$(require_bin solana-keygen)"
CARGO_BUILD_SBF_BIN="$(require_bin cargo-build-sbf)"
SBF_SDK_TEMPLATE_DIR="$(find_sbf_sdk_template_dir "$CARGO_BUILD_SBF_BIN")"
SBF_SDK_DIR="$SURFPOOL_DIR/platform-tools-sdk/sbf"

mkdir -p "$SURFPOOL_DIR"
BACKUP_FILE="$(mktemp "$SURFPOOL_DIR/$EXAMPLE_NAME.lib.rs.XXXXXX")"
cp "$EXAMPLE_SOURCE" "$BACKUP_FILE"
SURFPOOL_PID=""

cleanup() {
	if [[ -f "$BACKUP_FILE" ]]; then
		cp "$BACKUP_FILE" "$EXAMPLE_SOURCE"
		rm -f "$BACKUP_FILE"
	fi

	if [[ -n "$SURFPOOL_PID" ]] && kill -0 "$SURFPOOL_PID" >/dev/null 2>&1; then
		kill "$SURFPOOL_PID" >/dev/null 2>&1 || true
		sleep 1
		kill -9 "$SURFPOOL_PID" >/dev/null 2>&1 || true
	fi

	pkill -x surfpool >/dev/null 2>&1 || true
	for port in 8899 8900; do
		lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | xargs -r kill >/dev/null 2>&1 || true
		lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | xargs -r kill -9 >/dev/null 2>&1 || true
	done
}
trap cleanup EXIT

rm -f "$PAYER_KEYPAIR" "$PROGRAM_KEYPAIR" "$IDL_PATH" "$SURFPOOL_LOG"
"$SOLANA_KEYGEN_BIN" new -s --no-bip39-passphrase -o "$PAYER_KEYPAIR" >/dev/null
"$SOLANA_KEYGEN_BIN" new -s --no-bip39-passphrase -o "$PROGRAM_KEYPAIR" >/dev/null
PROGRAM_ID="$("$SOLANA_KEYGEN_BIN" pubkey "$PROGRAM_KEYPAIR")"
PAYER_PUBKEY="$("$SOLANA_KEYGEN_BIN" pubkey "$PAYER_KEYPAIR")"

if [[ -z "${HOME:-}" ]]; then
	export HOME="$SURFPOOL_DIR/home"
fi
mkdir -p "$HOME"

perl -0pi -e "s/declare_id!\(\"[^\"]+\"\);/declare_id!(\"$PROGRAM_ID\");/" "$EXAMPLE_SOURCE"
cargo run -p pina_cli --quiet -- idl --path "$EXAMPLE_DIR" --output "$IDL_PATH"

rm -rf "$SURFPOOL_DIR/platform-tools-sdk"
mkdir -p "$SURFPOOL_DIR/platform-tools-sdk"
cp -R "$SBF_SDK_TEMPLATE_DIR" "$SBF_SDK_DIR"
chmod -R u+w "$SURFPOOL_DIR/platform-tools-sdk"

"$CARGO_BUILD_SBF_BIN" \
	--manifest-path "$EXAMPLE_DIR/Cargo.toml" \
	--features bpf-entrypoint \
	--sbf-out-dir "$SURFPOOL_DIR" \
	--arch v0 \
	--sbf-sdk "$SBF_SDK_DIR"

PROGRAM_SO="$SURFPOOL_DIR/${EXAMPLE_NAME}.so"
if [[ ! -f "$PROGRAM_SO" ]]; then
	PROGRAM_SO="$SURFPOOL_DIR/lib${EXAMPLE_NAME}.so"
fi
if [[ ! -f "$PROGRAM_SO" ]]; then
	echo "missing built program artifact for ${EXAMPLE_NAME}.so in $SURFPOOL_DIR" >&2
	exit 1
fi

# Clear any stale Surfpool processes that may still own the fixed test ports.
pkill -x surfpool >/dev/null 2>&1 || true
for port in 8899 8900; do
	lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | xargs -r kill >/dev/null 2>&1 || true
	lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | xargs -r kill -9 >/dev/null 2>&1 || true
done
(
	cd "$SURFPOOL_DIR"
	"$SURFPOOL_BIN" start \
		--no-tui \
		--offline \
		--no-deploy \
		--yes \
		--feature enable-sbpf-v2-deployment-and-execution \
		--feature remaining-compute-units-syscall-enabled \
		--host 127.0.0.1 \
		--port 8899 \
		--ws-port 8900 \
		--log-level warn \
		--log-path "$SURFPOOL_DIR/.surfpool/logs" \
		>"$SURFPOOL_LOG" 2>&1
) &
SURFPOOL_PID=$!
if ! "$WAIT_FOR_THEM_BIN" --silent --timeout 60000 "$RPC_HOST_PORT"; then
	echo "surfpool RPC did not become ready in time" >&2
	if [[ -f "$SURFPOOL_LOG" ]]; then
		echo "--- surfpool.log ---" >&2
		tail -n 200 "$SURFPOOL_LOG" >&2 || true
	fi
	exit 1
fi

"$SOLANA_BIN" -u "$RPC_URL" airdrop 100 --keypair "$PAYER_KEYPAIR" >/dev/null
"$SOLANA_BIN" -u "$RPC_URL" program deploy "$PROGRAM_SO" \
	--program-id "$PROGRAM_KEYPAIR" \
	--keypair "$PAYER_KEYPAIR" \
	>/dev/null

(
	cd "$ROOT/codama/tests/js"
	IDL_PATH="$IDL_PATH" \
		PAYER_KEYPAIR_PATH="$PAYER_KEYPAIR" \
		PROGRAM_ID="$PROGRAM_ID" \
		INSTRUCTION_NAME="initialize" \
		INSTRUCTION_ACCOUNTS_JSON="{\"authority\":\"$PAYER_PUBKEY\",\"wallet\":\"$PAYER_PUBKEY\"}" \
		RPC_URL="$RPC_URL" \
		WS_URL="$WS_URL" \
		node --experimental-strip-types ./surfpool-idl-invoke.ts
)

echo "Surfpool IDL invocation smoke test passed."
