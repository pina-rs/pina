# CI and Releases

## CI jobs

The GitHub CI workflow verifies:

- `lint:clippy`
- `lint:format`
- `verify:docs`
- `verify:security`
- `test:all` (`cargo test --all-features --locked`)
- `feature-matrix` for `pina` across explicit configurations:
  - `default` (`build:pina:default` + `test:pina:default`)
  - `no-default` (`build:pina:no-default-only` + `test:pina:no-default` + `doc:pina:no-default`)
  - `token-only` (`build:pina:token-only` + `test:pina:token-only`)
  - `all-features` (`build:pina:all-features` + `test:pina:all-features`)
- `test:program-e2e` (Example program tests, SBF builds, mollusk-svm integration tests, and BPF artifact verification)
- `test:idl` (regenerate `codama/idls`, `codama/clients/rust`, `codama/clients/js`, validate outputs, and fail on any diff)
- `cargo build --locked`
- `cargo build --all-features --locked`

Separate PR workflows also verify:

- `binary-size` for SBF artifact size reporting
- `compute-units` for tracked static CU regression reporting vs the PR base revision

This keeps code quality, behavior, documentation build health, feature-flag compatibility, and performance visibility aligned.

## Compute-unit regression policy

The `compute-units` workflow builds tracked SBF example programs on both the PR head and the PR base, runs `pina profile --json` on each `.so`, and compares the resulting static `total_cu` estimates.

Tracked programs are defined in `scripts/compute-unit-policy.json`:

- `hello_solana`
- `anchor_duplicate_mutable_accounts`
- `anchor_events`
- `anchor_sysvars`
- `anchor_system_accounts`
- `anchor_realloc`

Current policy:

- warn when `total_cu` increases by at least `+250` CU and `+5.0%`
- fail when `total_cu` increases by at least `+500` CU and `+10.0%`
- decreases and smaller increases are informational

Notes:

- this workflow intentionally uses **static** SBF estimates from `pina profile`, not runtime validator traces
- the tradeoff is deliberate: static profiling is deterministic and stable for PR-vs-base comparison
- the tracked set should favor example programs that build reliably on both the PR head and the PR base with the gallery linker used in CI; richer CPI-heavy and token-heavy flows remain covered by the main `ci` and program E2E jobs
- if the tracked set or thresholds need to change, update `scripts/compute-unit-policy.json`

Local reproduction:

```bash
profile:cu:tracked
report:cu:compare:main
```

The comparison writes artifacts to `target/cu/`, including a markdown summary and a machine-readable JSON report.

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
