#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IDL_DIR="$ROOT/codama/idls"
JS_TEST_DIR="$ROOT/codama/tests/js"

if ! find "$IDL_DIR" -mindepth 1 -maxdepth 1 -type f -name 'anchor_*.json' | grep -q .; then
	echo "No anchor_*.json fixtures found in $IDL_DIR" >&2
	exit 1
fi

cargo test -p pina_cli --locked --test codama_anchor_idls
pnpm --dir "$JS_TEST_DIR" install --frozen-lockfile
pnpm --dir "$JS_TEST_DIR" test
