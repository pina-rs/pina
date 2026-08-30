# CI and Releases

## CI jobs

The GitHub CI workflow verifies:

- `lint:clippy`
- `lint:format`
- `verify:docs`
- `verify:security`
- `test:all` (workspace Rust tests, standalone fuzz-target compilation, and npm package tests)
- `test:npm-packages` (scoped package metadata, native-target coverage, launchers, and skill installation)
- `feature-matrix` for `pina` across explicit configurations:
  - `default` (`build:pina:default` + `test:pina:default`)
  - `no-default` (`build:pina:no-default-only` + `test:pina:no-default` + `doc:pina:no-default`)
  - `token-only` (`build:pina:token-only` + `test:pina:token-only`)
  - `all-features` (`build:pina:all-features` + `test:pina:all-features`)
- `test:program-e2e` (Example program tests, SBF builds, mollusk-svm integration tests, and BPF artifact verification)
- `test:idl` (regenerate `codama/idls` and the Rust, JavaScript, and Dart clients; validate every output; and fail on any diff)
- `cargo build --locked`
- `cargo build --all-features --locked`

Separate PR workflows also verify:

- `binary-size` for SBF artifact size reporting
- `surfpool` builds each example SBF program and exercises its runtime guards through the Surfpool SDK
- `compute-units` for tracked static CU regression reporting vs the PR base revision

The main CI workflow also runs `release-publish` on every pull request. When a PR contains releaseable changesets, the job creates the same release commit as the production release workflow and keeps that commit local to the runner. Registry readiness and a publish dry-run both select every package from its embedded release record, and CI requires their package sets to match so a newly added package cannot be omitted by a maintained allowlist. Cargo cannot completely verify dependent crates until their same-release dependencies exist in crates.io; Monochange plans those packages and publishes them in dependency order during the real release. Prepared release PRs are checked directly. Pull requests without a publishable release keep the job visible but skip the preflight explicitly.

This keeps code quality, behavior, documentation build health, feature-flag compatibility, and performance visibility aligned.

## Surfpool example security checks

`test:surfpool` builds every current example program and starts a fresh SDK-managed Surfpool instance for each one. Fresh instances are required because several Anchor-parity fixtures intentionally share a program ID. The harness has an explicit inventory assertion: adding an example crate or generated IDL without adding it to the test matrix fails immediately. A missing `.so` artifact also fails immediately; there are no best-effort skips.

Every program is deployed at its declared ID and is exercised with a malformed discriminator and an otherwise-valid instruction sent at a different deployment address. Every program also has an explicit expected entrypoint result: either a successful invocation or its exact expected `ProgramError` after dispatch (for example, `NotEnoughAccountKeys` or a documented custom error). Stateful examples additionally receive attacker-controlled readonly account metadata and must return their expected runtime guard error. Negative assertions use Surfpool simulation logs and a returned `InstructionError`, so an RPC, build, or deployment failure cannot satisfy them.

| Runtime guard                 | Surfpool adversarial case                                                                                                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Discriminator/data validation | Every example rejects an unknown discriminator before any state transition.                                                                                               |
| Program-ID binding            | Every example rejects its own ELF when it is deployed at an attacker-controlled address.                                                                                  |
| Required account boundary     | Every selected IDL entrypoint with required accounts rejects an omitted account list.                                                                                     |
| Stateful account metadata     | State-changing examples reject attacker-controlled readonly account sets with a runtime access error.                                                                     |
| Signer authorization          | `hello_solana` rejects an unsigned user and accepts the same user only when marked as a signer.                                                                           |
| Writable and alias checks     | `anchor_duplicate_mutable_accounts` rejects both a duplicate mutable alias and a non-writable account.                                                                    |
| Program address allowlist     | `anchor_declare_program` rejects an arbitrary account in place of its expected external program.                                                                          |
| Owner constraint              | `anchor_system_accounts` rejects an account explicitly created with a non-System owner.                                                                                   |
| Sysvar address validation     | `anchor_sysvars` rejects ordinary accounts substituted for Clock, Rent, and Stake History.                                                                                |
| Authority-bound PDA resize    | `anchor_realloc` proves initialize/grow/shrink for its owner and rejects an unrelated signer, a forged typed account, and duplicate resize targets without data mutation. |

The broader Pina examples also run their purpose-built Mollusk, LiteSVM, and Quasar tests in `test:program-e2e`; these cover PDA derivation, ownership, token-account, arithmetic/range, initialization, and unauthorized-mutation flows that need program-specific state setup. Surfpool complements those tests with a full, deployed SBF boundary check. It provides evidence that the listed invariants hold for the tested attacks; it is not a proof that no other attack exists.

### Known audit blocker

The `security/06-duplicate-mutable-accounts/secure` fixture checks distinct, program-owned balances but does not require that its signer matches the source balance's stored owner. It can debit a victim's logical balance into an attacker's destination. Its source fix needs a stateful regression: an unauthorized transfer must fail without changing either balance, while the legitimate owner path must succeed. The generic harness intentionally does not treat readonly-metadata rejection as evidence that this authorization invariant is enforced.

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

## CLI and npm releases

The `publish` workflow builds and uploads the `pina` CLI binary for all supported platforms on release tag pushes (`v*`):

- Trigger: tag push `v*` (created by the `release-pr` workflow after a release PR merges)
- Build scope: `crates/pina_cli` only (`bin = "pina"`)
- Artifacts: `pina-<target>-<tag>` archives with `sha256`/`sha512` checksums, attested with build provenance

The same workflow builds, uploads, and attests the CLI archives, then publishes the crates, including `pina_lints` — the crate whose lints are statically compiled into the `pina_lint_driver` binary. The lint driver is not a release asset: on first use, `pina lint` installs it from the `pina_lints` crates.io release matching the CLI version, so no separate tool or lint-bundle release jobs remain in the workflow.

`crates/pina_cli/lints.json` (schema version 3) is the catalog of lint names and default levels used to validate the `[lints]` configuration in `pina.toml`. A test in `pina_lints` keeps the catalog in sync with the registered lints.

After attestation, the publish job downloads those same archives and fills the platform-specific npm packages. `@pina-rs/cli` uses optional dependencies to install the matching native package without compiling Rust. The release target and npm package matrices are checked one-to-one for:

- macOS arm64 and x64
- Linux arm64 and x64 with glibc or musl
- Windows arm64 and x64
- FreeBSD x64

The same trusted-publishing workflow also publishes `@pina-rs/codama-nodes` and `@pina-rs/skill`. Dry-run package inspection verifies the CLI launchers, native binaries, Codama CommonJS/ESM/type entrypoints, and skill runtime files before any registry write.

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

### First-time packages

A new crates.io crate or npm package must exist before registry-side trusted publishing can be configured. Before its first real release, a registry owner should run `monochange step placeholder-publish --dry-run --package
<package-id>`, publish the `0.0.0` placeholder with the same command without `--dry-run`, then configure repository `pina-rs/pina`, workflow `publish.yml`, and environment `publisher` as its trusted publisher. The placeholder both prevents name squatting and lets PR publication preflight validate later versions before the release workflow obtains an OIDC token.
