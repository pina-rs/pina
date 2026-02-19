# CI and Releases

## CI jobs

The GitHub CI workflow verifies:

- `lint:clippy`
- `lint:format`
- `verify:docs`
- `cargo test`
- `cargo build --locked`
- `cargo build --all-features --locked`

This keeps code quality, behavior, and documentation build health aligned.

## Release workflow

Use `knope` for changelog/release management:

```bash
knope document-change
knope release
knope publish
```

Keep changeset descriptions explicit and user-impact focused.
