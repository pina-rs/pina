---
pina_cli: fix
---

# Harden CLI containment, verification, and mutation coverage

Fix containment escapes in `pina_cli` where repository-controlled Cargo configurations in enclosing Git superprojects/submodules, enclosing Cargo workspaces (including under `git init ~`), unpacked non-Git archives, in-tree `include` directives on the fast discovery path, and symlinked or aliased in-tree `CARGO_HOME` directories could redirect build outputs outside the repository. Match Cargo's `config`-before-`config.toml` precedence for out-of-tree operator configs, fix a wire-offset bug in `validate_transaction_wire` that skipped advancing past instruction `program_id_index`, share regular-file/reparse-point validation across CLI modules via `path_security::is_plain_file`, replace `verification::read_bounded`'s manual read loop with `Read::take` + `io::copy`, add focused behavioral tests covering all remaining nightly mutation survivors across `migrations`, `path_security`, `profile_trace`, `project`, `verification`, and `workflow`, and make `mutants-pr` fail when PR diffs introduce surviving mutants or timeouts.
