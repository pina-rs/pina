# Deep security and performance audit

Audit date: 2026-09-22

Audited revision: `407545c0e4eab33f444b49a886ab264f79ae1530` on branch `fix/migration-auto-schema-enum`

Status: read-only audit. This report is the only tracked artifact added by the audit. No production system, registry, release, or deployed Solana program was changed.

## Executive summary

This review found one critical asset-loss path in a value-bearing example, several high-impact governance and release-pipeline defects, and one feature-dependent invariant bypass in the shipped `pina` runtime. It also found benchmark defects that can hide real performance regressions.

The most urgent issue is in `staking_rewards_program`: `Deposit` credits arbitrary stake without transferring stake tokens, while `Claim` transfers real reward tokens. The existing Surfpool journey proves the sequence end to end by depositing before any stake tokens are minted, funding the reward vault, and successfully claiming.

The core runtime otherwise held up well against the classic Solana attack classes checked here. Signer, owner, writable, PDA, token-program, aliasing, checked-arithmetic, and CPI-boundary tests passed. That is not a claim that the codebase is safe. The feature-dependent compact update bug, untested state-machine edges, release trust boundaries, CLI filesystem/network boundaries, and incomplete example economics remain material risks.

A second-session continuation (same date, same revision) extended the audit into the `pinapod` dependency itself — the crate defining the workspace's account wire format and unsafe zero-copy validation contracts — covering its runtime, derive codegen, and version-drift surface. It found no exploitable defect in the locked 0.4.1 by review or by 40,000-operation property fuzzing and a Miri pass, but it did find that the wire-format crate floats on a caret requirement (SEC-33) and that the compact realloc/patch length-agreement guard is compiled out of every release and SBF build (SEC-34).

Severity in this report means:

- Critical: direct loss of all or a material portion of assets is practical under the stated preconditions.
- High: asset or release integrity can be violated, governance authority can persist or be captured, or a security gate can be bypassed.
- Medium: meaningful integrity, confidentiality, availability, or operator-safety failure with additional preconditions.
- Low: defense-in-depth, liveness, documentation, or misuse risk without a demonstrated direct compromise.

Example findings are labelled as such. Their severity applies to a deployed adaptation, not to the `pina` framework crate by itself.

### Finding index

| ID     | Severity          | Area             | Summary                                                                                                            |
| ------ | ----------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------ |
| SEC-01 | Critical, example | Staking          | Rewards can be claimed against stake that was never transferred or escrowed.                                       |
| SEC-02 | High              | Core runtime     | `UpdateResizableAccount` bypasses migration-envelope checks when `validation` is disabled.                         |
| SEC-03 | High              | Release          | Manual `release-pr` dispatch executes arbitrary-ref code with release-capable credentials.                         |
| SEC-04 | High              | Release          | Two tag-producing release targets cause a dry-run mismatch and partial release side effects.                       |
| SEC-05 | High              | Release          | Mutable draft assets are downloaded again after attestation without digest binding.                                |
| SEC-06 | High              | Release          | The tag gate waits for the globally newest publish run, not the run it dispatched.                                 |
| SEC-07 | High              | CI               | Semver acknowledgement turns any tool, build, or network failure into a passing gate.                              |
| SEC-08 | High, example     | Multisig         | A removed member retains spending-limit authority.                                                                 |
| SEC-09 | High, example     | Multisig         | Approved config proposals execute after expiry.                                                                    |
| SEC-10 | High, example     | Multisig         | Timelock reductions retroactively release older approved vault proposals.                                          |
| SEC-11 | High, example     | Multisig         | The first signer can capture the global config PDA.                                                                |
| SEC-12 | Medium            | CLI deployment   | An exit-zero deploy process records publication without proving on-chain deployment.                               |
| SEC-13 | Medium            | CLI deployment   | Explicit loopback RPC URLs bypass remote/mainnet safeguards.                                                       |
| SEC-14 | Medium            | CLI ledger       | Reconciliation reads before locking and can overwrite a newer deployment receipt.                                  |
| SEC-15 | Medium            | CLI network      | IDL URL policy is not revalidated across redirects.                                                                |
| SEC-16 | Medium            | CLI secrets      | Query credentials in import URLs are printed and persisted.                                                        |
| SEC-17 | Medium            | CLI filesystem   | IDL and verification outputs follow symlinked ancestor directories.                                                |
| SEC-18 | Medium            | CLI verification | Exported verification transactions are accepted by encoding and length alone.                                      |
| SEC-19 | Medium            | CLI availability | Official-client IDL extraction has output caps but no cancellation or timeout.                                     |
| SEC-20 | Medium, example   | Multisig         | The executor chooses the recipient of close and shrink rent refunds.                                               |
| SEC-21 | Medium, example   | Multisig         | Valid TTL/timelock settings can make every proposal impossible to execute.                                         |
| SEC-22 | Medium, example   | Multisig         | Expired and stale nonterminal proposals cannot be closed.                                                          |
| SEC-23 | Medium            | Supply chain     | Dependency policy misses a shipped CLI graph and standalone lockfiles.                                             |
| SEC-24 | Medium            | Deployment       | Manual Pages dispatch can publish an arbitrary selected branch.                                                    |
| SEC-25 | Medium            | Release          | Release binaries depend on moving toolchains and unversioned build tools.                                          |
| SEC-26 | High, example     | Staking          | Permissionless first initialization captures the singleton pool administrator.                                     |
| SEC-27 | High, example     | Staking          | Reward-index updates can create undercollateralized or unrepresentable liabilities.                                |
| SEC-28 | High, example     | Vesting          | Cancellation returns vested but unclaimed tokens to the administrator.                                             |
| SEC-29 | High, example     | Vesting          | A schedule becomes active before its promised allocation is funded.                                                |
| SEC-30 | High, example     | Token-2022       | Initializers accept configurations that every value-exit path later rejects.                                       |
| SEC-31 | High, example     | Escrow           | Offers have no cancellation or expiry path.                                                                        |
| SEC-32 | High, example     | Oracle           | Authority rotation does not rotate the price publisher.                                                            |
| SEC-33 | Medium            | Supply chain     | The account wire-format crate `pinapod` is a caret requirement; any lockfile refresh silently drifts it.           |
| SEC-34 | Low–Medium        | Core runtime     | Compact realloc/patch length agreement is enforced only by compiled-out debug assertions.                          |
| SEC-35 | Low, latent       | Dependency       | pinapod 0.4.1 wincode writers emit a corrupt stored prefix verbatim; the feature is not enabled in this workspace. |

Performance findings are indexed separately as `PERF-*` below.

## Scope and method

The review covered:

- the `pina` runtime and macro-generated account, migration, validation, CPI, and account-list paths;
- the `pinapod` 0.4.1 dependency itself — runtime pods, representation contracts, derive codegen, and the 0.4.1-to-0.4.3 drift surface (continuation session; see the continuation section);
- the CLI deployment, verification, IDL import/export, filesystem publication, and migration-ledger boundaries;
- value-bearing examples, with a deeper pass over staking, vesting, escrow, multisig, and the property-oracle example;
- release, CI, dependency policy, fuzz/Kani routing, and package assembly;
- core and generator performance hot paths plus the benchmark gates themselves.

The method combined manual data-flow and state-machine review, feature-matrix review, repository-wide pattern searches, focused existing tests, real SBF/Surfpool journeys, safe local host-boundary fixtures, dependency graph inspection, and native performance measurements.

This audit did not verify any deployed program ID, upgrade authority, release environment protection rule, GitHub tag protection rule, registry account, live RPC state, or built artifact hash. Those require deployment-specific evidence and remain out of scope.

## Security findings

### SEC-01: rewards can be claimed without staking tokens

Severity: Critical for a funded deployment; example/template finding.

Locations:

- `examples/staking_rewards_program/src/lib.rs:162-170`
- `examples/staking_rewards_program/src/lib.rs:378-462`
- `examples/staking_rewards_program/src/lib.rs:465-540`
- `examples/staking_rewards_program/src/lib.rs:543-647`
- `examples/staking_rewards_program/tests/surfpool/src/lib.rs:816-895`

Mechanism:

`DepositAccounts` and `WithdrawAccounts` do not contain the pool stake vault. Both handlers mutate `position_state.staked_amount` and `pool_state.total_staked`, but neither transfers stake tokens. `Claim`, in contrast, transfers real reward tokens from the canonical reward vault using the pool PDA. Any signer with a position can therefore manufacture stake bookkeeping and receive funded rewards.

The README discloses that stake-vault transfers are out of scope, but it also says rewards accrue and release correctly. The code-level module comment still says claim does not transfer tokens even though claim now does. These contradictory contracts make adaptation risk higher, not lower.

Executed reproduction:

```sh
devenv shell -- pina test \
  --project examples/staking_rewards_program \
  --filter rewards_accrue_once_per_index_and_release
```

The test passed. Its sequence calls `Deposit` before minting stake tokens, later mints stake to the user ATA rather than a pool vault, funds only the reward vault, advances the index, and successfully claims rewards. No stake was ever custodied by the program.

Fix:

1. Add a mutable canonical `stake_vault` to deposit and withdraw.
2. Validate it as the ATA for `(pool_state, stake_mint, token_program)`.
3. Validate the user ATA for `(user, stake_mint, token_program)`.
4. Deposit with `TransferChecked` from user ATA to stake vault before committing accounting. With the current extension policy, reject transfer-affecting Token-2022 extensions. If those extensions are supported later, record the observed vault delta instead of the requested amount.
5. Withdraw with `TransferChecked` from the stake vault to the user ATA, signed by the pool PDA.
6. Enforce `stake_vault.amount >= total_staked`; prefer equality if unsolicited donations are rejected or separately tracked.

Acceptance criteria:

- A user with a zero stake-token balance cannot increase recorded stake.
- Successful deposit changes user balance, vault balance, position stake, and pool total by the same amount.
- Successful withdrawal returns tokens and decrements the same four quantities consistently.
- A failed token CPI leaves every balance and state byte unchanged.
- Claim cannot accrue against zero custody.

### SEC-02: no-validation compact updates bypass migration state

Severity: High for programs using migrations and `compact,account-resize,derive` without `validation`.

Locations:

- `crates/pina/src/cpi.rs:1644-1683`
- `crates/pina_macros/src/account.rs:535-567`
- `crates/pina/Cargo.toml:32-40`

Mechanism:

The generated account-level `T::update` contract checks `require_current_migration_envelope` and writes the current migration marker. `UpdateResizableAccount` uses that contract only with the `validation` feature. Without it, the helper calls raw `PinaPodPatch::update`, writes only the discriminator, and never checks or advances the migration envelope. The feature combination is supported because `compact` and `account-resize` do not imply `validation`.

A stale account whose old and current compact layouts are structurally compatible can therefore be patched without returning `MigrationRequired`. This violates the migration state machine and can reinterpret old semantic state as current state.

Reproduction to add:

1. Define compact v0 and v1 layouts with the same physical shape and different semantic meaning.
2. Initialize bytes with the v0 migration marker.
3. Compile the test with `--no-default-features --features compact,account-resize,derive` and without `validation`.
4. Invoke `UpdateResizableAccount` with a same-size patch.
5. Current behavior applies the patch and leaves a stale marker. Correct behavior returns `PinaProgramError::MigrationRequired` and preserves all bytes.

Fix:

Always route updates through the generated account-level update contract. Do not solve the duplicate-validation performance issue by bypassing that contract. Introduce an internal prepared-patch capability that carries the validated layout and target size across realloc, then lets `T::update_prepared` enforce migration and discriminator invariants exactly once.

Acceptance criteria:

- The feature-matrix regression above fails closed.
- Current-version compact accounts still update under every supported feature combination.
- Stale and future migration markers return their specific stable errors.
- Rejected updates preserve account bytes and lamports.

### SEC-03: arbitrary-ref release dispatch crosses a credential boundary

Severity: High.

Locations:

