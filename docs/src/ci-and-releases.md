# CI and Releases

## CI jobs

The GitHub CI workflow verifies:

- `lint:clippy`
- `lint:format`
- `verify:docs`
- `cargo test --all-features --locked`
- `cargo build --locked`
- `cargo build --all-features --locked`

This keeps code quality, behavior, and documentation build health aligned.

## Coverage

The `coverage` workflow runs workspace coverage with `cargo llvm-cov` and publishes an LCOV artifact:

- Command: `coverage:all`
- Artifact: `target/coverage/lcov.info`
- Optional upload: Codecov (`fail_ci_if_error: false`)

## Docs publishing

The `docs-pages` workflow publishes the mdBook to GitHub Pages:

- Trigger: GitHub Release `published`
- Build command: `docs:build` (output in `docs/book`)
- Deploy target: GitHub Pages (`https://pina-rs.github.io/pina/`)

## Release workflow

Use `knope` for changelog/release management:

```bash
knope document-change
knope release
knope publish
```

Keep changeset descriptions explicit and user-impact focused.
