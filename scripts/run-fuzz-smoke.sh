#!/usr/bin/env bash

set -euo pipefail

readonly FUZZ_SECONDS_PER_TARGET="${PINA_FUZZ_SECONDS_PER_TARGET:-30}"
readonly MAX_FUZZ_SECONDS_PER_TARGET=300
readonly FUZZ_INPUT_TIMEOUT_SECONDS=10
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT_DIR
readonly FUZZ_CRATE_DIR="$ROOT_DIR/crates/pina_fuzz"
readonly FUZZ_DIR="$FUZZ_CRATE_DIR/fuzz"
readonly SEED_CORPUS_DIR="$FUZZ_DIR/seed_corpus"
readonly RUNTIME_CORPUS_DIR="$FUZZ_DIR/target/smoke-corpus"
readonly -a FUZZ_TARGETS=(account_deserialize migration_decode parse_instruction)

if [[ ! "$FUZZ_SECONDS_PER_TARGET" =~ ^[1-9][0-9]*$ ]]; then
	echo "PINA_FUZZ_SECONDS_PER_TARGET must be a positive integer." >&2
	exit 1
fi

if ((FUZZ_SECONDS_PER_TARGET > MAX_FUZZ_SECONDS_PER_TARGET)); then
	echo "PINA_FUZZ_SECONDS_PER_TARGET cannot exceed $MAX_FUZZ_SECONDS_PER_TARGET." >&2
	exit 1
fi

if ! command -v cargo-fuzz >/dev/null 2>&1; then
	echo "cargo-fuzz is required to run the fuzz smoke test." >&2
	exit 1
fi

HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$HOST_TARGET" ]]; then
	echo "Failed to determine rustc host target triple." >&2
	exit 1
fi
readonly HOST_TARGET

cd "$FUZZ_CRATE_DIR"
shopt -s nullglob

for target in "${FUZZ_TARGETS[@]}"; do
	target_seed_dir="$SEED_CORPUS_DIR/$target"
	target_runtime_dir="$RUNTIME_CORPUS_DIR/$target"
	seed_inputs=("$target_seed_dir"/*)

	if ((${#seed_inputs[@]} == 0)); then
		echo "No committed seed corpus found for $target." >&2
		exit 1
	fi

	mkdir -p "$target_runtime_dir"

	for seed_input in "${seed_inputs[@]}"; do
		[[ -f "$seed_input" ]] || continue
		echo "Replaying $seed_input..."
		cargo fuzz run --target "$HOST_TARGET" "$target" "$seed_input"
		cp "$seed_input" "$target_runtime_dir/"
	done
done

# All targets are already compiled above, so run the bounded fuzzing loops in
# parallel across available cores instead of paying 3x wall time sequentially.
declare -a fuzz_pids=()
declare -a fuzz_names=()
for target in "${FUZZ_TARGETS[@]}"; do
	target_runtime_dir="$RUNTIME_CORPUS_DIR/$target"
	echo "Fuzzing $target for ${FUZZ_SECONDS_PER_TARGET}s..."
	cargo fuzz run --target "$HOST_TARGET" "$target" "$target_runtime_dir" -- \
		"-max_total_time=$FUZZ_SECONDS_PER_TARGET" \
		"-timeout=$FUZZ_INPUT_TIMEOUT_SECONDS" &
	fuzz_pids+=("$!")
	fuzz_names+=("$target")
done

fuzz_failed=0
for index in "${!fuzz_pids[@]}"; do
	if ! wait "${fuzz_pids[$index]}"; then
		echo "Fuzz target '${fuzz_names[$index]}' failed." >&2
		fuzz_failed=1
	fi
done
exit "$fuzz_failed"
