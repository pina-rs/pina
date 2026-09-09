# CI and Releases

## CI jobs

The GitHub CI workflow verifies:

- `lint:clippy`
- `lint:format`
- `verify:docs`
- `security:pina-lint`, `security:audit`, and `security:npm-audit`
- `security:deny` in a dedicated `cargo-deny` job
- `security:zizmor` in a dedicated `zizmor` job
- `test:all` (workspace Rust tests, standalone fuzz-target compilation, and npm package tests)
- `test:npm-packages` (scoped package metadata, native-target coverage, launchers, and skill installation)
- `test:kani:quick` in a dedicated job for parser, arithmetic, compact-sizing, fixed-layout, and CPI invariants
- `test:kani:compact` in a separate job for bounded compact-patch state machines, initialization, and failed-update rollback
- `feature-matrix` for `pina` across explicit configurations:
  - `default` (`build:pina:default` + `test:pina:default`)
  - `no-default` (`build:pina:no-default-only` + `test:pina:no-default` + `doc:pina:no-default`)
  - `token-only` (`build:pina:token-only` + `test:pina:token-only`)
  - `compact-only` (`build:pina:compact-only` + `test:pina:compact-only` + `doc:pina:compact-only`)
  - `account-resize-only` (`build:pina:account-resize-only` + `test:pina:account-resize-only`)
  - `all-features` (`build:pina:all-features` + `test:pina:all-features`)
- `test:program-e2e` (Example program tests, SBF builds, mollusk-svm integration tests, and BPF artifact verification)
- `test:idl` (regenerate `codama/idls` and the Rust, JavaScript, and Dart clients; validate every output; and fail on any diff)
- `windows-cli` (portability: the `pina_cli` test suite on `x86_64-pc-windows-msvc`)
- `pina-test` (`verify:pina-test` for the published Surfpool test harness)
- `fuzz` (`test:fuzz:smoke` for every fuzz target, with artifacts uploaded)
- `miri` (`test:miri` zero-copy regressions for loader guards and token helpers)
- `cargo build --locked`
- `cargo build --all-features --locked`

Separate PR workflows also verify:

- `surfpool` builds each example SBF program and exercises its runtime guards through the Surfpool SDK
- `performance` for instruction compute units, every example program's build size and static CU estimate, CLI timings, and core host timings against the PR base

The main CI workflow also runs `release-publish` on every pull request. When a PR contains releaseable changesets, the job creates the same release commit as the production release workflow and keeps that commit local to the runner. Registry readiness and a publish dry-run both select every package from its embedded release record, and CI requires their package sets to match so a newly added package cannot be omitted by a maintained allowlist. Cargo cannot completely verify dependent crates until their same-release dependencies exist in crates.io; Monochange plans those packages and publishes them in dependency order during the real release. Prepared release PRs are checked directly. Pull requests without a publishable release keep the job visible but skip the preflight explicitly.

This keeps code quality, behavior, documentation build health, feature-flag compatibility, and performance visibility aligned.

## Surfpool example security checks

`test:surfpool` builds every current example program and starts a fresh SDK-managed Surfpool instance for each one. Fresh instances are required because several Anchor-parity fixtures intentionally share a program ID. The harness has an explicit inventory assertion: adding an example crate or generated IDL without adding it to the test matrix fails immediately. A missing `.so` artifact also fails immediately; there are no best-effort skips.

Every program is deployed at its declared ID and is exercised with a malformed discriminator and an otherwise-valid instruction sent at a different deployment address. Every program also has an explicit expected entrypoint result: either a successful invocation or its exact expected `ProgramError` after dispatch (for example, `NotEnoughAccountKeys` or a documented custom error). Stateful examples additionally receive attacker-controlled readonly account metadata and must return their expected runtime guard error. Negative assertions use Surfpool simulation logs and a returned `InstructionError`, so an RPC, build, or deployment failure cannot satisfy them.