- `.github/workflows/release-pr.yml:3-32`
- `.github/workflows/release-pr.yml:84-145`
- `.github/workflows/release-pr.yml:155-233`
- `monochange.toml:64-75`
- `.github/workflows/publish.yml:50-121`

Mechanism:

`release-pr` supports `workflow_dispatch` on a selected ref. The workflow checks out that ref, loads a ref-controlled local setup action and release configuration, configures `RELEASE_TOKEN`, and runs `monochange run release`. The later tag job requires a release-record-shaped HEAD but never proves that HEAD is protected `main` or an ancestor of it. `publish.yml` proves tag/ref equality, not trusted-branch ancestry.

A compromised or malicious write collaborator can place a release-shaped commit and command on a branch, dispatch the workflow for that branch, and execute ref-controlled code in a release-capable credential context. Environment and tag protection settings could reduce impact, but they were not verifiable locally.

Safe reproduction:

In a disposable fork, add a harmless branch-only release command that prints only whether `GH_TOKEN` is set, then dispatch `release-pr.yml --ref <branch>`. Do not print the token. The command runs from the selected ref with the credential present.

Fix:

- Remove manual dispatch, or hard-require `github.ref == 'refs/heads/main'` for every mutating path.
- Split planning into a read-only, no-secret job.
- Mint a short-lived GitHub App token only in a protected release environment after proving the selected SHA is an ancestor of `origin/main` and matches the embedded release record.
- Never run ref-controlled local actions, scripts, or Monochange command steps after exposing the release credential.
- Protect both `v*` and `abi/v*` tag namespaces.

Acceptance criteria:

- A branch-only commit cannot reach any step with release credentials or write permissions.
- The tag job rejects a valid-looking release record not reachable from protected `main`.
- The trusted SHA is recorded and checked in every downstream publish job.

### SEC-04: two release targets cause partial release state

Severity: High release integrity and availability.

Locations:

- `monochange.toml:192-235`
- `.github/workflows/release-pr.yml:131-145`
- `.github/workflows/release-pr.yml:198-216`
- `.github/workflows/release-pr.yml:219-264`
- `.github/workflows/publish.yml:52-54`

Mechanism:

The current release record contains two tag-producing targets. The preview emits `abi/v0.20.0` before `v0.20.0`. Both dry-run selectors take the first tag with `head -1`, but `publish.yml` skips any tag that does not start with `v`. Later, tags and draft releases are created for all targets, and only then does the dispatch loop reject the namespaced ABI tag. The workflow can therefore create partial release side effects and stop before publishing the core release.

Executed static reproduction:

```sh
devenv shell -- monochange --log-level error preview --format json \
  | jq -r '.release_targets[] | select(.tag == true) | .tag_name'
```

Observed order:

```text
abi/v0.20.0
v0.20.0
```

Fix:

Validate the complete release-target set and exact core/ABI mapping before any tag, branch, or draft-release mutation. Select the core target by stable ID, not list order. Either publish registries once from the core tag and treat the ABI tag as metadata-only, or create a separately supported ABI workflow.

Acceptance criteria:

- Tests cover zero, one, and two targets in both iteration orders.
- Unsupported target names fail before any remote mutation.
- Dry-run, tag, draft, and publish stages use the same explicitly selected target.

### SEC-05: attested bytes are not the bytes packaged for npm

Severity: High.

Locations:

- `.github/workflows/publish.yml:243-257`
- `.github/workflows/publish.yml:336-383`
- `.github/workflows/publish.yml:468-477`
- `scripts/npm/package-cli.mjs:149-169`
- `scripts/npm/package-cli.mjs:197-243`

Mechanism:

Matrix jobs upload assets to a mutable draft release. The attestation job downloads, hashes, attests, and verifies those assets. After environment approval, the privileged publisher downloads the draft assets again and packages them without comparing them to the attested bytes or manifest. Any actor or token with release-asset write access can replace a correctly named archive during the approval window.

The package script checks expected names and archive members, not a trusted checksum or provenance statement. Its fixture accepts arbitrary bytes under the expected binary name.

Fix:

Use immutable current-run workflow artifacts for inter-job transport. Build one exact filename/digest manifest, attest those local bytes, and package those same bytes. Upload to the GitHub release only after successful registry assembly. At minimum, pass the manifest as an immutable workflow artifact and re-run digest plus `gh attestation verify` bound to repository, workflow, ref, and SHA immediately before packaging.

Acceptance criteria:

- Altering one byte between attestation and package assembly fails before registry credentials are used.
- Extra, missing, and duplicate archives fail closed.
- The published package records the exact attested digest for its embedded binary.

### SEC-06: dry-run correlation can approve the wrong release

Severity: High.

Location: `.github/workflows/release-pr.yml:190-216`.

Mechanism:

After dispatching `publish.yml`, the release workflow sleeps and selects the globally newest `workflow_dispatch` run. It does not filter by the unique dry-run branch, expected SHA, tag input, actor, or dispatch time. A concurrent successful run can be watched while the intended exact-tree dry run is pending or failing. Tag creation then proceeds.

Fix:

Create a unique ref containing the caller run ID and attempt, record the expected SHA before dispatch, poll runs filtered by that ref, and require exactly one candidate with the expected `headSha`, event, inputs, and creation time. Watch that exact database ID. Delete the temporary ref in an always-run cleanup step.

Acceptance criteria:

- A fixture with two interleaved runs selects only the expected SHA/ref.
- Zero or multiple matching runs fail closed.
- Tagging cannot begin until the correlated run succeeds.

### SEC-07: semver gate fails open on operational errors

Severity: High.

Location: `.github/workflows/semver.yml:62-98`.

Mechanism:

The semver command uses `continue-on-error`. The evaluator treats every nonzero result as a breaking-change finding and passes it if the PR title contains `!` or any changeset in the repository is major/breaking. Compiler failures, registry outages, missing baselines, malformed output, and tool crashes are therefore indistinguishable from a valid incompatibility report. The current tree already contains historical major changesets, so the acknowledgement predicate is not even limited to the PR.

Fix:

- Derive acknowledgements only from changesets added by `base...HEAD` and map them to the affected package.
- Capture a machine-readable semver report and distinguish completed incompatibility analysis from tool failure.
- If an acknowledged break exists, rerun in explicit-major mode. The second command must still complete successfully; arbitrary failure never passes.

Acceptance criteria:

- API-break fixture plus matching PR changeset passes.
- Compile, registry, baseline, and malformed-report fixtures fail regardless of acknowledgement.
- Historical changesets outside the PR cannot satisfy the gate.

### SEC-08: removed multisig members retain allowances

Severity: High for a funded deployment; example/template finding.

Locations:

- `examples/multisig_program/src/lib.rs:1683-1693`
- `examples/multisig_program/src/lib.rs:2922-2969`

Mechanism:

Member removal changes the current roster. `SpendingLimitUse` loads the multisig but discards the snapshot, then authorizes only against the static member list stored in the spending-limit account. A removed member can keep spending and, for recurring limits, regain the allowance after each reset.

Reproduction to add:

Adapt `spending_limit_use_resets_the_period_and_moves_sol`: remove member A from the current multisig fixture while leaving A in the limit roster, then submit `SpendingLimitUse` signed by A. Current behavior transfers value. Correct behavior rejects and leaves the vault, destination, `remaining_amount`, and `last_reset` unchanged.

Fix:

Retain the loaded snapshot and require current membership plus the intended permission at use time. Also reject non-current members when adding a limit. If non-member delegates are a product requirement, model them as a distinct revocable authority rather than reusing a stale member record.

Acceptance criteria:

- Removal revokes one-time and recurring allowances immediately.
- Re-adding a key does not accidentally revive an old allowance unless governance explicitly refreshes it.

### SEC-09: expired config proposals still execute

Severity: High for a funded deployment; example/template finding.

Locations:

- `examples/multisig_program/src/lib.rs:2806-2875`
- comparison: `examples/multisig_program/src/lib.rs:2564-2568`

Mechanism:

`ConfigExecute` checks status, stale index, and timelock, but never checks `proposal.expires_at`. `VaultExecute` performs the missing check. An approved config proposal can therefore change membership, threshold, authority, timelock, or spending limits after its declared lifetime.

Fix:

Immediately after reading `now`, reject `is_expired(proposal.expires_at, now)` before parsing or applying actions.

Acceptance criteria:

- Test at `expires_at - 1`, the exact boundary, and `expires_at + 1` according to the documented inclusive policy.
- A rejected expired proposal leaves multisig bytes, proposal bytes, auxiliary accounts, and lamports unchanged.

### SEC-10: timelock changes affect already-approved vault transfers

Severity: High for a funded deployment; example/template finding.

Locations:

- `examples/multisig_program/src/lib.rs:1699-1701`
- `examples/multisig_program/src/lib.rs:2551-2563`
- `examples/multisig_program/src/lib.rs:2769-2785`

Mechanism:

A timelock change marks prior proposals stale, but `VaultExecute` deliberately ignores the stale index and compares approval time against the current multisig timelock. Governance can approve a vault transfer under a long delay, then approve a config change reducing the delay to zero, making the older transfer immediately executable.

Fix:

Either apply the stale-index guard to vault proposals or store immutable `execute_after` when a proposal first reaches approval. The latter preserves approved consent across unrelated roster changes without letting mutable future policy shorten its delay.

Acceptance criteria:

- Approve a vault proposal under a 3,600-second delay, execute a later `SetTimeLock(0)` proposal, and prove the older transfer still cannot execute early.
- Revocation and re-approval compute the new immutable execution time exactly once.

### SEC-11: first signer captures global multisig configuration

Severity: High during bootstrap; example/template finding.

Location: `examples/multisig_program/src/lib.rs:1864-1913`.

Mechanism:

`ConfigInitialize` accepts any signer for the one fixed `ProgramConfig` PDA and stores that signer as authority. A first caller can permanently occupy the singleton, choose the treasury and creation fee, and prevent the intended initializer. `ConfigUpdate` has no authority-transfer path.

Fix:

Bind one-time initialization to a compile-time bootstrap address, an upgrade-authority proof, or an atomic deployment/init ceremony. Separate the bootstrap initializer from a rotatable stored authority.

Acceptance criteria:

- An unrelated funded signer cannot create the PDA.
- The designated initializer succeeds, a second initialization fails, and authenticated two-step authority transfer is covered.

### SEC-12: process success is treated as deployment proof

Severity: Medium.

Locations:

- `crates/pina_cli/src/commands.rs:1189-1247`
- `crates/pina_cli/src/deploy.rs:844-876`
- `crates/pina_cli/src/migrations/ledger.rs:144-190`
- `crates/pina_cli/tests/deploy_command.rs:543-562`

Mechanism:

After the deploy subprocess exits successfully, the CLI revalidates only local inputs and records a publication receipt. It does not query finalized ProgramData, compare executable bytes, check loader ownership, or verify the upgrade authority. The existing test intentionally uses a fake command containing only `exit 0` and expects the receipt to be created.

Executed reproduction:

```sh
devenv shell -- cargo test -p pina_cli \
  --test deploy_command \
  remote_deployments_record_and_retain_crash_safe_migration_state \
  -- --exact
```

The test passed. A separate exit-zero `--remote-command` fixture also produced a publication receipt without making an RPC request.

Fix:

Before clearing `pending`, query the exact target RPC at finalized commitment. Verify program address, supported loader owner, ProgramData relationship, executable hash using one documented canonicalization, and expected upgrade authority. A custom command must undergo the same independent readback; its exit code is never sufficient proof.

Acceptance criteria:

- No-op success, wrong hash, wrong loader, wrong authority, unfinalized state, and RPC timeout all retain `pending`.
- Only a finalized exact match creates the receipt.

### SEC-13: loopback URLs bypass remote deployment policy

Severity: Medium operator-safety boundary.

Locations:

- `crates/pina_cli/src/deploy.rs:898-928`
- `crates/pina_cli/src/deploy.rs:1007-1071`
- `crates/pina_cli/src/deploy.rs:1727-1747`
- `crates/pina_cli/src/commands.rs:1191-1199`

Mechanism:

