---
pina_cli: minor
---

Add UX improvements and parallel file I/O to the `pina` CLI.

- **Parallel file reading**: `resolve_crate` reads sibling module files in parallel via `rayon` while preserving deterministic parsing and error reporting.
- **Colored output**: Error messages and success indicators now use `owo-colors` for semantic terminal styling.
- **Summary table**: `pina idl` prints a `comfy-table` summary showing instruction, account, PDA, and error counts after generation.
- **`docs` subcommand**: `pina docs <topic>` renders bundled `.t.md` documentation in-terminal using `termimad`, with `PINA_TEMPLATES_DIR` support for custom topics.
- **New dependencies**: `rayon`, `owo-colors`, `comfy-table`, `termimad`.
