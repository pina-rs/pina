# Audit remediation — implementation handoff

Companion to [`deep-audit-2026-09-18.md`](./deep-audit-2026-09-18.md) and [`deep-audit-2026-09-18-remediation-plan.md`](./deep-audit-2026-09-18-remediation-plan.md).

All work is committed on branches in `worktrees/` (gitignored). Nothing is merged to `main`. Every branch is clean with zero uncommitted changes. Base for all branches: `main` @ `f757a669`.

## Status

| Item                                | Branch                                   | Commit                 | What it does                                                                                                                                                        | How it was proven                                                                                                                                                                                |
| ----------------------------------- | ---------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **M3** cargo `[lib] name` traversal | `fix/validate-cargo-library-name`        | `5269db1b`             | Validates the name once at ingestion against `[A-Za-z0-9_-]`                                                                                                        | **Reproduced the exploit, then the fix.** Unfixed CLI wrote `escaped.json` _outside_ the project while reporting success; fixed CLI exits 1 with `InvalidLibraryName` and writes nothing         |
| **M2** IDL name code injection      | `fix/sanitize-idl-names-in-docs`         | `cc140ee6`             | Routes two raw-name doc lines through `render_doc`                                                                                                                  | **Proved the injection, then the fix.** Without it, `const _: () = loop {};` was emitted as live Rust from a crafted IDL; the test fails on unfixed code                                         |
| **M4** lint driver integrity        | `fix/lint-driver-download-integrity`     | `4066527f`             | Checksum verification via a `VerifiedDriver` newtype, 256 MiB cap, and the release workflow now publishes the standalone driver's `.sha256` (it previously did not) | 3 new tests: matching checksum installs, mismatch rejected, malformed rejected; `install` cannot accept unverified bytes by construction                                                         |
| **H1** vesting strands funds        | `fix/vesting-claim-and-cancel-economics` | `2fedb4b3`             | Clock-gated `Claim` with floor-rounded linear vesting and a real transfer; `Cancel` refunds and closes the vault                                                    | **SBF end-to-end.** Vault 1,000,000,000 → 600,000,000; beneficiary 0 → 400,000,000; cancel refunds the remainder and the vault is removed                                                        |
| **H2** staking repeatable no-op     | `fix/staking-reward-accounting`          | `af396812`             | Monotone `reward_index` + per-position checkpoint + `SetRewardIndex` drip + real payout                                                                             | **SBF end-to-end.** Drip pays exactly the accrued amount; a lower index is refused with `RewardIndexRegressed`; a second claim with no new accrual is refused with `NothingToClaim`              |
| **M1** `with_pda` silent weakening  | `feat/stored-bump-pda-naming`            | `139591a2`             | Renames the weak loader to `with_stored_bump_pda`; `with_pda` becomes a deprecated forwarding alias                                                                 | **Verified the warning fires at downstream call sites**, naming the difference and pointing at `with_checked_pda` — the compiler signal that was missing                                         |
| L1 PDA seed cap                     | `fix/pda-seed-count-cap`                 | `0b88f60f`             | 16-seed `#[pda]` is now a compile error instead of an unloadable program                                                                                            | Unit tests, trybuild fail pin, pass case at the limit                                                                                                                                            |
| L6 migrate trigger width            | `fix/migrate-trigger-width`              | `9429c2cb`             | Width-matches the reserved migrate trigger for u16/u32/u64 programs                                                                                                 | Per-width accept/reject tests; `cargo expand` on a u16 program confirmed the emitted helper                                                                                                      |
| L3 `emit` stack budget              | `fix/emit-stack-budget`                  | `f46950cf`             | Compile-time assert against `MAX_EVENT_RECORD_BYTES`                                                                                                                | Trybuild fail case for an oversized event, pass case under the budget                                                                                                                            |
| L2 unqualified `Address`            | `fix/pda-qualified-address`              | `cec256d2`             | Qualifies every `Address` in generated PDA code, plus an identity proof                                                                                             | Shadowing UI fail pin; expansion snapshots refreshed                                                                                                                                             |
| L11 hygiene                         | `chore/lockfile-and-tooling-hygiene`     | `8eef85ad`, `6881b30c` | Drops unused `tar`; one constant drives check and message; basename-sanitizes the `pina docs` topic; pins `packageManager`                                          | Full `pina_cli` suite passes, including new tests for both fixes                                                                                                                                 |
| L7 advisory gates                   | `ci/align-advisory-gates`                | `9a4a0967`             | `deny.toml` becomes the single policy; advisories checked with `--workspace`                                                                                        | Both gates exit 0; negated test (removing one ignore) makes the gate fail, proving the wiring is load-bearing                                                                                    |
| L8 publish hardening                | `ci/publish-pipeline-hardening`          | `b14ae5dd`             | Resolves tag + ref to one SHA and fails on mismatch; `--ignore-scripts`                                                                                             | Resolver shell logic executed against the real GitHub API across 6 cases. **Workflow itself not runtime-verified**                                                                               |
| L9 disclosure channel               | `docs/security-disclosure-channel`       | `23080ff8`             | Leads with GitHub private vulnerability reporting                                                                                                                   | Docs-only                                                                                                                                                                                        |
| L10 escrow nits                     | `fix/escrow-offer-validation`            | `c4322738`             | Rejects zero-amount offers; explicit maker writability                                                                                                              | Surfpool suite extended; the agent found the `amount_a == 0` case my plan missed                                                                                                                 |
| L5 checked discriminator write      | `feat/checked-discriminator-write`       | `32651d6b`             | `try_write_discriminator` reporting `DataTooShort`                                                                                                                  | 5 tests including short/exact/oversized buffers, agreement with the unchecked write, the generated-enum path, and the trait default body; host benchmark case added (34ns vs 38ns, within noise) |