Every explicit loopback host is classified as local. Local targets skip `--allow-mainnet`, interactive confirmation, and default publication receipts. A loopback endpoint can be an SSH tunnel, reverse proxy, or relay to mainnet; network location does not prove cluster identity.

Executed reproduction:

An explicit `http://127.0.0.1:8899` cluster plus a fake remote command executed noninteractively without `--yes` or `--allow-mainnet` and printed `Deployment complete`.

Fix:

Only the named, controlled local-validator mode should be local. Treat every explicit URL as unknown until the CLI queries and pins its genesis hash. If an escape hatch remains, call it `--trust-local-rpc` and make the trust decision explicit.

Acceptance criteria:

- Explicit `localhost`, `127.0.0.1`, `127.1`, and `::1` URLs require unknown/mainnet safeguards.
- A recognized local validator genesis retains the local workflow.
- Verification uses the same classification rule.

### SEC-14: reconcile can overwrite a newer ledger update

Severity: Medium integrity failure.

Location: `crates/pina_cli/src/migrations/ledger.rs:222-274`.

Mechanism:

Reconciliation loads and clones the ledger before it acquires the exclusive mutation lock. If another process records a successful deployment while reconciliation is waiting, reconciliation later writes its stale copy and can replace the new receipt with an abandoned record or lose unrelated entries.

Executed reproduction:

The audit held `migrations/.lock`, started `pina migrations reconcile --abandon` so it read then blocked, converted the pending entry into a successful receipt while holding the lock, and released it. Reconciliation overwrote the success with `abandoned: true`.

Fix:

Acquire the migration lock immediately after project discovery and before loading the manifest or ledger. Hold it through validation and atomic publication. The report-only reconciliation mode should also lock for a consistent snapshot.

Acceptance criteria:

- A barrier-based concurrent test preserves the newer receipt.
- Concurrent begin, record, make, and reconcile operations preserve the complete hash chain.

### SEC-15: IDL redirect destinations escape URL policy

Severity: Medium.

Location: `crates/pina_cli/src/import_idl.rs:304-336`.

Mechanism:

The importer validates only the initial URL, then uses the HTTP client's default redirect behavior. An allowed loopback HTTP URL can redirect to non-loopback plaintext HTTP. An HTTPS URL can also downgrade to HTTP unless the client is configured otherwise.

Executed reproduction:

A loopback listener returned `302 Location: http://0.0.0.0:<port>/idl.json`. `pina import` followed it and imported the destination, while supplying that destination directly was rejected by policy.

Fix:

For HTTPS, enforce HTTPS on every hop. For loopback HTTP, disable redirects or manually validate each `Location`, requiring every hop and final URI to remain exact loopback HTTP. Cap hops.

Acceptance criteria:

- HTTPS-to-HTTP and loopback-to-nonloopback redirects fail before the destination is contacted.
- Same-policy redirects work; loops and excessive chains fail deterministically.

### SEC-16: import URL secrets are logged and committed to generated output

Severity: Medium confidentiality failure.

Locations:

- `crates/pina_cli/src/import_idl.rs:45-50`
- `crates/pina_cli/src/import_idl.rs:175-205`
- `crates/pina_cli/src/commands.rs:2619-2625`

Mechanism:

The raw import URL is printed, stored as provenance, and written to the generated README. Query parameters are accepted, so pre-signed tokens or API keys leak into stdout, logs, generated files, and potentially version control.

Executed reproduction:

Importing a loopback URL with `?token=PINA_AUDIT_SECRET_84f17` printed the sentinel and wrote it into README line 14.

Fix:

Separate request URL from display/provenance URL. Reject userinfo. If query-bearing signed URLs remain supported, strip query and fragment from every display, diagnostic, `Debug`, and persisted field. Use content SHA-256 as durable provenance.

Acceptance criteria:

- A sentinel never appears in stdout, stderr, errors, debug output, README, or manifests.
- The actual HTTP request still receives the original query when supported.

### SEC-17: output publication follows symlinked ancestors

Severity: Medium filesystem-boundary failure.

Locations:

- `crates/pina_cli/src/idl_metadata.rs:622-640`
- `crates/pina_cli/src/idl_metadata.rs:856-885`
- `crates/pina_cli/src/idl_command.rs:259-270`
- `crates/pina_cli/src/verification.rs:1440-1465`

Mechanism:

The path guard checks the final output path but not ancestor components. Atomic output beneath a symlinked directory is published outside the apparent tree. This affects IDL fetch/export and verification export.

Executed reproduction:

The audit created `inside/link -> outside`, exported to `inside/link/fetched.json`, and observed the same inode at `outside/fetched.json`.

Fix:

Centralize secure output publication. At minimum reject symlink or reparse-point ancestors. Close the check/open race with descriptor-relative `openat`/`renameat`, `O_NOFOLLOW`, and parent-directory fsync; implement the Windows equivalent.

Acceptance criteria:

- Existing and missing destinations below a symlinked ancestor are rejected and the external target is untouched.
- Ancestor replacement races and Windows reparse points are tested.

### SEC-18: verification transaction output is not semantically validated

Severity: Medium integrity risk at a tool boundary.

Location: `crates/pina_cli/src/verification.rs:1393-1437`.

Mechanism:

The CLI accepts the final base58/base64 line from the external official client when decoded bytes are at least 64 bytes. It does not deserialize a Solana transaction or verify the program, instruction, accounts, uploader, program ID, repository, revision, or absence of extra instructions. Current tests accept arbitrary repeated byte arrays.

Fix:

Deserialize the supported versioned transaction, require the exact expected verification instruction and accounts, reject unrecognized or extra instructions, and reserialize byte-for-byte. Optionally simulate the unsigned transaction against the selected RPC.

Acceptance criteria:

- Existing arbitrary-byte fixtures fail.
- A canonical fixture passes.
- Mutating one account, field, program ID, or adding one instruction fails.

### SEC-19: IDL subprocess can hang after exceeding output limits

Severity: Medium availability.

Location: `crates/pina_cli/src/idl_metadata.rs:669-751`.

Mechanism:

Reader threads stop retaining bytes after the configured limit but continue draining until EOF. The parent waits without a wall-clock or idle timeout and never cancels the child on overflow. A child that writes past the limit and then sleeps can hang the CLI indefinitely.

Bounded reproduction:

Run the official-client path with a fixture executable that writes more than the cap and then blocks. An outer process timeout expires instead of the CLI returning its own bounded error.

Fix:

Share cancellation between readers and parent. On the first overflow, terminate and reap the child. Add separate wall-clock and idle-output timeouts and distinguish `ClientOutputTooLarge` from `ClientTimedOut`.

Acceptance criteria:

- An over-limit sleeping child returns the size error within a fixed deadline and leaves no live child.
- A silent child returns a timeout; a valid near-limit response succeeds.

### SEC-20: executor selects rent-refund destination

Severity: Medium for a funded deployment; example/template finding.

Locations:

- `examples/multisig_program/src/lib.rs:1782-1797`
- `examples/multisig_program/src/lib.rs:2762-2792`
- `crates/pina/src/cpi.rs:1841-1856`

Mechanism:

`ConfigExecute` accepts a signed caller-selected `rent_payer`. Spending-limit removal closes the entire account to it, and multisig shrink refunds excess rent to it. The approved action stream does not commit the recipient, so an executor can direct rent to themselves.

Fix:

Separate growth funding from refunds. Bind refunds to the stored rent collector, vault, or per-account original payer. If governance wants a different recipient, commit it in the proposal payload.

Acceptance criteria:

- Removing a funded spending-limit account credits only the deterministic collector.
- Executor-controlled accounts cannot receive shrink or close proceeds unless explicitly approved.

### SEC-21: valid TTL and timelock values can make execution impossible

Severity: Medium liveness; example/template finding.

Locations:

- `examples/multisig_program/src/lib.rs:1844-1858`
- `examples/multisig_program/src/lib.rs:1954-1975`
- `examples/multisig_program/src/lib.rs:2117-2140`

Mechanism:

TTL and timelock are bounded independently. A nonzero TTL shorter than or equal to the timelock can expire a proposal before execution becomes possible, because expiry starts at creation while the delay starts at approval.

Fix:

At minimum reject nonzero `ttl <= timelock`. A cleaner model separates voting deadline from an immutable post-approval execution deadline.

Acceptance criteria:

- Creation and config mutation reject impossible combinations.
- Boundary tests prove every accepted autonomous configuration has a reachable execution interval.

### SEC-22: expired and stale proposals strand rent

Severity: Medium liveness; example/template finding.

Locations:

- `examples/multisig_program/src/lib.rs:1633-1638`
- `examples/multisig_program/src/lib.rs:3070-3090`

Mechanism:

`ProposalClose` has no Clock account and accepts only executed, rejected, or cancelled states. Expired active/approved proposals and stale nonterminal proposals can neither progress nor close, permanently locking rent.

Fix:

Pass Clock and allow permissionless close when terminal, expired, or stale. Keep the refund address fixed by multisig state.

Acceptance criteria:

- Expired and stale fixtures close and refund only the configured collector.
- Exact expiry/stale boundaries and spoofed collector accounts are covered.

### SEC-23: dependency policy does not cover every executed graph

Severity: Medium supply-chain gap.

Locations:

- `devenv.nix:1290-1315`
- `devenv.nix:1347-1354`
- `.github/workflows/ci.yml:61-119`
- `crates/pina_fuzz/Cargo.lock`
- `crates/pina_fuzz/fuzz/Cargo.lock`
- `benchmarks/framework-comparison/**/Cargo.lock`

Mechanism:

The normal dependency gate passes because bans/licenses/sources are not run across the complete workspace graph. A direct workspace-wide license/source check rejects `monochange_snapshot`'s `Unlicense`, reached as a normal dependency of shipped `pina_cli`. Advisory checking receives only the root lockfile, while standalone fuzz and benchmark workspaces have committed locks and are compiled or executed by CI. Changes to `deny.toml` also do not trigger either dependency job.

Executed reproduction:

- `devenv shell -- security:deny` passed.
- `devenv shell -- cargo-deny --workspace check licenses sources` failed on the `pina_cli -> monochange_snapshot` path.
- `cargo tree --locked -p pina_cli -i monochange_snapshot --edges normal,build` confirmed normal reachability.

Fix:

Discover every committed Cargo workspace/lockfile that CI builds. Run advisory, source, license, and ban policy per graph, with exact documented exceptions where necessary. Add `deny.toml` to the dependency-policy path class and add an inventory test so new standalone manifests cannot escape the sweep.

Acceptance criteria:

- The shipped CLI graph is checked.
- Every CI-executed lockfile is inventoried once.
- Adding a standalone workspace without policy coverage fails CI.

### SEC-24: Pages can deploy from a selected branch

Severity: Medium official-documentation integrity.

Location: `.github/workflows/docs-pages.yml:14-76`.

Mechanism:

Manual dispatch checks only repository owner/tag shape, then checks out and deploys the selected ref using Pages/OIDC. It does not require protected `main` or prove release-tag ancestry.

Fix:

Require `refs/heads/main` for manual dispatch. For release events, prove the tag commit is on protected main and matches the release record. Enforce the same branch restriction and reviewers in the `github-pages` environment.

Acceptance criteria:

- A branch-only documentation change cannot reach deploy.
- Main and trusted release tags continue to publish.

### SEC-25: release builds are not reproducible by configuration

Severity: Medium supply-chain integrity.

Locations:

- `.github/workflows/publish.yml:158-180`
- `.github/workflows/publish.yml:211-242`
- `.github/workflows/publish.yml:436-439`

Mechanism:

Release jobs use moving `*-latest` runners, explicitly install floating `stable`, install `cross` without a version, and build the lint driver without `--locked` or `--frozen`. Retrying the same tag later can produce different binaries even though each individual build is honestly attested.

Fix:

Pin one exact compiler/toolchain, `cross` version, and practical runner/Xcode image set; use locked/frozen resolution; normalize archive metadata; record all tool and source digests in provenance. Add same-source double-build comparisons where the platform permits deterministic output.

