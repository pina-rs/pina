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

- precompiled `cargo-dylint` 6.0.4 and `dylint-link` are downloaded from Pina's reusable `dylint-v6.0.4` tool release on first use;
- later Pina releases, runs, and projects reuse that versioned, user-owned tool cache instead of compiling Dylint;
- native lint libraries are downloaded from the GitHub release for the installed `pina` version, never compiled from repository source on the user's machine;
- the release asset name includes the exact host target and CLI version, for example `pina-lints-aarch64-apple-darwin-v0.11.0.tar.gz`;
- project-provided Dylint metadata is ignored by this command, so adding an unrelated library cannot silently extend the trusted code loaded by `pina lint`.

There is deliberately no `PINA_LINT_REVISION`. The CLI version is the lint-library release identity, while `dylintVersion` in the embedded lint catalog selects the reusable Dylint tool release. The workflow builds either versioned release before package publication can continue. A release therefore cannot require a follow-up commit merely to point back at itself.

Generated projects do not install Dylint or register a Git source under `[workspace.metadata.dylint]`. Use `pina lint` for the official catalog. Direct `cargo dylint` remains available when a project intentionally manages additional libraries itself.

The first run requires network access to download the pinned runner and lint bundle. Set `CARGO_HOME` to move both caches. Managed runners use `$CARGO_HOME/pina/tools/dylint-v<version>/<target>/`; bundles use `$CARGO_HOME/pina/lints/v<version>/<target>/`. `CARGO_TARGET_DIR` continues to control normal project build artifacts.

`cargo-dylint`, `dylint-link`, and Pina's lint libraries are precompiled. Dylint itself may still compile and cache its small `dylint-driver` executable for the project's exact nightly toolchain on first use. That driver links against the local `rustc-dev` installation and is not portable between Rust toolchain locations, so it is deliberately not shipped as a release asset. This step does not fetch or compile Pina lint source.

## Download and verification

Pina treats lint libraries as native executable code:

1. It queries `dylint-v<version>` for the pinned Dylint executables and the GitHub release whose tag exactly matches the installed CLI for the lint libraries.
2. It accepts only the target-specific asset names and canonical `github.com/pina-rs/pina/releases/download/<tag>/` URLs.
3. It checks the downloaded byte count and GitHub-provided SHA-256 asset digest before extraction.
4. It rejects nested paths, links, special files, unknown extensions, duplicate files, and oversized archives.
5. It validates the bundle manifest against the CLI's embedded catalog, Dylint version, Rust toolchain, host target, exact filenames, file sizes, and per-library SHA-256 digests.
6. It passes every verified absolute path with `--lib-path` and `--no-metadata`; Dylint neither discovers nor builds project-provided library sources.

Every published Dylint tool bundle, lint bundle, and CLI archive receives GitHub's OIDC-backed build-provenance attestation. The release job first checks whether the exact `dylint-v<version>` release already has the complete target matrix. When it does, every Pina release reuses it. When it does not exist, the workflow builds and publishes `cargo-dylint` and `dylint-link` natively for macOS, Linux glibc, Linux musl, and Windows on arm64 and x64, plus FreeBSD x64. It then builds the Pina lint libraries for that same matrix.

## Fix mode

```bash
pina lint --fix
git diff
```

Dylint delegates fix mode to `cargo fix`. Pina supplies `--allow-dirty`, `--allow-staged`, and `--allow-no-vcs` because requesting `--fix` is explicit permission to edit the current working tree, including a newly initialized project that has not entered version control yet. Only diagnostics carrying machine-applicable suggestions can be changed automatically; findings without a safe rewrite remain diagnostics. Always inspect and test the resulting diff.

## Security boundary

Dylint libraries are native compiler plugins, not passive rule files. Running a lint grants the selected library code the same local permissions as the invoking user. Pina therefore couples each bundle to an exact CLI release and host, verifies it before every use, keeps reusable Dylint executables outside project-controlled target trees, and disables project metadata for `pina lint`. Obtain the CLI from a trusted channel and review CLI upgrades as executable tooling changes.

Use direct `cargo dylint` only when you intentionally want to run additional project-defined lint libraries. Review and pin every such source before loading it.

## Exit behavior

The command exits successfully only when tool preparation, compilation, and all enabled security lints succeed. A diagnostic at an error level, a compilation failure, an unavailable network dependency on first use, or an invalid download produces a non-zero exit. Child Cargo and Dylint output stays attached to the terminal; Pina prints a short completion summary only after success.
