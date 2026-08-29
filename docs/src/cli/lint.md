# `pina lint`

Run Pina's official security lint set against the discovered program.

## Synopsis

```text
pina lint [OPTIONS]
```

| Input                 | Default | Meaning                                                       |
| --------------------- | ------- | ------------------------------------------------------------- |
| `-p, --project <DIR>` | `.`     | Directory inside the Pina or Cargo project to discover.       |
| `--fix`               | off     | Apply machine-applicable suggestions, then rerun diagnostics. |

## Examples

```bash
pina lint
pina lint --fix
pina lint --project ./programs/counter
```

`pina lint` discovers the nearest `Pina.toml` or unambiguous Cargo package. It checks only that program package and does not lint workspace dependencies.

## Reproducible tool and lint selection

The command owns the complete official-lint selection:

- `cargo-dylint` 6.0.4 and `dylint-link` 6.0.4 are installed below Cargo home on first use;
- later runs and projects reuse that versioned, user-owned installation;
- the lint libraries come from `lints/*` at the full immutable Git revision reviewed for this CLI;
- project-provided Dylint metadata is ignored by this command, so adding an unrelated library cannot silently extend the trusted code loaded by `pina lint`.

The generated `Cargo.toml` records the same binary versions under `[workspace.metadata.bin]` and the same revision-pinned lint source under `[workspace.metadata.dylint]`. This lets Dylint-aware editors and direct `cargo dylint --all` workflows discover the official set. `pina lint` still supplies its own reviewed selection so older projects receive the set associated with their installed CLI.

The first run requires network access to install the pinned tools and fetch the lint libraries. Set `CARGO_HOME` to move Pina's managed Dylint installation; `CARGO_TARGET_DIR` continues to control normal project build artifacts.

## Fix mode

```bash
pina lint --fix
git diff
```

Dylint delegates fix mode to `cargo fix`. Pina supplies `--allow-dirty`, `--allow-staged`, and `--allow-no-vcs` because requesting `--fix` is explicit permission to edit the current working tree, including a newly initialized project that has not entered version control yet. Only diagnostics carrying machine-applicable suggestions can be changed automatically; findings without a safe rewrite remain diagnostics. Always inspect and test the resulting diff.

## Security boundary

Dylint libraries are native compiler plugins, not passive rule files. Running a lint grants the selected library code the same local permissions as the invoking user. Pina therefore selects only its official, immutable reviewed revision, keeps reusable Dylint executables outside project-controlled target trees, and disables project metadata for `pina lint`. Obtain the CLI from a trusted channel and review CLI upgrades as executable tooling changes.

Use direct `cargo dylint` only when you intentionally want to run additional project-defined lint libraries. Review and pin every such source before loading it.

## Exit behavior

The command exits successfully only when tool preparation, compilation, and all enabled security lints succeed. A diagnostic at an error level, a compilation failure, an unavailable network dependency on first use, or a failed tool installation produces a non-zero exit. Child Cargo and Dylint output stays attached to the terminal; Pina prints a short completion summary only after success.