Acceptance criteria:

- The release manifest names exact tool versions.
- Lockfile mutation is impossible during build.
- Retried builds either match hashes or emit a documented, investigated variance report.

### SEC-26: first caller captures the singleton staking pool

Severity: High during bootstrap; example/template finding.

Locations:

- `examples/staking_rewards_program/src/lib.rs:72-82`
- `examples/staking_rewards_program/src/lib.rs:231-273`

Mechanism:

Pool seeds include only the mint pair, while initialization accepts any signer and stores that signer as administrator. The first caller can occupy the canonical singleton, control reward-index updates, and block the intended administrator.

Fix:

Either require a configured factory/governance/bootstrap signer, prove the intended mint authority, or include creator/administrator in the seeds and explicitly define pools as permissionless namespaces.

Acceptance criteria:

- An unauthorized first caller cannot occupy the official singleton.
- Authorization failure creates no state or vaults.

### SEC-27: reward index can create liabilities the vault cannot honor

Severity: High for a funded deployment; example/template finding.

Locations:

- `examples/staking_rewards_program/src/lib.rs:197-204`
- `examples/staking_rewards_program/src/lib.rs:651-680`
- `examples/staking_rewards_program/src/lib.rs:118-130`

Mechanism:

`SetRewardIndex` accepts any monotone `u64` index without receiving the reward vault, tracking aggregate debt, or checking reserves. With insufficient funding, otherwise equal users are paid according to claim order. A sufficiently large index can also make per-position accrual unrepresentable and freeze deposit, withdrawal, and claim for affected positions.

Fix:

Track funded, reserved, accrued, and paid rewards. Compute aggregate liability increments in `u128`, reject unrepresentable results, and require canonical vault reserves to cover all outstanding liabilities. A separate `FundRewards` instruction makes the accounting boundary explicit.

Acceptance criteria:

- Equal entitlements are not claim-order dependent.
- Index updates cannot exceed reserves or numeric capacity.
- Rejected updates leave every index and liability field unchanged.

### SEC-28: vesting cancellation confiscates vested entitlement

Severity: High for a standard revocable-vesting interpretation; example/template finding.

Locations:

- `examples/vesting_program/src/lib.rs:418-506`
- `examples/vesting_program/tests/surfpool/src/lib.rs:297-376`

Mechanism:

Cancellation does not read Clock or calculate vested-but-unclaimed entitlement. It returns the entire remaining vault to the administrator, even after the schedule has ended. The existing real-runtime test asserts this behavior after an elapsed schedule.

Fix:

Choose and document the policy. For revocable vesting, add Clock and beneficiary ATA to cancellation, pay `vested - claimed` to the beneficiary first, return only unvested/excess tokens, then close. If vesting is intended to be confiscatory or irrevocable, name and enforce that policy explicitly rather than presenting it as ordinary vesting.

Acceptance criteria:

- Tests at pre-start, cliff, midpoint, and end conserve the vault and never return vested entitlement to the administrator under the revocable policy.

### SEC-29: vesting promises value before it is funded

Severity: High for an asset-bearing adaptation; example/template finding.

Locations:

- `examples/vesting_program/src/lib.rs:214-269`
- `examples/vesting_program/tests/surfpool/src/lib.rs:297-328`

Mechanism:

Initialization stores `total_amount` and creates an empty vault. Funding occurs in a later transaction. A valid-looking, already-active schedule can therefore promise arbitrary value while holding zero tokens; failure surfaces only when the beneficiary claims.

Fix:

Atomically transfer the allocation during initialization and record the observed amount. If a two-phase design is required, keep the schedule inactive until a funding instruction proves full collateralization.

Acceptance criteria:

- Every active schedule has `vault.amount >= total_amount` under the supported token policy.
- Insufficient funding rolls back state and vault creation.

### SEC-30: accepted Token-2022 state can have no exit path

Severity: High liveness/asset-lock risk; example/template finding.

Locations:

- `examples/staking_rewards_program/src/lib.rs:247-274`
- `examples/staking_rewards_program/src/lib.rs:629-635`
- `examples/vesting_program/src/lib.rs:214-269`
- `examples/vesting_program/src/lib.rs:393-399`
- `examples/vesting_program/src/lib.rs:492-498`

Mechanism:

Initializers accept both supported token-program owners without applying the later exit path's extension policy. Claim/cancel calls `assert_no_extensions()`. A Token-2022 mint with a benign extension can initialize and receive funds, then fail every exit with `InvalidAccountData`.

Fix:

Apply the exact mint and token-account extension policy before creating state or vaults. Either reject Token-2022, reject all extensions at initialization, or explicitly allow-list and implement each supported extension.

Acceptance criteria:

- Every configuration accepted at initialization has working withdraw/claim/cancel/close paths.
- Unsupported extensions fail before lamports or tokens move.

### SEC-31: escrow has no supported recovery path

Severity: High liveness for a value-bearing adaptation; disclosed example limitation.

Locations:

- `examples/escrow_program/src/lib.rs:35-39`
- `examples/escrow_program/src/lib.rs:179-214`
- `examples/escrow_program/src/lib.rs:236-364`

Mechanism:

Only `Make` and `Take` exist. If no taker arrives, requested token B becomes unavailable, or the market moves, the maker has no instruction that returns token A and closes the state. Self-taking depends on owning B and is not a cancellation contract.

Fix:

Add maker-signed `Cancel`, validate the maker, stored mints, escrow PDA, and canonical vault, return the recorded token A, define treatment of unsolicited donations, then close vault and state. Add expiry if offers should eventually become untakeable.

Acceptance criteria:

- The maker can cancel without holding token B.
- A wrong signer cannot cancel.
- Exactly one of Take or Cancel can succeed, including transaction-order races.

### SEC-32: oracle authority rotation does not rotate the publisher

Severity: High for a financial consumer; example/template finding.

Locations:

- `examples/prop_amm_program/src/lib.rs:36-40`
- `examples/prop_amm_program/src/lib.rs:105-166`
- `examples/prop_amm_program/tests/surfpool/src/lib.rs:138-205`

Mechanism:

`Update` authorizes an immutable global constant. `RotateAuthority` changes a stored field that `Update` never reads. After rotation, the new authority still cannot publish while the old global key remains authorized for every oracle. The existing test explicitly confirms this behavior.

Fix:

If the stored authority is the publisher, authorize `Update` against `oracle.authority` and remove the constant. If administrator and publisher are distinct, store both and provide separately named, two-step rotations.

Acceptance criteria:

- After publisher rotation, the new key can update, the old key cannot, and unrelated feeds are unaffected.
- The account also needs feed identity, exponent, update slot/sequence, confidence, and status before it is suitable for financial consumption; consumers must enforce freshness and domain bounds.

## Performance findings

### PERF-01: the performance gate can compare the wrong artifacts or workloads

Priority: P0 because trustworthy measurement is a prerequisite for every later optimization.

Locations:

- `.github/workflows/performance.yml:87-335`
- `.github/workflows/performance.yml:414-429`
- `scripts/benchmark-core.ts:213-224`
- `scripts/compare-compute-units.ts:404-410`
- `scripts/measure-example-compute-units.ts:50-54`
- `scripts/measure-example-compute-units.ts:452-475`
- `crates/pina_test/src/lib.rs:1485-1489`

Mechanisms:

1. Core base and head builds share one Cargo target directory. The workflow already documents a stale-artifact failure in the CLI benchmark and isolates those builds, but `benchmark-core.ts` still shares its target. A head measurement can reuse a base artifact.
2. Failed or incomplete base instruction runs become green “new baselines” because head-only records are accepted.
3. The candidate revision supplies its own policy and benchmark harness for both sides. A change can weaken the gate that evaluates itself.
4. Instruction records collapse fixtures by program plus first discriminator byte and omit success/error outcome. Multiple cases can be merged into one maximum, and a newly failing transaction can be compared as if it were the same successful case.
5. Base and head static scores can use different profiler implementations and cost models.

Fix:

- Give base and head isolated target directories and set `CARGO_INCREMENTAL=0` for release measurements.
- Record and compare executable/ELF hashes.
- Run one trusted base-revision harness and policy against both artifacts whenever compatibility permits.
- Require identical case inventories for code that exists on both revisions. A missing base result is an error, not a new baseline.
- Give every fixture a stable case ID containing full discriminator/version, fixture identity, and expected outcome.
- Build one pinned profiler/cost model and analyze both ELFs with it.

Acceptance criteria:

- A source-distinct base/head sentinel test proves each side executes its own artifact.
- A forced base-case failure fails the comparison.
- Two fixtures sharing the first discriminator byte remain separate.
- A success-to-error change fails even if compute units decrease.

### PERF-02: compact resizable updates repeat structural patch work

Locations:

- `crates/pina/src/cpi.rs:1647-1658`
- `crates/pina/src/cpi.rs:1671-1683`
- `crates/pina_macros/src/account.rs:552-569`

Mechanism:

`UpdateResizableAccount` calls `updated_len` before reallocating. The patch's update path recalculates the same layout, and application validation can construct and scan another compact view. Large or many-tailed accounts pay repeated content-dependent traversal.

Fix:

Introduce a detached `PreparedPatch` containing the validated source layout, target length, tail offsets, and staged edits. Prepare it before reallocating, then consume it exactly once after realloc. The prepared type must be internal/sealed so it cannot be forged across different bytes. It must also enforce the migration contract from SEC-02.

Acceptance criteria:

- A test-only validation counter proves one source structural scan per update.
- Benchmarks cover 1/4/8/16 tails for no-op, same-size, grow, and shrink patches.
- Corresponding SBF instructions record CU; host timing alone is insufficient.

### PERF-03: compact migration ladders perform `3L + 1` scans

Locations:

- `crates/pina/src/migration.rs:717-726`
- `crates/pina/src/migration.rs:825-831`
- `crates/pina/src/migration.rs:900-907`

Mechanism:

Each step validates its source, validates its destination twice around a version-field write, and the next step validates that destination again as its source. Finalization performs another current-layout validation. An eight-step ladder can perform 25 full structural scans.

Fix:

Carry a sealed `ValidatedDestination` capability between adjacent steps. Validate each source and each destination once. After a range-checked version-field write, perform only the cheap envelope check needed to preserve the capability. Keep public planners defensive; use the optimized capability only inside the migration executor.

Acceptance criteria:

- A counting fake proves at most one source and one destination validation per step.
- Benchmarks cover 1/4/8 steps, 1/4/8 tails, and small versus near-maximum payloads.
- Malformed intermediate output still fails before commit.

### PERF-04: the migration Rent cache is never populated

Locations:

- `crates/pina/src/migration.rs:681-700`
- `crates/pina/src/migration.rs:750-756`
- `crates/pina/src/migration.rs:1056-1059`
- `crates/pina/src/migration.rs:1222-1233`

Mechanism:

The context stores `Option<Rent>`, but a cache miss calls `Rent::get()` without writing the result back. Every growing transition can reload the sysvar, and reserved multi-account migration passes the unchanged empty cache between accounts.

Fix:

Pass `&mut Option<Rent>` through the executor, populate it on the first actual growth, and share it across all steps and reserved account slots. Preserve the zero-fetch path when no account grows.

Acceptance criteria:

- A counting provider proves eight growing steps across multiple accounts perform one lookup.
- No-growth migration performs zero lookups.

### PERF-05: generated mutable-account parsing is quadratic

Locations:

- `crates/pina_macros/src/accounts.rs:160-171`
- `crates/pina/src/traits.rs:1087-1096`
- `crates/pina/src/traits.rs:1124-1138`
- `crates/pina/src/traits.rs:1181-1191`
- `crates/pina/src/traits.rs:1212-1224`

Mechanism:

The derive emits one cursor advance per mutable field. Each advance scans all remaining accounts for the same address, producing `n(n-1)/2` comparisons on the valid all-unique path. Mutable `remaining` adds another nested scan. Because account lists are attacker-supplied instruction input, this is also a bounded compute-exhaustion surface.

Fix:

