#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXAMPLE_NAME="anchor_system_accounts"
EXAMPLE_DIR="$ROOT/examples/$EXAMPLE_NAME"
EXAMPLE_SOURCE="$EXAMPLE_DIR/src/lib.rs"
IDL_PATH="$ROOT/target/surfpool/$EXAMPLE_NAME.idl.json"
PAYER_KEYPAIR="$ROOT/target/surfpool/payer.json"
PROGRAM_KEYPAIR="$ROOT/target/surfpool/program.json"
SURFPOOL_LOG_DIR="$ROOT/target/surfpool"
SURFPOOL_LOG="$SURFPOOL_LOG_DIR/surfpool.log"
RPC_URL="http://127.0.0.1:8899"
WS_URL="ws://127.0.0.1:8900"
SURFPOOL_BIN="$(command -v surfpool)"
SOLANA_BIN="$(command -v solana)"
SOLANA_KEYGEN_BIN="$(command -v solana-keygen)"
CARGO_BUILD_SBF_BIN="$(command -v cargo-build-sbf)"
SBF_SDK_DIR="$SURFPOOL_LOG_DIR/platform-tools-sdk/sbf"

mkdir -p "$SURFPOOL_LOG_DIR"
BACKUP_FILE="$(mktemp "$SURFPOOL_LOG_DIR/$EXAMPLE_NAME.lib.rs.XXXXXX")"
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
}
trap cleanup EXIT

if [[ ! -x "$SURFPOOL_BIN" ]]; then
	echo "missing surfpool binary: $SURFPOOL_BIN" >&2
	exit 1
fi

if [[ ! -x "$SOLANA_BIN" ]] || [[ ! -x "$SOLANA_KEYGEN_BIN" ]] || [[ ! -x "$CARGO_BUILD_SBF_BIN" ]]; then
	echo "missing solana binaries on PATH (need agave + surfpool from devenv)" >&2
	exit 1
fi

rm -f "$PAYER_KEYPAIR" "$PROGRAM_KEYPAIR" "$IDL_PATH" "$SURFPOOL_LOG"

"$SOLANA_KEYGEN_BIN" new -s --no-bip39-passphrase -o "$PAYER_KEYPAIR" >/dev/null
"$SOLANA_KEYGEN_BIN" new -s --no-bip39-passphrase -o "$PROGRAM_KEYPAIR" >/dev/null
PROGRAM_ID="$("$SOLANA_KEYGEN_BIN" pubkey "$PROGRAM_KEYPAIR")"
PAYER_PUBKEY="$("$SOLANA_KEYGEN_BIN" pubkey "$PAYER_KEYPAIR")"

if [[ -z "${HOME:-}" ]]; then
	CANDIDATE_HOME="/Users/$(whoami)"
	if [[ -d "$CANDIDATE_HOME" ]]; then
		export HOME="$CANDIDATE_HOME"
	else
		export HOME="$SURFPOOL_LOG_DIR/home"
	fi
fi
mkdir -p "$HOME"

perl -0pi -e "s/declare_id!\(\"[^\"]+\"\);/declare_id!(\"$PROGRAM_ID\");/" "$EXAMPLE_SOURCE"

cargo run -p pina_cli --quiet -- idl --path "$EXAMPLE_DIR" --output "$IDL_PATH"

mkdir -p "$SBF_SDK_DIR/scripts"
# Create stub SDK scripts. These are expected by cargo-build-sbf but are not
# invoked during the smoke test. When agave is installed from nixpkgs, the
# SDK scripts directory may not exist as a standalone path.
for script_name in install.sh dump.sh objcopy.sh package.sh strip.sh; do
	local_bin="$(command -v "$script_name" 2>/dev/null || true)"
	if [ -n "$local_bin" ]; then
		ln -sf "$local_bin" "$SBF_SDK_DIR/scripts/$script_name"
	else
		# Create a no-op stub so cargo-build-sbf doesn't fail looking for it.
		printf '#!/usr/bin/env bash\nexit 0\n' > "$SBF_SDK_DIR/scripts/$script_name"
		chmod +x "$SBF_SDK_DIR/scripts/$script_name"
	fi
done

cat >"$SBF_SDK_DIR/env.sh" <<'EOF'
#
# Configures the SBF SDK environment
#

if [ -z "$sbf_sdk" ]; then
  sbf_sdk=.
fi

# Ensure the sdk is installed
"$sbf_sdk"/scripts/install.sh

# Use the SDK's version of llvm to build the compiler-builtins for SBF
export CC="$sbf_sdk/dependencies/platform-tools/llvm/bin/clang"
export AR="$sbf_sdk/dependencies/platform-tools/llvm/bin/llvm-ar"
export OBJDUMP="$sbf_sdk/dependencies/platform-tools/llvm/bin/llvm-objdump"
export OBJCOPY="$sbf_sdk/dependencies/platform-tools/llvm/bin/llvm-objcopy"
EOF

"$CARGO_BUILD_SBF_BIN" \
	--manifest-path "$EXAMPLE_DIR/Cargo.toml" \
	--features bpf-entrypoint \
	--sbf-out-dir "$SURFPOOL_LOG_DIR" \
	--arch v0 \
	--sbf-sdk "$SBF_SDK_DIR"

PROGRAM_SO="$SURFPOOL_LOG_DIR/${EXAMPLE_NAME}.so"
if [[ ! -f "$PROGRAM_SO" ]]; then
	PROGRAM_SO="$SURFPOOL_LOG_DIR/lib${EXAMPLE_NAME}.so"
fi
if [[ ! -f "$PROGRAM_SO" ]]; then
	echo "missing built program artifact for ${EXAMPLE_NAME}.so in $SURFPOOL_LOG_DIR" >&2
	exit 1
fi

# Stop any stale local surfpool process from previous runs.
pkill -f "${SURFPOOL_BIN//\//\/} start" >/dev/null 2>&1 || true

(
	cd "$SURFPOOL_LOG_DIR"
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
		--log-path "$SURFPOOL_LOG_DIR/.surfpool/logs" \
		>"$SURFPOOL_LOG" 2>&1
) &
SURFPOOL_PID=$!

ready=0
for _ in $(seq 1 60); do
	if "$SOLANA_BIN" -u "$RPC_URL" cluster-version >/dev/null 2>&1; then
		ready=1
		break
	fi
	sleep 1
done

if [[ "$ready" -ne 1 ]]; then
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
