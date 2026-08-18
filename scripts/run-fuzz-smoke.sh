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
readonly -a FUZZ_TARGETS=(account_deserialize parse_instruction)

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
		cargo fuzz run "$target" "$seed_input"
		cp "$seed_input" "$target_runtime_dir/"
	done

	echo "Fuzzing $target for ${FUZZ_SECONDS_PER_TARGET}s..."
	cargo fuzz run "$target" "$target_runtime_dir" -- \
		"-max_total_time=$FUZZ_SECONDS_PER_TARGET" \
		"-timeout=$FUZZ_INPUT_TIMEOUT_SECONDS"
done
