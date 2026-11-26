---
pina_cli: fix
---

# Accept operator-configured external Cargo target directories

Pina now accepts an external `build.target-dir` configured in an operator Cargo config outside the repository (such as a parent worktree `.cargo/config.toml` or `$CARGO_HOME/config.toml`) and normalizes any `..` segments Cargo leaves in `target_directory`, while repository-local Cargo configuration inside the worktree or workspace still cannot redirect builds outside the tree.
