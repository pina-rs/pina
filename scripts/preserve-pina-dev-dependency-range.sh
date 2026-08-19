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

read_manifest_values() {
	cargo metadata \
		--format-version 1 \
		--locked \
		--manifest-path "$MANIFEST" \
		--no-deps |
		jq -er '
			[
				.packages[]
				| select(.name == "pina_macros")
				| .version,
				  (.dependencies[] | select(.name == "pina" and .kind == "dev") | .req)
			]
			| select(length == 2)
			| .[]
		'
}

if ! manifest_values="$(read_manifest_values)"; then
	echo "Cargo metadata must contain exactly one pina_macros package and its Pina development dependency." >&2
	exit 1
fi
workspace_version="$(sed -n '1p' <<<"$manifest_values")"
readonly workspace_version
current_range="$(sed -n '2p' <<<"$manifest_values")"
if [[ "$current_range" == "$EXPECTED_RANGE" ]]; then
	exit 0
fi

if [[ "$MODE" == "--check" ]]; then
	echo "workspace.dependencies.pina.version must remain '$EXPECTED_RANGE'; found '$current_range'." >&2
	exit 1
fi

if [[ "$current_range" != "^$workspace_version" ]]; then
	echo "Refusing to replace unexpected Pina dependency range '$current_range'." >&2
	exit 1
fi

matching_lines="$(grep -c '^pina = ' "$MANIFEST" || true)"
if [[ "$matching_lines" != "1" ]]; then
	echo "Expected exactly one workspace dependency named pina; found $matching_lines." >&2
	exit 1
fi

PINA_CURRENT_RANGE="$workspace_version" \
	PINA_EXPECTED_RANGE="$EXPECTED_RANGE" \
	perl -pi -e '
		if (/^pina = /) {
			$replaced = s/version = "\Q$ENV{PINA_CURRENT_RANGE}\E"/version = "$ENV{PINA_EXPECTED_RANGE}"/;
			die "pina dependency line did not contain the expected release version\n" unless $replaced;
		}
	' "$MANIFEST"

if ! manifest_values="$(read_manifest_values)"; then
	echo "Cargo metadata became invalid after restoring the Pina development dependency range." >&2
	exit 1
fi
updated_range="$(sed -n '2p' <<<"$manifest_values")"
if [[ "$updated_range" != "$EXPECTED_RANGE" ]]; then
	echo "Failed to restore the Pina development dependency range." >&2
	exit 1
fi