Generate composable mutable-slot metadata and perform one top-level, allocation-free alias preflight. Preserve nested, optional, filler, and `remaining(distinct = false)` semantics and current error ordering. A heapless sorted index or a runtime-provided duplicate-origin identity is preferable to repeated 32-byte address comparisons.

Acceptance criteria:

- Benchmarks cover 4/16/32/64 unique writable accounts and duplicates at early/late positions.
- Comparison counts scale near-linearly.
- Nested and optional-account policy tests remain identical.

### PERF-06: unchanged generated trees are deleted and rewritten

Locations:

- `crates/pina_cpi_renderer/src/lib.rs:172-176`
- `crates/pina_cpi_renderer/src/render/scaffold.rs:112-132`
- `crates/pina_codama_renderer/src/lib.rs:169-178`
- `crates/pina_codama_renderer/src/render/scaffold.rs:69-85`

Measured evidence:

- Identical CPI update: `21.4 ± 6.1 ms` over 10 runs.
- Identical Rust-client update: `30.4 ± 3.4 ms` over 10 runs.
- In both cases `claim.rs` retained the same SHA-256 and size but received a new modification time.

The direct milliseconds are modest. The larger cost is invalidating downstream Cargo incremental builds and file watchers.

Fix:

Replace delete-and-recreate with a content-aware sync. Skip writes when bytes match, atomically replace changed files, and remove only stale renderer-owned paths. Preserve all current symlink protections.

Acceptance criteria:

- A second identical generation preserves hash, inode, and mtime.
- `cargo build -vv` schedules no generated crate rebuild after a no-op update.
- Removing an IDL item still deletes only its stale generated files.

### PERF-07: Dart publication copies the complete bin tree once per program

Location: `crates/pina_cli/src/codama.rs:178-185`.

Mechanism:

The generated publication template copies staging `bin` into output `bin` inside the program loop. The current 26-program, 26-bin tree performs 676 bin-file copies instead of 26.

Fix:

Move `publishDirectory(stagingRoot/bin, outputRoot/bin)` after the program loop.

Acceptance criteria:

- An injected copy counter reports one tree publication for an N-program fixture.
- Add 1/8/26-program Dart generation timings to the CLI benchmark.

### PERF-08: IDL extraction contains avoidable quadratic work

Locations:

- `crates/pina_cli/src/parse/mod.rs:559-595`
- `crates/pina_cli/src/parse/module_resolver.rs:37-69`
- `crates/pina_cli/src/parse/module_resolver.rs:90-163`

Mechanism:

Dispatch resolution linearly searches instruction and account collections for every instruction. Module deduplication uses `Vec::contains`, and lexical aliases such as `a/../a.rs` can evade identity checks. The parser also rescans AST facts already extracted by the assembler.

Fix:

- Build validated borrowed name maps once and reject duplicate names explicitly.
- Normalize or canonicalize existing module paths before identity checks, require containment within the source root, mark before scheduling, and use a `HashSet` beside a deterministic order vector.
- Return entrypoint/instruction facts from the first AST pass.

Acceptance criteria:

- Synthetic 100/500/1,000-instruction and 16/64/256-module fixtures scale near-linearly.
- A `#[path]` alias cycle terminates with a typed error and reads each inode once.

### PERF-09: profiler retains avoidable copies and broad object formats

Locations:

- `crates/pina_profile/Cargo.toml:13-18`
- `crates/pina_profile/src/lib.rs:38-49`
- `crates/pina_profile/src/elf.rs:78-127`
- `crates/pina_profile/src/sbf.rs:119-128`
- `crates/pina_profile/src/output.rs:69-71`
- `crates/pina_profile/src/compare.rs:565-570`

Mechanism:

The profiler reads the complete ELF, clones `.text`, allocates symbol names and clones them again, then materializes complete JSON strings. The `object/read` feature also enables archive, COFF, Mach-O, PE, and XCOFF readers although the tool rejects non-ELF input.

Fix:

Borrow `.text` from the ELF buffer, consume the symbol vector so names move, stream JSON with `to_writer_pretty`, and narrow `object` to the minimal ELF/read-core feature set after confirming compatibility.

Acceptance criteria:

- Counting-allocator benchmarks cover 1/8/32 MiB text and 10k/100k symbols.
- Measure binary-size change with the actual release profile before merging the feature reduction.

### PERF-10: core timing quantizes valid measurements to zero

Locations:

- `crates/pina/tests/benchmarks.rs:29-45`
- `scripts/benchmark-core.ts:136-190`

Measured evidence:

The release benchmark passed 25/25 but reported several `0 ns` operations. It divides a fixed 1,000 iterations into integer nanoseconds. The comparator treats a zero baseline as having no usable percentage, so regressions in the fastest operations can escape classification.

The same run measured:

| Case                                     | Native host time |
| ---------------------------------------- | ---------------: |
| Compact PDA loader with stored bump      |         5,055 ns |
| Compact PDA loader with canonical search |        43,012 ns |
| Ratio for that fixture                   |            8.51× |
| `create_program_address` with known bump |         3,475 ns |

These are host timings, not Solana CU measurements.

Fix:

Calibrate batch size to a minimum wall time, emit total duration plus iteration count, retain raw samples at higher precision, alternate AB/BA ordering, and compare paired uncertainty. Reject samples at timer resolution instead of recording zero. Criterion is acceptable if its output remains deterministic and easy for CI to compare.

Acceptance criteria:

- No accepted sample is zero or below timer resolution.
- A controlled 5–10% slowdown is detected reliably without flagging an unchanged control.

### PERF-11: verification tail buffering repeatedly moves memory

Location: `crates/pina_cli/src/verification.rs:524-540`.

Mechanism:

The one-megabyte tail buffer is a `Vec` that repeatedly drains its prefix as new 8 KiB chunks arrive. Long output causes repeated memory moves proportional to the retained tail size.

Fix:

Use a fixed-size ring buffer or bounded `VecDeque` and write through a two-slice view when formatting the final tail.

Acceptance criteria:

- A 100 MiB stream has bounded RSS and near-linear time.
- The retained final bytes match the current behavior exactly.

## Hardening and coverage findings

These items are real weaknesses but should not be presented as demonstrated asset-loss vulnerabilities without their stated preconditions.

### Fractional staking rewards are lost at every checkpoint

`examples/staking_rewards_program/src/lib.rs:118-130`, `:428-445`, and `:511-527` floor each accrual interval independently, then advance the checkpoint. Splitting one index movement across deposit/withdraw checkpoints can pay less than calculating the same total movement once. Store a per-position scaled remainder: `numerator = remainder + stake * delta` in `u128`, bank `numerator / SCALE`, and persist `numerator % SCALE`. A property test should prove checkpoint partitioning changes the result by zero base units.

### Compact application validation is post-write

`crates/pina_macros/src/account.rs:546-569` documents that structural patch failures are atomic but application validation runs after mutation. This is safe only when callers propagate the error so the Solana runtime rolls back the whole instruction. `crates/pina/src/traits.rs:861` should not describe the operation as unconditionally atomic. Correct the public contract and add an instruction-level rollback test. If practical, stage or roll back bytes so off-chain callers and error-swallowing code cannot observe rejected mutations.

### Release dry runs do not exercise package readiness

`.github/workflows/publish.yml:314-319` skips attestation in dry-run mode and `:406-411` skips the whole package-assembly/publish job. The pre-tag gate therefore does not run `package-cli.mjs`, npm pack inventory checks, or Monochange registry readiness. Split credential-free assembly/readiness into an always-run validation job and gate only registry mutation on the protected environment.

### Archive extraction and npm wrapper versioning need tightening

`scripts/npm/package-cli.mjs` should reject symlink archive entries and require regular files at exact expected paths, clear known output destinations before extraction, and clean temporary directories in `finally`. `scripts/npm/sync-release-versions.mjs:43-58` should pin platform optional dependencies to the exact wrapper version instead of a caret range. Dry-run fixtures should cover missing lint driver, symlink entry, stale destination driver, extra archive, and wrapper/native version skew.

### Fuzz smoke coverage can pass without reaching valid states

The committed account seed is 18 bytes while the target accepts 11, 43, or 84 bytes. The instruction seed begins outside both tested discriminator ranges, and matching-ID success is not asserted. The smoke script hardcodes three targets, changes to the three example dependencies do not trigger fuzz CI, and standalone fuzz locks escape dependency audit. Add at least one accepted seed per state/version, assert success-path semantics, derive targets from Cargo metadata, require a seed directory per target, and include all direct code/lock inputs in path filters.

### Release overflow behavior should be an explicit product decision

`crates/pina_cli/src/build.rs:429-439` disables overflow checks unless requested or enabled by the workspace release profile, and scaffolding writes `overflow-checks = false`. The framework uses checked arithmetic in audited value paths, so this review did not demonstrate an overflow exploit. Still, value-bearing programs should either default to checked release arithmetic or require an explicit deployment acknowledgement recorded in provenance. Add `pina doctor`/lint guidance rather than silently assuming every downstream calculation is guarded.

### Static code cost is not runtime compute units

`crates/pina_profile/src/cost.rs:20-34` assigns flat syscall costs and `src/sbf.rs:29-44` charges every eight-byte slot without control-flow execution. Treat this as a static code-cost score, not runtime CU. Rename the metric and never substitute it for real instruction measurements.

## Continuation: PinaPod dependency deep-dive

Continuation date: 2026-09-22 (second session). Same audited revision `407545c0e4eab33f444b49a886ab264f79ae1530`; still read-only. This section extends the audit into the `pinapod` dependency itself — the crate that defines the workspace's account wire format and its unsafe zero-copy validation contracts. The first session audited Pina's use of `PinaPodPatch` (SEC-02, PERF-02) but not the dependency's own runtime and derive.

Scope of this continuation:

- `pinapod` 0.4.1 runtime: `lib.rs`, `traits.rs` (the `ZcElem`, `ZcValidate`, `ZcField`, `PinaPodFixed`, `PinaPodCompact`, and `PinaPodPatch` contracts and default bodies), `error.rs`, and every `pod` module (`bool`, `numeric`, `option`, `string`, `vec`, `wincode`).
- `pinapod-derive` 0.4.1 codegen: `compact.rs` (header/ref/mut/patch generation and the two-phase tail commit), `compact_enum.rs` (storage-enum patches), `fixed.rs` (fixed companions and fieldless storage enums), `schema.rs` (attribute grammar), and `type_map.rs` (type classification and pod mapping).
- The version-drift surface between the locked 0.4.1 and the published 0.4.3.
- Pina-side integration edges not covered by the first session: `PinaCompactAccount::validate_size` delegation (`crates/pina/src/traits.rs:214-217`), the discriminator stored as a `skip_patch` inline header field, and the `UpdateResizableAccount` grow-before/shrink-after ordering (`crates/pina/src/cpi.rs:1644-1698`).

### SEC-33: the wire-format crate is a caret requirement, not an exact pin

Severity: Medium supply-chain and deployment-consistency risk.

Locations:

- `Cargo.toml:255`
- `Cargo.lock` (`pinapod` 0.4.1, `pinapod-derive` 0.4.1)
- `crates/pina/Cargo.toml:46`

Mechanism:

`pinapod` and its derive define the account byte layout, the unsafe `ZcElem` validation contract, and the compact patch commit. The workspace requirement is `pinapod = { version = "0.4.0" }` — a caret range — while the same manifest exact-pins `fixed = { version = "=1.30.0" }` with a comment stating that bit layouts are the schema contract (`Cargo.toml:234-236`). The lockfile holds 0.4.1; the registry publishes 0.4.3, which changes generated code and runtime behavior (SEC-34 and SEC-35 document the drift).

CI builds with `--locked`, so drift never happens implicitly inside a job. The exposure is procedural: any dependency-refresh pull request, lockfile-conflict resolution, or local `cargo update` silently moves the pair, and every job passes because the move is semver-compatible. A program rebuilt after such a refresh links different account-layout codegen than the previously deployed binary, with no gate that notices.

Executed reproduction:

