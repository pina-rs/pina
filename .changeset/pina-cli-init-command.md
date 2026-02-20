---
pina_cli: patch
---

Added a new `pina init` command to scaffold a starter Pina program project.

The command now:

- Creates a new project directory (default `./<name>`) with `Cargo.toml`, `src/lib.rs`, `README.md`, and `.gitignore`.
- Provides a `--path` option to control destination.
- Provides a `--force` option to overwrite scaffold files when they already exist.

The generated project includes a minimal no-std Pina program skeleton with entrypoint wiring and an `Initialize` instruction.
