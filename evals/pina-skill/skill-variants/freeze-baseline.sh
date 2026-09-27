#!/usr/bin/env bash
# Refresh the `baseline` variant from a committed revision of the skill package.
#
# Re-freezing discards the reference point every recorded result was measured
# against, so only do it when starting a new round of comparisons.
set -euo pipefail

revision="${1:-HEAD}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

git -C "$root" archive "$revision" packages/pina__skill | tar -x -C "$tmp"
rm -rf "$root/evals/pina-skill/skill-variants/baseline"
mkdir -p "$root/evals/pina-skill/skill-variants/baseline"
cp -R "$tmp/packages/pina__skill/." "$root/evals/pina-skill/skill-variants/baseline/"
rm -rf "$root/evals/pina-skill/skill-variants/baseline/bin"

echo "Froze baseline from $revision"
