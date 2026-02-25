# CI and Releases

## CI jobs

The GitHub CI workflow verifies:

- `lint:clippy`
- `lint:format`
- `verify:docs`
- `verify:security`
- `test:all` (`cargo test --all-features --locked`)
- `test:anchor-parity` (Anchor parity examples + `pina_bpf` nightly build (`-Z build-std=core,alloc`) + ignored BPF artifact verification tests)
- `test:idl` (regenerate `codama/idls`, `codama/clients/rust`, `codama/clients/js`, validate outputs, and fail on any diff)
- `cargo build --locked`
- `cargo build --all-features --locked`

This keeps code quality, behavior, and documentation build health aligned.

## Coverage

The `coverage` workflow runs focused coverage with `cargo llvm-cov` and publishes an LCOV artifact:

- Command: `coverage:all`
- Artifact: `target/coverage/lcov.info`
- Optional upload: Codecov (`fail_ci_if_error: false`)

## Docs publishing

The `docs-pages` workflow publishes the mdBook to GitHub Pages:

- Trigger: pushes to `main` that touch docs + GitHub Release `published`
- Build command: `docs:build` (output in `docs/book`)
- Deploy target: GitHub Pages (`https://pina-rs.github.io/pina/`)

## CLI asset releases

The `assets` workflow only publishes binaries for CLI tags:

- Required tag format: `pina_cli/v<version>`
- Tag/version check: release tag must match `crates/pina_cli/Cargo.toml`
- Build scope: `crates/pina_cli` only (`package = "pina_cli"`)

## Release workflow

Use `knope` for changelog/release management:

<!-- {=releaseWorkflowCommands} -->

```bash
knope document-change
knope release
knope publish
```

<!-- {/releaseWorkflowCommands} -->

Keep changeset descriptions explicit and user-impact focused.
