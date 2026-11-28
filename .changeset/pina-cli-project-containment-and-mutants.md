---
pina_cli: fix
---

# Tighten Cargo config containment and kill project.rs mutants

Fix containment escapes in `pina_cli` where repository-controlled Cargo configurations in enclosing Git superprojects/submodules, enclosing Cargo workspaces (including under `git init ~`), unpacked non-Git archives, in-tree `include` directives on the fast discovery path, and symlinked or aliased in-tree `CARGO_HOME` directories could redirect build outputs outside the repository. Match Cargo's `config`-before-`config.toml` precedence for out-of-tree operator configs, add regression and mutant-killing tests covering all surviving `project.rs` mutants, and make `mutants-pr` fail when PR diffs introduce surviving mutants or timeouts.
