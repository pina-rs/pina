# CI and Releases

## CI jobs

The GitHub CI workflow verifies:

- `lint:clippy`
- `lint:format`
- `verify:docs`
- `verify:security` (Rust security checks plus moderate-or-higher npm lockfile advisories)
- `test:all` (`cargo test --all-features --locked`)
- `test:fuzz:smoke` (replay committed seeds, then fuzz every target for 30 seconds)
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

## Dependency and bootstrap integrity

CI installs Devenv from the immutable revision in `devenv.lock`, configures the Devenv binary cache with its static endpoint and public key, and includes the environment lockfiles in tool-cache keys. The shared security task also runs `pnpm audit` at the moderate severity threshold. Weekly grouped Dependabot updates keep the npm lockfile current without making every newly available package version a blocking PR check.

## Fuzz smoke testing

The `fuzz` CI job runs in parallel with the rest of CI. It installs `cargo-fuzz` from the Nixpkgs revision locked by `devenv.lock`, replays every input under `crates/pina_fuzz/fuzz/seed_corpus/`, and gives each fuzz target 30 seconds of mutation time. Keeping both targets in one bounded job limits runner overhead and avoids adding their durations to the main test job's critical path.

Failed runs upload `crates/pina_fuzz/fuzz/artifacts/`. After reproducing a useful artifact locally, add it to the corresponding seed corpus so later CI runs replay the regression before mutation starts.

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
- reviewed redesigns may record an absolute total in `approvedTotals`; the allowance applies only while the base is below that total, so later increases are still evaluated normally
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

The `publish` workflow builds and uploads the `pina` CLI binary for all supported platforms on release tag pushes (`v*`):

- Trigger: tag push `v*` (created by the `release-pr` workflow after a release PR merges)
- Build scope: `crates/pina_cli` only (`bin = "pina"`)
- Artifacts: `pina-<target>-<tag>` archives with `sha256`/`sha512` checksums, attested with build provenance

## Release workflow

Use `monochange` for changelog/release management:

<!-- {=releaseWorkflowCommands} -->

```bash
monochange run change
monochange run release
monochange step publish-packages
```

<!-- {/releaseWorkflowCommands} -->

Keep changeset descriptions explicit and user-impact focused.
