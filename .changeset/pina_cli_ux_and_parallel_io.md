---
pina_cli: minor
---

Add UX improvements and parallel file I/O to the `pina` CLI.

- **Parallel file reading**: `resolve_crate` now discovers all module paths sequentially, then reads source files in parallel via `rayon` for faster IDL generation on crates with many modules.
- **Colored output**: Error messages and success indicators now use `owo-colors` for semantic terminal styling.
- **Summary table**: `pina idl` prints a `comfy-table` summary showing instruction, account, PDA, and error counts after generation.
- **`docs` subcommand**: `pina docs <topic>` renders `.t.md` template files in-terminal using `termimad`.
- **New dependencies**: `rayon`, `indicatif`, `owo-colors`, `comfy-table`, `miette`, `termimad`.
