---
pina_cli: feat
---

# Anchor pina.toml paths at the repository root

Configured paths may now traverse upward with `..`, and a `{{root}}` anchor expands to the git working-tree top level, so a `pina.toml` nested in `programs/<name>/` can generate clients into a repository-level directory. `[project.paths]` declares named anchors (for example `crates = "{{root}}/crates"`) usable in `project.program`, `project.idl_dir`, `clients.output`, and every per-client `output`. Anchor values may only reference `{{root}}`; anchor names must match `[A-Za-z_][A-Za-z0-9_-]*` with `root` reserved, and unknown or unterminated anchors fail closed. The repository root is discovered with `git rev-parse --show-toplevel`, so linked worktrees resolve to the worktree itself, with a nearest-ancestor `.git` fallback when git is unavailable; it is consulted only when an anchor is actually used. Literal absolute paths stay rejected, symlinked components inside the trusted base stay rejected, and every generation-time guard (filesystem roots, symlink targets, git working-tree roots for `overwrite`) is unchanged. The project configuration reference now documents the path rules, per-target scaffolding ownership, the `[migrations]` table, and worked examples for each layout.
