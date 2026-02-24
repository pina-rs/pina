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

## Codama/IDL workflow

```bash
# Regenerate all example IDLs.
codama:idl:all

# Generate clients from Codama JSON.
codama:clients:generate

# Full Codama pipeline (build CLI, generate IDLs, generate clients, checks).
codama:test

# CI-oriented IDL validation.
test:idl
```

## Dependency security

- `security:deny` runs policy checks (license allow-list, source restrictions, dependency bans).
- `security:audit` runs RustSec vulnerability checks over `Cargo.lock`.
- `verify:security` runs both checks.

## Coverage

Generate coverage locally for `pina` and `pina_cli`:

```bash
coverage:all
```

This produces an LCOV report at `target/coverage/lcov.info`.

For experimental Solana-VM coverage collection (non-blocking), run:

```bash
coverage:vm:experimental
```

## Changesets

Any code changes in `crates/` or `examples/` should include a file in `.changeset/` describing impact and release type.
