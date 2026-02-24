# Development Workflow

## Daily loop

```bash
devenv shell
cargo build --all-features
cargo test
lint:all
verify:docs
```

## Formatting and linting

- Rust and markdown formatting are enforced through `dprint`.
- Clippy runs with strict workspace lint settings.

## Dependency/tooling updates

```bash
update:deps
```

## Coverage

Generate workspace coverage locally:

```bash
coverage:all
```

This produces an LCOV report at `target/coverage/lcov.info`.

## Changesets

Any code changes in `crates/` or `examples/` should include a file in `.changeset/` describing impact and release type.