| Runtime guard                 | Surfpool adversarial case                                                                                                                                                          |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Discriminator/data validation | Every example rejects an unknown discriminator before any state transition.                                                                                                        |
| Program-ID binding            | Every example rejects its own ELF when it is deployed at an attacker-controlled address.                                                                                           |
| Required account boundary     | Every selected IDL entrypoint with required accounts rejects an omitted account list.                                                                                              |
| Stateful account metadata     | State-changing examples reject attacker-controlled readonly account sets with a runtime access error.                                                                              |
| Signer authorization          | `hello_solana_program` rejects an unsigned user and accepts the same user only when marked as a signer.                                                                            |
| Writable and alias checks     | `duplicate_mutable_accounts_program` rejects both a duplicate mutable alias and a non-writable account.                                                                            |
| Program address allowlist     | `declare_program` rejects an arbitrary account in place of its expected external program.                                                                                          |
| Owner constraint              | `system_accounts_program` rejects an account explicitly created with a non-System owner.                                                                                           |
| Sysvar address validation     | `sysvar_checks_program` rejects ordinary accounts substituted for Clock, Rent, and Stake History.                                                                                  |
| Authority-bound PDA resize    | `account_realloc_program` proves initialize/grow/shrink for its owner and rejects an unrelated signer, a forged typed account, and duplicate resize targets without data mutation. |
| Compact account lifecycle     | `compact_accounts_program` proves header-only creation, atomic patch growth and shrink, exact rent adjustment, and rollback for bounds and authority failures.                     |

The broader Pina examples also run their purpose-built Mollusk, LiteSVM, and Quasar tests in `test:program-e2e`; these cover PDA derivation, ownership, token-account, arithmetic/range, initialization, and unauthorized-mutation flows that need program-specific state setup. Surfpool complements those tests with a full, deployed SBF boundary check. It provides evidence that the listed invariants hold for the tested attacks; it is not a proof that no other attack exists.

### Previously-tracked audit finding

An earlier revision of the `security/06-duplicate-mutable-accounts/secure` fixture checked distinct, program-owned balances but did not require that its signer matches the source balance's stored owner, so an unrelated signer could debit a victim's logical balance into an attacker's destination. That authorization invariant is now enforced: `validate_source_authority` rejects a signer that does not match the source balance's stored owner with `LedgerError::UnauthorizedSigner`, and dedicated regression tests cover both the unauthorized-transfer rejection and the legitimate owner path.

## Performance regression policy

The `performance` workflow checks out the pull-request base in a sibling worktree. Four jobs run in parallel and update one sticky pull-request comment:

- Instruction CU from the ignored Surfpool suites, simulated against copied base and head ELF files.
- Static `pina profile --json` estimates and build sizes for every top-level example with a `bpf-entrypoint` feature.
- Hyperfine timings for representative CLI commands and median host timings from `crates/pina/tests/benchmarks.rs`.

The report names the latest release reachable from the base. When that release is the base commit, the displayed comparison is also release-to-head. Otherwise the tag is context and timings remain base-to-head so the workflow avoids a third build.

The inventories are discovered instead of maintained by hand. A new example or instruction appears as a new baseline with its current result. Future pull requests compare against it. A missing head profile, ELF, or instruction measurement fails CI.

The compute-unit policy is:

- warn when `total_cu` increases by at least `+250` CU and `+5.0%`
- fail when `total_cu` increases by at least `+500` CU and `+10.0%`
- decreases are positive and increases are negative
- smaller static increases remain visible but do not fail the threshold gate
- instruction runtime increases fail unless a reviewed absolute ceiling permits that total

Notes:

- instruction cases use real transaction simulation through Surfpool; static profiles complement them with whole-program coverage and binary sizes
- reviewed redesigns may record an absolute total in `approvedTotals`; the allowance applies only while the base is below that total, so later increases are still evaluated normally
- reviewed instruction redesigns use `runtimeApprovedTotals` with the same absolute-ceiling behavior
- update `scripts/compute-unit-policy.json` only for exclusions, thresholds, or reviewed ceilings; do not add new examples to an allowlist

Local reproduction:

```bash
devenv shell -- profile:cu:tracked
devenv shell -- report:cu:compare:main
```

The local comparison writes artifacts to `target/cu/`, including a Markdown summary, copied ELFs, and machine-readable JSON.

See [Compute-unit performance](./compute-unit-performance.md) for the exact PinaPod v0.2 migration results and approval rationale.

## Coverage

The `coverage` workflow runs focused coverage with `cargo llvm-cov` and publishes an LCOV artifact:

- Command: `coverage:all`
- Artifacts: `target/coverage/lcov.info` and `target/coverage/pina-test.info`
- Optional upload: Codecov (`fail_ci_if_error: true`, so a failed upload fails the job)

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
