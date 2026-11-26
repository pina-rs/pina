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
# Stream stdin with `node:readline` instead of `fs.readFileSync(0, "utf8")`:
# Node 24.21.0..24.22.3 (and 26.8.0..26.13.0) has a heap buffer overflow in
# `ReadFileUtf8()` when a pipe delivers >64 KiB in <8 KiB short reads before a
# full 8 KiB read (https://github.com/nodejs/node/issues/66341).
PINA_BIN="$(
	cargo build --locked --quiet -p pina_cli --bin pina --message-format=json-render-diagnostics |
		node -e '
			const readline = require("node:readline");
			let executable = "";
			const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
			rl.on("line", (line) => {
				if (!line.startsWith("{")) return;
				const msg = JSON.parse(line);
				if (msg.reason === "compiler-artifact" && msg.target?.name === "pina" && typeof msg.executable === "string") {
					executable = msg.executable;
				}
			});
			rl.on("close", () => {
				if (executable) process.stdout.write(executable);
			});
		'
)"
if [ -z "$PINA_BIN" ]; then
	TARGET_DIR="$(
		cargo metadata --format-version 1 --no-deps |
			node -e '
				const path = require("node:path");
				const chunks = [];
				process.stdin.on("data", (chunk) => chunks.push(chunk));
				process.stdin.on("end", () => {
					const metadata = JSON.parse(Buffer.concat(chunks).toString("utf8"));
					if (typeof metadata.target_directory === "string" && metadata.target_directory.length > 0) {
						process.stdout.write(path.resolve(metadata.target_directory));
					}
				});
			'
	)"
	PINA_BIN="${TARGET_DIR:-$ROOT/target}/debug/pina"
fi

for program_dir in "${EXAMPLES[@]}"; do
	program_name="$(basename "$program_dir")"

	echo "Generating clients for $program_name"
	# `--npx node` uses the workspace-installed renderers directly instead of
	# downloading them, which keeps generation offline and version-pinned.
	"$PINA_BIN" generate --project "$program_dir" --npx node
done
