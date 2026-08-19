#!/usr/bin/env bash

set -euo pipefail

readonly EXPECTED_RANGE=">=0.8.0, <1.0.0"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT_DIR
readonly MANIFEST="$ROOT_DIR/Cargo.toml"
readonly MODE="${1:-write}"

if [[ "$MODE" != "write" && "$MODE" != "--check" ]]; then
	echo "Usage: $0 [--check]" >&2
	exit 2
fi

if ! command -v taplo >/dev/null 2>&1; then
	echo "taplo is required to preserve the Pina development dependency range." >&2
	exit 1
fi

if ! current_range="$(taplo get --file-path "$MANIFEST" --strip-newline workspace.dependencies.pina.version 2>/dev/null)"; then
	echo "Cargo.toml must define workspace.dependencies.pina.version." >&2
	exit 1
fi
if [[ "$current_range" == "$EXPECTED_RANGE" ]]; then
	exit 0
fi

if [[ "$MODE" == "--check" ]]; then
	echo "workspace.dependencies.pina.version must remain '$EXPECTED_RANGE'; found '$current_range'." >&2
	exit 1
fi

workspace_version="$(taplo get --file-path "$MANIFEST" --strip-newline workspace.package.version)"
if [[ "$current_range" != "$workspace_version" ]]; then
	echo "Refusing to replace unexpected Pina dependency range '$current_range'." >&2
	exit 1
fi

matching_lines="$(grep -c '^pina = ' "$MANIFEST" || true)"
if [[ "$matching_lines" != "1" ]]; then
	echo "Expected exactly one workspace dependency named pina; found $matching_lines." >&2
	exit 1
fi

PINA_CURRENT_RANGE="$current_range" \
	PINA_EXPECTED_RANGE="$EXPECTED_RANGE" \
	perl -pi -e '
		if (/^pina = /) {
			$replaced = s/version = "\Q$ENV{PINA_CURRENT_RANGE}\E"/version = "$ENV{PINA_EXPECTED_RANGE}"/;
			die "pina dependency line did not contain the expected release version\n" unless $replaced;
		}
	' "$MANIFEST"

updated_range="$(taplo get --file-path "$MANIFEST" --strip-newline workspace.dependencies.pina.version)"
if [[ "$updated_range" != "$EXPECTED_RANGE" ]]; then
	echo "Failed to restore the Pina development dependency range." >&2
	exit 1
fi