## Corrections the implementation found in the audit

Two audit statements were wrong, and both are now corrected in the source documents:

1. **L11's "orphan lockfile entries" claim was false.** The advisory-affected versions are reachable through the dev/test stack. My verification used `<crate> v<version>`, but Cargo.lock dependency specs omit the `v` prefix, so the pattern could never match and "no dependents" was misread from a vacuous grep. Deleting the blocks makes `cargo metadata --locked` fail. Corrected in both audit docs; the surviving hygiene items were kept.

2. **L7's recommended fix was insufficient.** `cargo-deny`'s default graph contains only ~98 crates and excludes the entire dev/test stack, so adding `advisories` to the existing invocation would have reported nothing. The working fix scopes the advisories check with `--workspace`; bans/licenses/sources stay at default scope because `--workspace` makes them fail on dev-stack duplicates.

3. **L4 was already implemented** as `require_zeroed_before_close` on `main` (registered, wired, UI-tested). The plan listed it as pending without checking. Nothing to do.

## A note on one branch's history

`feat/checked-discriminator-write` was rewritten once. While the implementing agent was mid-item, I committed a partial version of the same change, reverting the expansion snapshots and the benchmark case as formatter noise. They were not noise: they are the real consequences of adding a trait method, and the agent's version is the complete one. The agent amended, reset to the base commit, and recommitted; its commit `32651d6b` supersedes mine, and the partial commit is unreachable.

The lesson is the same one the L11 correction taught from the other direction: I discarded a diff without proving it was inert. Verify before discarding, not only before claiming.

## What remains unverified

Honest limits, per the repo's own rule that an unproven job is a known failure:

- **Benchmark, coverage, and Kani tiers were not run** for any branch. Every PR needs `coverage:all` (100% patch gate) and the consolidated benchmark comment before merge. The two example branches change CU on real instruction paths (vesting `Claim`/`Cancel` gained accounts and CPIs; staking gained an instruction), so their benchmark deltas need review rather than acceptance.
- **L8's workflow cannot be executed locally.** Only its resolver shell logic was run. A dry-run dispatch is the real test.
- **IDL/client regeneration** was verified only for `vesting_program` and `staking_rewards_program` (the two whose surfaces changed). The generator rewrites tab/space style on every IDL it touches, so those diffs were pruned to the one file with real content changes; the drift job will confirm.
- **Surfpool suites for other examples** were not re-run after the shared-crate changes (only the vesting and staking suites were, plus escrow by the agent).

## Merge order

Independent, but two orderings matter:

1. **M1 before any release that cuts the `with_pda` deprecation.** The alias is a one-release affordance; merging M1 without a follow-up removal leaves the deprecated path indefinitely.
2. **M4 before the next release.** It changes the client and the workflow together: the CLI now requires a `.sha256` beside the driver asset, which only the updated workflow publishes. Merging the client half alone would break `pina lint` downloads.

Suggested sequence for the rest: the two example fixes (H1, H2) carry the largest user-visible value and the largest benchmark discussion, so start their CI early; then M3/M2 (smallest diffs, host-side security); then the macro batch; then the hygiene and CI items.

## Merge state and pull request

All sixteen branches were integrated onto one integration branch through sixteen merge commits, in an order chosen to keep the generated snapshot files (`tests/expand/*.expanded.rs`) consistent: hygiene and docs first, then the host-tooling fixes, then the examples, then the macro batch whose members share snapshots. This pull request carries that integration branch; merging it lands the whole remediation as one reviewable unit, and the per-branch history remains readable through the merge commits.

Integration-time verification, all on the integrated tree: every affected crate's suite was re-run after its merge; the `pina_root` expansion snapshots were regenerated once (`MACROTEST=overwrite`) when the three-way merge of independently generated snapshots drifted by one line; the `pina_cli` examples IDL snapshot for `escrow_program` was refreshed to carry the new `EmptyOffer` error (the one integration conflict that was a real drift rather than formatting); `security:deny` and `security:audit` pass; the merged `publish.yml` keeps both the L8 resolver job and the M4 checksum upload; and the full `cargo test --workspace --all-features` run is the gate this PR's checks re-run in CI.

Rollback point before integration: `backup/pre-merge-audit-2026-09-18`. The sixteen source branches and their worktrees still exist and can be deleted once this pull request merges.

What this pull request cannot prove by itself: the consolidated performance benchmark report, the coverage gate, and the Kani proofs all run as CI checks on it and must be read before merging — the benchmark gate in particular applies to the vesting, staking, and escrow changes, which grow real instruction paths (new accounts and CPIs), plus the M1 loader rename, which is `no_std` code compiled into every downstream program.

## Verification commands per branch

```sh
cd worktrees/<branch-dir>
devenv shell -- cargo test -p <affected-crate>     # per the table above
devenv shell -- lint:all
devenv shell -- coverage:all                        # before any merge
```

For the example branches, additionally build the SBF artifact and run the ignored surfpool suite:

```sh
cargo build-sbf --manifest-path examples/vesting_program/Cargo.toml \
  --sbf-out-dir target/deploy --features bpf-entrypoint
PINA_SBF_ARTIFACT=$PWD/target/deploy/vesting_program.so \
  devenv shell -- cargo test -p vesting-program-surfpool-tests -- --ignored
```

Each worktree needs `node_modules` symlinked from the repo root (`ln -sfn ../../node_modules node_modules`) or `pina_cli` tests fail with `Cannot find package 'codama'` — an environment artifact, not a code failure.
