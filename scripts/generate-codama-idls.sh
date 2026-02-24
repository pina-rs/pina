#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IDL_DIR="$ROOT/codama/idls"
mkdir -p "$IDL_DIR"

mapfile -t EXAMPLES < <(find "$ROOT/examples" -mindepth 1 -maxdepth 1 -type d | sort)

if [ "${#EXAMPLES[@]}" -eq 0 ]; then
	echo "No examples found in $ROOT/examples" >&2
	exit 1
fi

for program_dir in "${EXAMPLES[@]}"; do
	program_name="$(basename "$program_dir")"
	output_path="$IDL_DIR/$program_name.json"

	echo "Generating Codama IDL for $program_name"
	cargo run -p pina_cli --quiet -- idl --path "$program_dir" --output "$output_path"
done