```sh
devenv shell -- cargo update --dry-run --offline -p pinapod
```

Observed:

```text
Locking 2 packages to highest compatible versions
Updating pinapod v0.4.1 -> v0.4.3
Updating pinapod-derive v0.4.1 -> v0.4.3
```

A `diff -ru` of the two registry checkouts shows the drift is not cosmetic: 0.4.3 adds new public trait surface (`ZcValidate::validate_slice`, `PinaPodCompact::validate_layout`, `pinapod::traits::commit_entry_validate` with its `compact-commit-full-validation` feature), promotes the patch length-agreement checks from `debug_assert_eq!` to release-mode errors, and adds write-time capacity rejection to the `wincode` serializers.

Fix:

1. Change the workspace requirement to an exact pin: `pinapod = { version = "=0.4.1", default-features = false, features = ["solana-address", "solana-program-error"] }`.
2. Add a lockfile-integrity check to `lint:all` (TypeScript, per repository scripting conventions) that fails when `pinapod`, `pinapod-derive`, or `fixed` in `Cargo.lock` differs from the exact-pinned version, so a future move requires editing the pin deliberately and in review. The same script should assert that no workspace crate enables `pinapod/wincode` (see SEC-35).
3. Record the review checklist for moving the pin: diff both versions' generated code for at least one compact and one fixed example, confirm the ABI document and generated clients regenerate byte-identically, and rerun the WO-09 patch-contract suite on both versions.

Acceptance criteria:

- `cargo update -p pinapod` succeeds without changing the lockfile.
- The integrity check fails on a fixture lockfile that moves `pinapod` within the old caret range.
- Moving the pin requires a manifest edit that appears in the diff of the same pull request.

### SEC-34: patch length agreement is enforced only by debug assertions

Severity: Low–Medium hardening; an availability risk, not a memory-safety risk.

Locations:

