#!/usr/bin/env bash
set -euo pipefail

# Regenerate the committed `codama/idls` fixtures and every client family with
# `pina generate`. Each example's `pina.toml` is the single source of truth for
# its IDL location and client outputs, so this script only discovers the
# programs and drives the CLI once per program.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"

mapfile -t EXAMPLES < <(find "$ROOT/examples" -mindepth 1 -maxdepth 1 -type d | sort)

if [ "${#EXAMPLES[@]}" -eq 0 ]; then
	echo "No examples found in $ROOT/examples" >&2
	exit 1
fi

# The TypeScript and Dart CLI renderers are resolved from this workspace
# package, so its bundle has to exist before generation runs.
pnpm --dir "$ROOT" run build:codama-renderer-cli

for program_dir in "${EXAMPLES[@]}"; do
	program_name="$(basename "$program_dir")"

	echo "Generating clients for $program_name"
	# `--npx node` uses the workspace-installed renderers directly instead of
	# downloading them, which keeps generation offline and version-pinned.
	cargo run -p pina_cli --quiet -- generate --project "$program_dir" --npx node
done
