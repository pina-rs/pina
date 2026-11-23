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

# Build the CLI binary once so each example only pays the generation cost
# instead of re-entering Cargo 28 times.
PINA_BIN="$(
	cargo build --locked --quiet -p pina_cli --bin pina --message-format=json-render-diagnostics |
		node -e '
			const fs = require("node:fs");
			for (const line of fs.readFileSync(0, "utf8").split(/\r?\n/)) {
				if (!line.startsWith("{")) continue;
				const msg = JSON.parse(line);
				if (msg.reason === "compiler-artifact" && msg.target?.name === "pina" && typeof msg.executable === "string") {
					process.stdout.write(msg.executable);
				}
			}
		'
)"
if [ -z "$PINA_BIN" ]; then
	PINA_BIN="${CARGO_TARGET_DIR:-${CARGO_BUILD_TARGET_DIR:-$ROOT/target}}/debug/pina"
fi

for program_dir in "${EXAMPLES[@]}"; do
	program_name="$(basename "$program_dir")"

	echo "Generating clients for $program_name"
	# `--npx node` uses the workspace-installed renderers directly instead of
	# downloading them, which keeps generation offline and version-pinned.
	"$PINA_BIN" generate --project "$program_dir" --npx node
done
