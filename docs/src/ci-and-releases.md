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