- `crates/pina/src/cpi.rs:1685` (`debug_assert_eq!(encoded_len, target_size)` after the realloc)
- `crates/pina_macros/src/account.rs:552-570` (the account-level `update` returns the patch's length with no cross-check)
- pinapod 0.4.1 generated `Patch::update` and `Patch::try_initialize` (`debug_assert_eq!(encoded_len, expected_len)`)

Mechanism:

`UpdateResizableAccount::invoke_signed_inner` computes `target_size` from `PinaPodPatch::updated_len`, reallocates the account to it, applies the patch, and then verifies `encoded_len == target_size` only through `debug_assert_eq!`. The workspace defines no `[profile.release]` overrides, so release and SBF builds compile the assertion away. The same pattern exists inside pinapod 0.4.1's generated update and initialize.

If the preflight and the commit ever disagree — a pinapod generator defect, or a drift like SEC-33 that changes one walk but not the other — a shipped program silently persists an account whose allocation was sized for the prediction while its bytes encode a different length. The next validated read rejects that account. No memory-safety violation occurs, because pinapod's commit is bounds-checked against the buffer and the buffer is exactly `target_size`; the account is instead bricked with its lamports locked until the bytes are repaired off-chain.

Upstream pinapod 0.4.3 reached the same conclusion and promoted both checks to release-mode error returns, with the comment that failing loudly is the right release-mode answer because the caller would otherwise trust and persist a length the bytes do not support.

Fix:

1. In `crates/pina/src/cpi.rs`, replace the `debug_assert_eq!` with a real error return (`ProgramError::InvalidAccountData`) when `encoded_len != target_size`. Returning the error fails the instruction, and Solana rolls the realloc back with the rest of the transaction.
2. Mirror the check in the macro-generated account-level `update` (`crates/pina_macros/src/account.rs`) by comparing the patch result against a pre-computed `updated_len` prediction.
3. Adopt the pinapod patch-contract regression suite (WO-09) so prediction-versus-commit agreement is asserted externally on every pull request, independent of the compiled-out internal check.

Regression that must pass after the fix and would fail on the first invariant break:

```rust
let predicted = PinaPodPatch::<T>::updated_len(&patch, &buf)?;
let got = PinaPodPatch::<T>::update(&patch, &mut buf)?;
assert_eq!(predicted, got);
```

executed over a randomized corpus of mixed grow/shrink edits across every tail shape (WO-09 specifies the suite). On the current tree this passes — 40,000 randomized operations during this audit — but nothing in CI would catch its first failure, which is precisely the gap.

Acceptance criteria:

- A release-built example program contains the comparison (prove by disassembly or by a unit test on the new error path).
- The WO-09 contract suite runs in both `coverage:all` and `test:all`.
- The account-level `update` fails closed on prediction mismatch under every feature combination reachable through SEC-02's fix.

### SEC-35: wincode serializers emit a corrupt stored prefix verbatim

Severity: Low, latent. No default or optional feature path in this workspace enables `pinapod/wincode`: `pina` adds only `solana-address` and `solana-program-error` through the workspace dependency, and its `fixed`/`floats` features add only `pinapod/fixed` and `pinapod/floats`. The `wincode` occurrences in `benchmarks/framework-comparison` are those programs' own direct dependency, not pinapod's feature.

Locations:

- pinapod 0.4.1 `src/pod/wincode.rs`, `PodString::write` and `PodVecRepr::write`.
- Upstream fix: pinapod 0.4.3 adds a write-time `decode_len() > capacity()` rejection to both writers.

Mechanism:

The canonical serializers write the stored length prefix verbatim and clamp only the payload copy. A `PodString` whose prefix decodes above its capacity — constructible only from unvalidated raw bytes in current safe code — serializes into wire bytes whose prefix lies about the payload that follows, and the round trip fails validation on re-read. Fail-closed, but the writer knowingly produces unreadable bytes. Pinapod 0.4.3 rejects at the boundary.

Actionable step: include "wincode stays disabled, or the pinapod pin is ≥ 0.4.3" in the pin-movement checklist from SEC-33, and have the integrity script assert the feature set so enabling `pinapod/wincode` anywhere is a reviewed decision.

### Verified controls and negative findings from the continuation

The following were checked and held; they should be preserved:

- Alignment and padding: every pod is `#[repr(transparent)]` or `#[repr(C)]` over byte arrays with compile-time alignment assertions; generic companions carry explicit `ZcElem` bounds; no padding path was found.
- Prefix decoding: `try_decode_len` rejects values wider than `usize` on every target; safe accessors clamp decoded lengths to capacity before slicing; the compact codegen licenses its unchecked arithmetic per prefix width — compile-time capacity assertions for prefixes of one and two bytes, checked forms for wider prefixes.
- Option tags, bool bytes, enum discriminants, UTF-8, and per-element validation are each enforced in `validate` before any reference is formed. `as_str`'s `from_utf8_unchecked` is reachable only after validation, because no safe constructor can produce invalid active bytes: raw construction requires `unsafe`, and every writer takes a `&str` or validated element.
- The two-phase commit (backward shifts forward-first, forward shifts reverse-first, edited fields written last) provably never reads overwritten source bytes for mixed grow/shrink edits; post-shrink gaps are zeroed back to the old encoded length; `initialize` zeroes the destination before and after failure.
- Buffer aliasing between patch sources and the destination is prevented by Rust borrows: both the patch builders and the staged writer tie their source lifetimes to the data borrow, so a source pointing into the buffer being updated cannot be constructed through safe code.
- The derive grammar rejects unsupported dynamic nesting (`Option<Vec<String<_>, _>>`), unsupported prefix widths, non-suffixed compact fields, and valued or duplicate `#[pinapod(...)]` options with compile errors rather than silent misclassification. Caller-local lookalikes (a user module named `pina` exporting its own `String`) can be misrecognized by the compact classifier, but the generated code stays internally consistent and anything requiring the representation contract fails to compile.
- `PinaCompactAccount::validate_size` delegates directly to `PinaPodCompact::validate_storage_len`, so Pina's size gate and pinapod's allocation gate cannot disagree. The account discriminator is a `skip_patch` inline header field, and the new encoded length after any patch is always on the storage grid because each tail's encoded size is a multiple of the declared tail-alignment atoms.

Executed verification (host, scratch crate outside the repository at `/tmp/pinapod-audit-2026-09-22`, pinapod `=0.4.1`):

- Property fuzz: 20,000 randomized compact-struct patch operations plus 20,000 compact-enum operations across every supported tail shape (always-string, always-vec, optional-string, optional-vec, fixed-stride vec-of-string; unit, string, and vector enum payloads), asserting prediction/commit agreement, post-update validation, shadow-model readback of every field, shrink-gap zeroing, reject-path byte preservation (`Overflow` and `BufferTooSmall`), corrupt prefix/tag/UTF-8/bool/discriminant rejection, off-grid and over-`MAX_SIZE` storage rejection, and `initialize` zeroing. All passed in debug and in release with overflow checks retained.
- Miri: the same suite at 400 steps per fuzzer plus every reject path ran under `cargo miri test` on `nightly-2026-09-15-aarch64-apple-darwin` with no undefined behavior (2,054.9 s).

No exploitable defect was found in pinapod 0.4.1's compact patch machinery by source review or by execution. The findings above are the pin-drift exposure (SEC-33), the compiled-out consistency guard (SEC-34), and the latent wincode boundary (SEC-35).

## Verified controls and negative findings

The following controls were checked and should be preserved:

- No confirmed signer, owner, writability, arbitrary-CPI, PDA-address, checked-arithmetic, or duplicate-mutable bypass was found in the core runtime paths reviewed.
- The custom Pina security lint suite passed.
- The 26-test adversarial invariant suite passed.
- Focused CPI tests passed 25/25, and account-derive validation tests passed 28/28.
- All external GitHub Actions references inspected were pinned to full commit SHAs.
- Checkout credentials are disabled on reviewed checkouts; no `pull_request_target` workflow was found.
- npm installation in the publisher is frozen and runs with lifecycle scripts disabled.
- OIDC is used for registry publishing behind the `publisher` environment.
- A full-history gitleaks scan reported no secret leak.
- No generic shell argument injection was found in normal deployment. Shell execution is limited to the explicitly operator-controlled `--remote-command` path; its missing independent deployment proof is SEC-12.

These controls do not cancel the findings above. In particular, an attestation proves who built a byte sequence, not that later mutable downloads contain that sequence, and pinned Actions do not make ref-controlled local actions safe in a credentialed arbitrary-ref workflow.

## Implementation work orders

Each work order is intentionally scoped so another coding agent can implement and verify it without rediscovering the audit.

### WO-01: restore staking custody before any further example work

Files:

- `examples/staking_rewards_program/src/lib.rs`
- `examples/staking_rewards_program/tests/e2e.rs`
- `examples/staking_rewards_program/tests/surfpool/src/lib.rs`
- generated IDL and clients for this example

Steps:

1. Add canonical stake-vault accounts to Deposit and Withdraw.
2. Implement checked token movement with the current no-extension policy.
3. Commit accounting only in the same instruction after the transfer; use observed deltas if extension support is added.
4. Rewrite the existing reward journey so tokens are minted before deposit and assert vault/accounting equality after every transition.
5. Add zero-balance, wrong-vault, wrong-token-program, insufficient-balance, late-CPI-failure, and withdrawal rollback tests.
6. Fix the module/README contract so it says exactly what moves value.

Exit gate: SBF/Surfpool journey proves an unfunded depositor receives zero rewards and every success preserves custody/accounting equality.

### WO-02: close the compact update feature-matrix bypass

Files:

- `crates/pina/src/cpi.rs`
- `crates/pina_macros/src/account.rs`
- compact/migration test fixtures

Steps:

1. Add the no-`validation` stale-envelope regression first.
2. Route every branch through one migration-aware update contract.
3. Introduce a sealed prepared-patch plan only if needed to avoid repeated traversal.
4. Test current/stale/future envelopes across supported feature subsets.
5. Measure SBF CU before and after; do not trade correctness for one fewer scan.

Exit gate: stale bytes are rejected and unchanged in every feature combination, with no regression for current data.

### WO-03: split release planning from release authority

Files:

- `.github/workflows/release-pr.yml`
- `.github/workflows/publish.yml`
- `.github/workflows/semver.yml`
- `monochange.toml`

Steps:

1. Make arbitrary-ref planning read-only and secretless or remove manual arbitrary-ref dispatch.
2. Prove protected-main ancestry before minting credentials or creating tags.
3. Validate and select the complete release target set before mutation.
4. Correlate the exact dry-run ID by ref and SHA.
5. Transport one immutable asset set plus digest manifest through attestation and packaging.
6. Distinguish semantic incompatibility from semver tool failure.
7. Add fixture-based workflow tests for malicious ref, target order, interleaved dispatch, asset mutation, and operational semver failure.

Exit gate: no branch-controlled command runs with release authority, no mutation precedes complete validation, and packaged bytes equal attested bytes.

### WO-04: repair multisig temporal and authority invariants

Files:

- `examples/multisig_program/src/lib.rs`
- `examples/multisig_program/tests/e2e.rs`
- `examples/multisig_program/tests/surfpool/**`
- migration manifest and generated clients if proposal layout changes

Steps:

1. Revoke spending limits against current membership at use time.
2. Add the missing config expiry check.
3. Store immutable `execute_after` or consistently stale older vault proposals after timelock changes.
4. Bind global config initialization to a trusted bootstrap mechanism and add authority rotation.
5. Bind all refunds to a deterministic approved collector.
6. Reject impossible TTL/timelock combinations.
7. Permit close of terminal, expired, or stale proposals.

Exit gate: the seven adversarial state transitions in SEC-08 through SEC-11 and SEC-20 through SEC-22 pass with byte/lamport conservation assertions.

### WO-05: make deployment receipts evidence-based and ledger mutation serializable

Files:

- `crates/pina_cli/src/commands.rs`
- `crates/pina_cli/src/deploy.rs`
- `crates/pina_cli/src/migrations/ledger.rs`
- deployment/ledger integration tests

Steps:

1. Add finalized RPC readback and exact executable/authority verification.
2. Treat every explicit URL as unknown until genesis identity is proven.
3. Acquire the ledger lock before every read-modify-write sequence.
4. Add no-op deploy, wrong hash/authority, tunnelled loopback, RPC ambiguity, and barrier-controlled concurrency tests.

Exit gate: a receipt is cryptographic/on-chain evidence, not a subprocess status, and concurrent mutation cannot lose a successful record.

### WO-06: centralize secure URL and filesystem publication boundaries

Files:

- `crates/pina_cli/src/import_idl.rs`
- `crates/pina_cli/src/idl_metadata.rs`
- `crates/pina_cli/src/verification.rs`
- shared path/network utility modules

Steps:

1. Validate every redirect hop and redact all URL credentials from display/persistence.
2. Create one descriptor-relative, no-follow atomic output primitive and migrate IDL/verification sinks.
3. Add subprocess cancellation and timeouts.
4. Semantically deserialize and verify exported transactions.

Exit gate: redirect, secret sentinel, symlink ancestor, over-limit child, and mutated transaction fixtures all fail safely.

### WO-07: repair the performance oracle before optimizing hot paths

Files:

- `.github/workflows/performance.yml`
- `scripts/benchmark-core.ts`
- `scripts/compare-compute-units.ts`
- `scripts/measure-example-compute-units.ts`
- `scripts/benchmark-cli.ts`

Steps:

1. Isolate base/head artifacts and record hashes.
2. Pin one trusted harness/policy and profiler.
3. Require complete, stable case inventories and outcomes.
4. Replace zero-quantized fixed-iteration timing with calibrated raw samples.
5. Add no-op, multi-program, multi-module, Dart, TypeScript, CPI, profiler, and failure-path benchmark cases.

Exit gate: sentinel tests prove the gate fails for stale artifact reuse, missing base results, success-to-error changes, and a controlled slowdown.

### WO-08: optimize only after the oracle is trustworthy

Suggested stack, each independently reviewable:

1. Populate the migration Rent cache.
2. Move Dart bin publication out of the program loop.
3. Add content-aware generated-tree sync.
4. Add name maps and canonical module identity to IDL extraction.
5. Introduce compact prepared-patch and validated-destination capabilities.
6. Replace quadratic account alias scanning.
7. Reduce profiler copies and feature closure.

Every change should include a regression test, a before/after benchmark, and an SBF CU measurement when the path executes on-chain.

### WO-09: pin pinapod exactly and adopt the patch-contract suite

Files:

- `Cargo.toml`
- `Cargo.lock`
- `scripts/` (new integrity script; TypeScript)
- `crates/pina/tests/` (new contract suite)
- `crates/pina/src/cpi.rs`
- `crates/pina_macros/src/account.rs`
- devenv task wiring for `lint:all`, `coverage:all`, and `test:all`

Steps:

1. Exact-pin `pinapod = "=0.4.1"` in the workspace manifest and verify `cargo update -p pinapod` becomes a no-op.
2. Add the lockfile/feature integrity script to `lint:all`: exact versions for `pinapod`, `pinapod-derive`, and `fixed`; assert `pinapod/wincode` is enabled nowhere in the workspace graph. Include a drifted-fixture test so the script itself is covered.
3. Port the audit's patch-contract suite into `crates/pina/tests/` against a local compact schema and compact enum covering every tail shape: seeded deterministic prediction/commit agreement fuzz, shadow-model readback of every field after each update, shrink-gap zeroing (`[new_encoded, old_encoded)` all zero), reject-path byte preservation for `Overflow` and `BufferTooSmall`, corrupt-representation rejection (length prefix above capacity, option tag not 0/1, invalid UTF-8, bool byte not 0/1, unknown enum discriminant), off-grid and over-`MAX_SIZE` storage rejection, and `initialize` zeroing on failure. Make the step count configurable through an environment variable so a Miri invocation can run a reduced corpus.
4. Add the release-mode length-agreement error to `UpdateResizableAccount::invoke_signed_inner` and to the macro-generated account-level `update` (SEC-34), with a unit test on the new error path.
5. Register the suite in both `coverage:all` and `test:all` per the coverage-equivalence rule, and document the manual Miri command in the test module.

Exit gate: the integrity script fails on a drifted fixture lockfile; the contract suite fails if pinapod ever returns a commit length that differs from its preflight prediction; no example program binary relies on `debug_assert_eq!` for the realloc consistency invariant; `cargo update -p pinapod` cannot move the lockfile.

## Verification log

Completed successfully:

- `devenv shell -- security:pina-lint`
- `devenv shell -- cargo test -p pina --all-features --test adversarial_invariants` — 26/26
- `devenv shell -- cargo test -p pina --all-features --lib cpi --locked -q` — 25/25
- focused account derive validation suite — 28/28
- `devenv shell -- pina test --project examples/staking_rewards_program --filter rewards_accrue_once_per_index_and_release` — 1/1
- focused vesting and property-oracle Surfpool journeys
- multisig unit tests — 22/22
- multisig Mollusk SBF tests — 11/11
- `devenv shell -- cargo test --release --locked -p pina --test benchmarks -- --nocapture --test-threads=1` — 25/25
- `devenv shell -- security:audit` — completed with the documented allow-list
- `devenv shell -- security:npm-audit` — no moderate/high/critical advisory
- `devenv shell -- security:zizmor` — no finding under the configured default persona

Dependency residuals reported by RustSec but limited to the host test stack were `atty 0.2.14` (RUSTSEC-2021-0145), `memmap2 0.5.10` (RUSTSEC-2026-0186), and `rand 0.7.3` (RUSTSEC-2026-0097). They do not reach the shipped no-std runtime, but their allow-list entries still need owner, scope, and expiry review.

`devenv shell -- lint:all` reached the formatter gate and failed because a pre-existing untracked research Markdown file is not dprint-formatted. The audit did not modify that user file. Code compilation and earlier lint stages completed before that failure.

The final status of the longer workspace, IDL, fuzz-smoke, and Kani tiers is recorded in the handoff accompanying this report. A passing suite does not invalidate any finding whose path the suite does not exercise.

Continuation (second session) completed successfully:

- Full-source review of `pinapod` 0.4.1 and `pinapod-derive` 0.4.1 from the registry checkouts.
- `devenv shell -- cargo update --dry-run --offline -p pinapod` — demonstrates the 0.4.1 → 0.4.3 drift (SEC-33 evidence).
- Host contract suite (scratch crate, pinapod `=0.4.1`): 5/5 tests; 40,000 randomized patch operations; debug and release (release retaining overflow checks).
- `cargo miri test` on `nightly-2026-09-15-aarch64-apple-darwin` (400 steps per fuzzer plus all reject paths): 5/5, no undefined behavior, 2,054.9 s.
- `diff -ru` of pinapod 0.4.1 versus 0.4.3 runtime and derive sources (drift characterization behind SEC-33, SEC-34, and SEC-35).

## Residual risk and limits

- This was a source and local-harness audit, not a deployed-artifact audit.
- The continuation session covered the `pinapod` dependency itself (runtime, derive, drift surface), closing the first session's dependency gap; its dynamic verification ran on host targets and under Miri only. The contract suite was not executed on an SBF artifact, and pinapod's own upstream Kani proofs were not re-executed by the continuation — the first session's Kani tier status applies. WO-09 should route the in-repo successor suite through the existing Surfpool journey where practical.
- No live executable hash, ProgramData authority, cluster genesis hash, registry package, GitHub environment rule, tag protection rule, or secret scope was verified.
- Generated clients were covered through consistency/tests and targeted renderer review, not a manual line-by-line read of every generated file.
- SBF compute measurements were not available for every performance hypothesis. Items labelled measurement-required should not be sold as realized CU improvements until measured on optimized SBF artifacts.
- Example severities assume someone adapts or deploys the example with assets. The repository documents that several are teaching scaffolds, but the code and tests are still likely copy sources and should fail safely.
- The current checkout is not necessarily latest `main`. Rebase and re-check line numbers, workflow semantics, and finding applicability immediately before implementation.

This report does not certify the codebase as safe. It identifies reproducible failures and a prioritized route to reduce known risk at the audited commit.

## Re-verification at the 0.20.0 release HEAD (2026-09-23)

Re-verified revision: `37fc48b9` (`chore(release): prepare release (#459)`, workspace 0.20.0, pinapod 0.4.3 adopted by #485). Every finding above was re-tried against this tree, and each still-live finding now has a committed-style failing regression test that proves the exploit today and must pass once its fix lands. All tests use deterministic seeds per the benchmark conventions.

### The commit-message divergence (new finding, process-level)

Two merged security commits claim closures that are not in the code at this HEAD:

- #472 ("multisig ConfigExecute and VaultExecute enforce the expiry and stale-transaction guards"): only `VaultExecute` checks `is_expired`. `ConfigExecute` checks staleness but never expiry (`examples/multisig_program/src/lib.rs`, `ConfigExecuteAccounts` process), so **SEC-09 is live** for config proposals.
- #472 ("member removal prunes spending-limit rosters" + "SpendingLimitUse re-checks the live roster"): the prune exists, but `SpendingLimitUse` still authorizes against the static roster stored in the spending-limit account. Its safety comment (`examples/multisig_program/src/lib.rs`, above `prune_spending_limit_roster`) claims "SpendingLimitUseAccounts independently requires the drawer to be a current member of the multisig" — **no such check exists in the code**. A removal executed without the limit account in the remaining accounts leaves a removed member's allowance fully spendable. **SEC-08 is live** with a code/comment contradiction.
- #472 ("vesting … Cancel settles vested-but-unclaimed to the beneficiary"): `Cancel` returns the entire unclaimed balance to the administrator with no clock and no beneficiary settlement (`examples/vesting_program/src/lib.rs`, `CancelAccounts` process). **SEC-28 is live.**
- #483 ("escrow: new maker Cancel refunds the vault and closes both accounts"): the escrow instruction set at this HEAD is still exactly `Make = 1, Take = 2`. No `Cancel` exists anywhere in `examples/escrow_program/`. **SEC-31 is live.**
- #483 ("prop_amm: Update honours the oracle's stored authority"): `UpdateAccounts::process` still calls `assert_update_authority` (the immutable global `UPDATE_AUTHORITY`); `assert_oracle_authority` is used only by `RotateAuthority` itself. **SEC-32 is live.**

Whatever the intended end state of those PRs was, the squash that landed on main dropped these particular hunks while keeping the descriptions. Treat the five findings above as open work orders, and reconcile the commit-message claims before the 0.20.0 release ships.

### Status matrix at `37fc48b9`

Closed on main (verified in source and exercised by the merged suites):

- SEC-01 stake custody — deposits and withdrawals move real stake through the pool vault with observed-delta crediting; `assert_stake_backing` holds across the suite.
- SEC-10 timelock reductions releasing older vault proposals — `VaultExecute` rejects stale proposals.
- The vault half of SEC-09 — `VaultExecute` enforces expiry.
- SEC-35 — pinapod 0.4.3 carries the upstream wincode write-boundary fix (the feature remains disabled in this workspace).

Still live, each proven by a failing regression test below:

| Finding                          | Live proof (test fails today)                                                   |
| -------------------------------- | ------------------------------------------------------------------------------- |
| SEC-02                           | `audit_sec_02_stale_envelope_compact_update_is_rejected`                        |
| SEC-09 (config half)             | `audit_sec_09_an_expired_config_proposal_cannot_execute`                        |
| SEC-11                           | `audit_sec_11_unauthorized_first_initializer_cannot_capture_the_program_config` |
| SEC-12                           | `audit_sec_12_an_exit_zero_remote_command_is_not_deployment_evidence`           |
| SEC-16                           | `audit_sec_16_url_query_credentials_are_never_printed_or_persisted`             |
| SEC-18                           | `audit_sec_18_an_arbitrary_byte_payload_is_not_a_verification_transaction`      |
| SEC-20                           | `audit_sec_20_config_execution_refunds_closed_rent_to_the_configured_collector` |
| SEC-21                           | `audit_sec_21_creation_rejects_a_ttl_that_expires_before_the_timelock_elapses`  |
| SEC-22                           | `audit_sec_22_an_expired_proposal_can_be_closed_by_anyone`                      |
| SEC-26                           | `audit_sec_26_unapproved_first_initializer_cannot_capture_the_singleton_pool`   |
| SEC-27 (reserves, order, freeze) | the three `audit_sec_27_*` tests                                                |
| SEC-28                           | `audit_sec_28_cancellation_settles_vested_entitlement_to_the_beneficiary`       |
| SEC-29                           | `audit_sec_29_an_active_schedule_must_be_funded_at_initialization`              |
| SEC-32                           | `audit_sec_32_update_follows_the_rotated_authority`                             |

Still live, verified in source without a runtime test (see notes): SEC-08 (narrow execution window; the contradicting comment is the smoking gun), SEC-13/14/15/17/19 (CLI, host-boundary reproductions from the original report unchanged), SEC-23/24/25/33/34 (release/policy items; SEC-33's pinapod requirement is now a caret `"0.4.3"` at `Cargo.toml:273`, still not an exact pin), SEC-30 (below), SEC-31 (the missing instruction cannot be exercised at runtime; `examples/escrow_program/src/lib.rs:36-39` still declares only `Make = 1, Take = 2`).

### The failing exploit tests

Every test asserts the _secure_ behavior and fails on the current tree exactly where the exploit fires. Surfpool tests follow this repo's `#[ignore = "run with pina test"]` convention; CLI tests are `#[ignore]`d with the finding number as the reason.

Staking (`examples/staking_rewards_program/tests/surfpool/src/lib.rs`) — run with `pina test --project examples/staking_rewards_program --filter audit_sec_`:

- SEC-26: an attacker-funded `InitializePool` on the canonical pool succeeds (the `expect_err` fails with the success signature), proving singleton capture.
- SEC-27a: `SetRewardIndex` accepting 20 owed against a 10-token reward vault.
- SEC-27b: after that acceptance, the first claim drains the vault and the second equal claim fails at the token program.
- SEC-27c: `SetRewardIndex(u64::MAX)` is accepted over a 2,000,000,000,000-base-unit stake; the subsequent withdrawal fails with `Program arithmetic overflowed`, proving the frozen position. (A 10-unit stake does not overflow — `staked × u64::MAX / 1e12` fits `u64`; the freeze needs ≈10¹² base units.)

Vesting (`examples/vesting_program/tests/surfpool/src/lib.rs`) — `pina test --project examples/vesting_program --filter audit_sec_`:

- SEC-28: a fully elapsed, fully funded schedule is cancelled with nothing claimed; the beneficiary ATA receives 0 and the assertion demanding `TOTAL` fails.
- SEC-29: an elapsed schedule initializes with an empty vault and records `total_amount = 1_000_000_000`; the collateralization assertion fails on `0 in its vault`.

Multisig (`examples/multisig_program/tests/surfpool/src/lib.rs`) — `pina test --project examples/multisig_program --filter audit_sec_`:

- SEC-09: a config proposal with a 60-second lifetime executes after a time-travel far past expiry and raises the timelock.
- SEC-11: an attacker-funded `ConfigInitialize` occupies the global config PDA.
- SEC-20: a governed `RemoveSpendingLimit` closes the funded limit account; the rent refund lands on the executing member who named themselves rent payer, and the configured collector's balance is unchanged.
- SEC-21: a multisig is created with a 60-second TTL under a 3,600-second timelock.
- SEC-22: a proposal that expired while active cannot be closed; the close is refused (`custom program error: 0x8` = `InvalidProposalStatus`) and its rent stays stranded.

Prop AMM (`examples/prop_amm_program/tests/surfpool/src/lib.rs`) — `pina test --project examples/prop_amm_program --filter audit_sec_32`:

- SEC-32: after `RotateAuthority`, the rotated-in authority's `Update` is refused, proving the rotation never transfers publishing power.

CLI (`crates/pina_cli/tests/`) — `cargo test -p pina_cli --test deploy_command --test import_command --test verification_command -- --ignored audit_sec_`:

- SEC-12: a fake `solana` that only `exit 0`s produces a finalized publication receipt.
- SEC-16: the sentinel `PINA_AUDIT_SECRET_84f17` in the import URL query reaches CLI output and the generated README.
- SEC-18: 128 repeated `0x09` bytes, base64-encoded, are accepted and persisted as the exported verification transaction.

Core runtime (`security/regressions/sec02-compact-no-validation/`) — `cargo test --manifest-path security/regressions/sec02-compact-no-validation/Cargo.toml`:

- SEC-02: a standalone crate (deliberately outside the workspace) compiles `pina` with exactly `--no-default-features --features derive,compact,account-resize`, declares a two-version compact schema whose v0 and v1 share one physical shape, stamps a stale v0 envelope, and runs `UpdateResizableAccount`. The update succeeds and leaves the stale envelope in place, failing the `MigrationRequired` assertion. The crate carries its own `migrations/manifest.json` and pinned transition file; it is the executable form of WO-02's first step.

### SEC-30 harness blocker

The vesting Token-2022 extension test exists and asserts the right contract, but its mint fixture cannot be provisioned on the current Surfpool runtime: the harness's Token-2022 builtin rejects the canonical extension-init → `InitializeMint2` sequence with `InvalidAccountData` at `InitializeMint2` (identical failure for `MetadataPointer` and `NonTransferable`, and the harness's `install_historical_account` cheatcode refuses SPL-owned accounts). The code-level mismatch stands verified — `Initialize` performs no `assert_no_extensions` on the mint while `Claim` (`src/lib.rs:394`) and `Cancel` (`src/lib.rs:493`) reject every extension — and the test becomes executable as soon as either the builtin accepts the canonical sequence or the harness gains an SPL-account fixture API. The attempted wire encodings are preserved in the test's history and in the backups below.

### Working-tree incident during this session

At one point every tracked file edited in this session (the four Surfpool suites and the three CLI test files) was reverted to HEAD content in a single event (all mtimes `Sep 23 00:42`), while untracked paths were untouched. The actor was not identified: `pina test` does not rewrite these files (verified by a marker experiment), and the configured git hooks only format. The tests were re-applied and re-proven afterwards. Because the cause is unknown, canonical copies of every edited test file are preserved under `security/regressions/backups/`, and the incident itself deserves investigation — a tool that silently restores tracked files mid-session can eat any contributor's work.

## Remediation record (2026-09-23, `fix/audit-2026-09-22-findings`)

Implemented in the `worktrees/audit-fixes` work tree on top of the exploit-regression commit; every fix's failing test from this report now passes in that tree.

Fixed with their failing tests now passing:

- **SEC-02/SEC-34** — `UpdateResizableAccount` always routes through the generated account-level update contract (envelope checked before any byte moves, envelope advanced after), and the preflight-versus-commit length agreement is a release-mode `InvalidAccountData` instead of a compiled-out `debug_assert`. `security/regressions/sec02-compact-no-validation` passes; `cargo test -p pina --all-features --lib` and the cpi/compact suites are green.
- **SEC-33** — `pinapod` exact-pinned to `=0.4.3`, with the re-export contract made explicit and executable: downstream consumers import `pina::pinapod` and `pina::fixed` (behind the `fixed` feature) and never add either crate directly — proven by `security/regressions/sec33-downstream-reexport`, which builds a fixed-point schema against `pina` alone. pinapod does not re-export `fixed` itself, so `pina` keeps its matching `=1.30.0` entry (the only way Cargo permits the re-export); pina-rs/pinapod#41 tracks removing it.
- **SEC-09 config half** — `ConfigExecute` enforces expiry like `VaultExecute` (the earlier "divergence" note: only VaultExecute carried it on this line).
- **SEC-20** — `ConfigExecute` gained a `rent_collector` account; when the multisig configures one, closes and shrinks refund it (address-asserted), otherwise refunds alias the rent payer as before.
- **SEC-21** — multisig creation, `SetTimeLock`, and `SetProposalTtl` reject a nonzero TTL at or below the (new or current) timelock.
- **SEC-22** — `ProposalClose` takes the Clock and accepts terminal, expired, or stale proposals, refunding the configured collector.
- **SEC-27** — `SetRewardIndex` takes the reward mint, token program, and canonical reward vault; the aggregate liability `total_staked * index / SCALE` (in `u128`) must fit `u64` and be covered by the vault. All three audit tests pass, plus the two pre-existing journeys.
- **SEC-28** — `Cancel` takes the Clock and the beneficiary ATA, settles the vested-but-unclaimed entitlement to the beneficiary first, and refunds only the remainder to the admin.
- **SEC-29** — `Initialize` takes the admin's ATA and moves the full allocation into the vault in the same instruction; the unfunded schedule can no longer exist.
- **SEC-30** — `InitializePool` and vesting `Initialize` assert extension-free mints, matching every exit path. The executable proof is `initialize_rejects_a_token_2022_mint_with_extensions` in `tests/e2e.rs` (Mollusk installs arbitrary accounts); the Surfpool variant cannot run because its historical-account cheatcode only accepts program-owned fixtures — recorded as a harness gap.
- **SEC-16** — import URL credentials are redacted (`…#redacted`) from the echoed source, the provenance README, and the outcome; the HTTP request keeps the full URL.
- **SEC-18** — the exported verification payload must deserialize as a structurally valid legacy or versioned Solana transaction (bounded signature count, in-range account indices, non-empty instruction list) before anything is written. The three export unit tests now use valid wire fixtures.

Deferred to decisions, with issues:

- **SEC-11** — #501 (bootstrap mechanism for the multisig program config).
- **SEC-26** — #502 (staking pool initialization trust model).
- **SEC-12** — #503 (what on-chain evidence a deployment receipt requires).
- **SEC-13/14/15/17/19** — #504 (CLI host-boundary hardening batch).

The exploit-regression tests for SEC-11, SEC-26, and SEC-12 remain red in the tree and are the acceptance tests for those issues. All IDLs and clients for the touched examples were regenerated with `pina migrations sync`, and changesets are recorded for both batches.
