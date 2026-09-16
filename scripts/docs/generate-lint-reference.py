#!/usr/bin/env python3
"""Generate `docs/src/lints.md` from the CLI's lint reference table.

The reference table in `crates/pina_cli/src/lint_reference.rs` is the single
source of truth for what each lint enforces and how to bless an exception, so
`pina lint --explain` and the published page cannot drift. This script renders
that table as the docs page; `crates/pina_cli/tests/lint_reference_docs.rs`
fails the build when the two disagree.

Usage: `devenv shell -- python3 scripts/docs/generate-lint-reference.py`
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SOURCE = REPOSITORY_ROOT / "crates/pina_cli/src/lint_reference.rs"
DESTINATION = REPOSITORY_ROOT / "docs/src/lint-reference.md"

ENTRY = re.compile(
    r'\{\s*name: "([^"]+)",\s*default_level: "([^"]+)",\s*'
    r'contract: "(.*?)",\s*rationale: "(.*?)",\s*blessing: "(.*?)",\s*\}',
    re.S,
)


def unescape(value: str) -> str:
    """Join a Rust `\\`-continued string literal and collapse its whitespace."""
    value = re.sub(r"\\\s*\n\s*", " ", value)
    value = value.replace('\\"', '"').replace("\\\\", "\\")
    return re.sub(r"\s+", " ", value).strip()


def entries() -> list[tuple[str, str, str, str, str]]:
    source = SOURCE.read_text()
    body = source[source.index("pub const LINT_REFERENCE"):]
    parsed = [
        (
            match.group(1),
            match.group(2),
            unescape(match.group(3)),
            unescape(match.group(4)),
            unescape(match.group(5)),
        )
        for match in ENTRY.finditer(body)
    ]
    if not parsed:
        raise SystemExit(f"no lint reference entries found in {SOURCE}")
    return parsed


def render(parsed: list[tuple[str, str, str, str, str]]) -> str:
    # `dprint` formats markdown with `textWrap = "never"`, so every paragraph
    # is emitted on one line. Generating the wrapped form would make the
    # committed file differ from both the generator and the formatter, and the
    # `verify:docs` check would fail on a clean checkout.
    lines = [
        "# Lint reference",
        "",
        "Every lint Pina ships, with the contract it enforces, why violating it is a vulnerability, and how to bless an intentional exception.",
        "",
        "`pina lint --explain <LINT>` prints the same entry for one lint without leaving the terminal.",
        "",
        "## How to read this page",
        "",
        "A lint is a claim about your program, and a finding is the compiler telling you it could not prove that claim. Two things follow from that:",
        "",
        "- **Fix the finding, do not silence it.** Every entry names the API or restructure that satisfies the contract. That is the intended resolution.",
        "- **Blessing is a documented decision, not a suppression.** Where an entry describes an `#[allow]`, it is scoped to the smallest item and the entry says what to write in the comment. A crate-wide `allow` in `pina.toml` removes the signal for everyone, including the next person who adds a genuine violation to the same module.",
        "",
        "### Configuring levels",
        "",
        "Default levels are set per lint. Override them in the `[lints]` table of `pina.toml`:",
        "",
        "```toml",
        "[lints]",
        "# Raise a heuristically-noisy warn to a hard requirement.",
        'deny_heap_allocations_in_onchain_instruction_handlers = "deny"',
        "# Accept a documented, deliberate exception at crate scope.",
        'require_explicit_discriminators_and_seed_namespaces = "allow"',
        "```",
        "",
        "Prefer an item-scoped `#[allow]` over a crate-scoped `allow`, with a comment naming the invariant that makes the exemption safe.",
        "",
        "## Default levels",
        "",
        "A `deny` lint is a security property: the build fails, and there is no supported way to ship without addressing it. A `warn` lint is heuristically detected or advisory, so a false positive is expected occasionally and the blessing guidance is what matters.",
        "",
    ]
    # `dprint` pads every markdown table cell to the widest entry in its
    # column, including the header, so the generator emits that layout rather
    # than depending on the formatter to produce the committed bytes.
    rows = [(f"[{name}](#{name})", f"`{level}`") for name, level, _, _, _ in parsed]
    lint_width = max([len("Lint"), *(len(lint) for lint, _ in rows)])
    level_width = max([len("Default"), *(len(level) for _, level in rows)])
    lines += [
        f"| {'Lint'.ljust(lint_width)} | {'Default'.ljust(level_width)} |",
        f"| {'-' * lint_width} | {'-' * level_width} |",
        *(f"| {lint.ljust(lint_width)} | {level.ljust(level_width)} |" for lint, level in rows),
        "",
    ]

    for name, level, contract, rationale, blessing in parsed:
        lines += [
            f"## {name}",
            "",
            f"Default level: `{level}`",
            "",
            f"**Contract.** {contract}",
            "",
            f"**Why this matters.** {rationale}",
            "",
            f"**Blessing an exception.** {blessing}",
            "",
        ]

    return "\n".join(lines)


def main() -> int:
    rendered = render(entries())
    if "--check" in sys.argv:
        current = DESTINATION.read_text() if DESTINATION.exists() else ""
        if current != rendered:
            print(
                f"{DESTINATION.relative_to(REPOSITORY_ROOT)} is out of date; "
                "run scripts/docs/generate-lint-reference.py",
                file=sys.stderr,
            )
            return 1
        print(f"{DESTINATION.relative_to(REPOSITORY_ROOT)} is up to date")
        return 0

    DESTINATION.write_text(rendered)
    print(f"wrote {DESTINATION.relative_to(REPOSITORY_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
