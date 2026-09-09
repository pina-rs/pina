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
- Clippy runs with strict workspace lint settings, including the `pina_lints` crate that holds every Pina lint.
- `security:pina-lint` runs every registered Pina lint over all example programs and secure security fixtures. It builds the workspace `pina_lint_driver` and runs cargo with it as `RUSTC_WORKSPACE_WRAPPER`.
- The [Security Lints](./security-lints.md) reference documents each rule, compliant patterns, and heuristic limitations.

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

# Generate Rust + CPI + JS + Dart clients.
codama:clients:generate

# Generate IDLs + Rust/CPI/JS/Dart clients in one command.
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

- `security:deny` runs policy checks (license allow-list, source restrictions, dependency bans). CI exposes it as the dedicated `cargo-deny` job.
- `security:audit` runs RustSec vulnerability checks over `Cargo.lock`.
- `security:zizmor` audits GitHub Actions workflows and composite actions for security anti-patterns. CI exposes it as the dedicated `zizmor` job.
- `verify:security` runs all of the checks above.

## Coverage

Generate coverage locally for Pina's runtime, CLI, Codama renderer, and profile codec fixtures:

```bash
coverage:all
```

This produces an LCOV report at `target/coverage/lcov.info`.

## Bit-precise verification

Kani proves bounded safety and correctness properties over Pina's parsers, lamport arithmetic, resize planning, fixed PinaPod validation, CPI metadata, and compact account layouts. Compact coverage includes size checks, valid initialization, patch preflight, grow and shrink ordering, and rejected updates that leave bytes and lamports unchanged:

```bash
# Fast proofs intended for every pull request.
devenv --profile kani shell -- test:kani:quick

# Heavier compact patch and layout state-machine proofs.
devenv --profile kani shell -- test:kani:compact

# Every proof harness.
devenv --profile kani shell -- test:kani
```

Kani is provided by the pinned `ifiokjr/nixpkgs` devenv input. Compact proofs use explicit unwind bounds. A successful result applies to the capacities and operation sequences encoded by each proof.

For experimental Solana-VM coverage collection (non-blocking), run:

```bash
coverage:vm:experimental
```

## Changesets

Any code changes in `crates/` or `examples/` should include a file in `.changeset/` describing impact and release type.
