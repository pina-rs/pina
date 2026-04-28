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
- Clippy runs with strict workspace lint settings, and `lint:clippy` also checks the custom Dylint crates under `lints/`.

## Reusable documentation blocks

- Template providers live in `templates/*.t.md`.
- Prefer updating the shared provider block first when the same guidance appears in the README, crate readmes, and mdBook.
- Run `docs:sync` after changing provider blocks to refresh all consumer blocks.
- Run `docs:check` (or `verify:docs`) in CI to ensure docs stay synchronized.

## Dependency/tooling updates

```bash
update:deps
```

## Codama/IDL workflow

<!-- {=codamaWorkflowCommands} -->

```bash
# Generate Codama IDLs for all examples.
codama:idl:all

# Generate Rust + JS clients.
codama:clients:generate

# Generate IDLs + Rust/JS clients in one command.
pina codama generate

# Run the complete Codama pipeline.
codama:test

# Run IDL fixture drift + validation checks used by CI.
test:idl

# Run Quasar SVM generated-client e2e checks alongside LiteSVM.
pnpm run test:quasar-svm
```

<!-- {/codamaWorkflowCommands} -->

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
