# Development Workflow

## Daily loop

<!-- {=dailyDevelopmentLoop} -->

```bash
devenv shell
cargo build --all-features
cargo test
lint:all
verify:docs
verify:security
test:idl
```

<!-- {/dailyDevelopmentLoop} -->

## Formatting and linting

- Rust and markdown formatting are enforced through `dprint`.
- Clippy runs with strict workspace lint settings.

## Reusable documentation blocks

- Template providers live in `template.t.md`.
- Run `docs:sync` after changing provider blocks to refresh all consumer blocks.
- Run `docs:check` (or `verify:docs`) in CI to ensure docs stay synchronized.

## Dependency/tooling updates

```bash
update:deps
```

## Dependency security

- `security:deny` runs policy checks (license allow-list, source restrictions, dependency bans).
- `security:audit` runs RustSec vulnerability checks over `Cargo.lock`.
- `verify:security` runs both checks.

## Coverage

Generate workspace coverage locally:

```bash
coverage:all
```

This produces an LCOV report at `target/coverage/lcov.info`.

## Changesets

Any code changes in `crates/` or `examples/` should include a file in `.changeset/` describing impact and release type.
