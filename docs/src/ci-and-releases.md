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
- `surfpool` builds each example SBF program and exercises its runtime guards through the Surfpool SDK
- `compute-units` for tracked static CU regression reporting vs the PR base revision

This keeps code quality, behavior, documentation build health, feature-flag compatibility, and performance visibility aligned.

## Surfpool example security checks

`test:surfpool` builds every current example program and starts a fresh SDK-managed Surfpool instance for each one. Fresh instances are required because several Anchor-parity fixtures intentionally share a program ID. The harness has an explicit inventory assertion: adding an example crate or generated IDL without adding it to the test matrix fails immediately. A missing `.so` artifact also fails immediately; there are no best-effort skips.

Every program is deployed at its declared ID and is exercised with a malformed discriminator and an otherwise-valid instruction sent at a different deployment address. Every program also has an explicit expected entrypoint result: either a successful invocation or its exact expected `ProgramError` after dispatch (for example, `NotEnoughAccountKeys` or a documented custom error). Stateful examples additionally receive attacker-controlled readonly account metadata and must return their expected runtime guard error. Negative assertions use Surfpool simulation logs and a returned `InstructionError`, so an RPC, build, or deployment failure cannot satisfy them.

| Runtime guard                 | Surfpool adversarial case                                                                              |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ |
| Discriminator/data validation | Every example rejects an unknown discriminator before any state transition.                            |
| Program-ID binding            | Every example rejects its own ELF when it is deployed at an attacker-controlled address.               |
| Required account boundary     | Every selected IDL entrypoint with required accounts rejects an omitted account list.                  |
| Stateful account metadata     | State-changing examples reject attacker-controlled readonly account sets with a runtime access error.  |
| Signer authorization          | `hello_solana` rejects an unsigned user and accepts the same user only when marked as a signer.        |
| Writable and alias checks     | `anchor_duplicate_mutable_accounts` rejects both a duplicate mutable alias and a non-writable account. |
| Program address allowlist     | `anchor_declare_program` rejects an arbitrary account in place of its expected external program.       |
| Owner constraint              | `anchor_system_accounts` rejects an account explicitly created with a non-System owner.                |
| Sysvar address validation     | `anchor_sysvars` rejects ordinary accounts substituted for Clock, Rent, and Stake History.             |

The broader Pina examples also run their purpose-built Mollusk, LiteSVM, and Quasar tests in `test:program-e2e`; these cover PDA derivation, ownership, token-account, arithmetic/range, initialization, and unauthorized-mutation flows that need program-specific state setup. Surfpool complements those tests with a full, deployed SBF boundary check. It provides evidence that the listed invariants hold for the tested attacks; it is not a proof that no other attack exists.

### Known audit blockers

Two source-level authorization findings remain outside the generic account-metadata checks and must be fixed before claiming the examples are secure against state-substitution attacks:

- `anchor_realloc` currently accepts any signer together with a writable program-owned resize target; it does not bind the target to an authority, a typed state record, or a PDA. An attacker can therefore resize another user's eligible target account.
- The `security/06-duplicate-mutable-accounts/secure` fixture checks distinct, program-owned balances but does not require that its signer matches the source balance's stored owner. It can debit a victim's logical balance into an attacker's destination.

The follow-up source-fix PR must add stateful Surfpool exploit regressions for both findings: unauthorized calls must fail and leave the victim data unchanged, while the legitimate owner path must succeed. The generic harness intentionally does not treat its readonly-metadata rejection as evidence that either authorization invariant is already enforced.

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
