# Deep security audit — 2026-09-21

Audit date: 2026-09-21 · Base: `fix/migration-auto-schema-enum` @ `407545c0` (worktree branch `audit/solana-security-analysis`) · Method: solana-audit skill — three-layer review (operations / code / economics), systematic pass over the sealevel-attacks taxonomy, tooling pass, incident cross-check, and executable adversarial proofs for every finding that admits one.

Relation to prior audits: this review picks up where `deep-audit-2026-09-18.md` and its remediation (#457) left off. **Read the remediation status at the end of this document first**: the report was written against a base that did not include the maintainer's concurrent 2026-09-19 sweep (#472) or the Surfpool 1.6 harness migration (#482), so three findings (A1, A2, A10) describe gaps that were fixed independently before this change set landed. The vesting/staking economics were rewritten and the compact `with_pda` split landed since; the **multisig program (#466) has never been audited** and is a primary target here. Everything below marked _proven_ carries a committed, passing exploit test.

---

## Verdict

The framework core (`crates/pina`, `pina_macros`) remains strong, and the remediated vesting template is sound. The residual risk concentrates in four places:

1. **The staking template still ships a half-implemented value path, in the new code**: `Deposit` records stakes that were never transferred while `Claim` pays real tokens — an executable phantom-stake harvest (the exact H2 hazard class the 2026-09-18 remediation was meant to end, reborn in the rewrite).
2. **The multisig program's spending-limit surface breaks the membership invariant**: a member removed by full governance keeps drawing from vaults, proven end-to-end through the real removal flow on SBF.
3. **The fixed-account PDA loader family has no canonical-bump option at all** — the asymmetry left behind when the compact family gained `with_checked_pda`.
4. **Discriminators remain author-chosen integers with no cross-enum uniqueness**, leaving the sealevel-attacks type-cosplay class open to a one-line program-author mistake that Anchor-class hashing prevents by construction.

---

## Findings summary

| ID  | Severity            | Area                               | Summary                                                                                                                                                 | Proof                                       |
| --- | ------------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| A1  | High (template)     | `examples/staking_rewards_program` | Phantom deposits harvest real rewards: `Deposit` moves no tokens, `Claim` pays from the reward vault                                                    | `tests/audit_adversarial.rs` ✅ executed    |
| A2  | Medium              | `examples/multisig_program`        | Removed members keep spending through stale spending-limit rosters                                                                                      | `tests/audit_adversarial.rs` ✅ executed    |
| A3  | Low-Medium          | `examples/multisig_program`        | `MultisigImport` adopts any fabricated account/roster (no PDA or provenance binding)                                                                    | `tests/audit_adversarial.rs` ✅ executed    |
| A4  | Medium (downstream) | `pina_macros` fixed accounts       | `load_pda`/`load_pda_mut` accept noncanonical shadow PDAs; no `with_checked_pda` equivalent exists for fixed schemas                                    | `crates/pina/tests/audit_adversarial.rs` ✅ |
| A5  | Medium (downstream) | `pina_macros` discriminators       | Cross-enum discriminator collisions permit type cosplay through `as_account`                                                                            | `crates/pina/tests/audit_adversarial.rs` ✅ |
| A6  | Medium (deployment) | `examples/multisig_program`        | Permissionless global `ConfigInitialize`: first initializer captures authority, treasury, and creation fee                                              | repro = existing e2e test                   |
| A7  | Low                 | `examples/staking_rewards_program` | Permissionless pool init per mint-pair: first initializer becomes pool admin                                                                            | trivial from source                         |
| A8  | Low                 | `examples/multisig_program`        | Controlled multisigs: the config authority can move vault funds with no timelock via the spending-limit route                                           | from source                                 |
| A9  | Low (design)        | `examples/multisig_program`        | `VaultExecute`'s protected set is only `[multisig, this proposal]`; other governed accounts are writable in inner CPIs                                  | from source                                 |
| A10 | Low                 | `examples/vesting_program`         | `Cancel` claws back vested-but-unclaimed tokens (works pre-cliff); `validate_schedule` accepts empty/elapsed windows its own error doc claims to reject | from source                                 |
| A11 | Low                 | `examples/escrow_program`          | No maker cancel/refund: maker funds are stranded until a taker appears                                                                                  | from source                                 |
| A12 | Low                 | workspace                          | No `overflow-checks = true` in any profile; release/SBF builds wrap silently for derived programs (examples themselves use checked math)                | `Cargo.toml` has no `[profile]` section     |
| A13 | Low (supply chain)  | `pina_cli`                         | Runtime `npx` fetch-and-execute without integrity pinning; lint-driver checksum is same-origin; 1 low npm advisory passes the gate                      | tooling pass                                |
| A14 | Informational       | `examples/prop_amm_program`        | `RotateAuthority` is cosmetic for `Update` (hard-coded benchmark key); oracle is a keypair account, not a PDA                                           | from source                                 |
| A15 | Informational       | `pina` / docs                      | Optional-account program-id filler skips constraint sets (documented); immutable `take_remaining` unchecked (documented)                                | from source                                 |

No Critical findings. The on-chain runtime crate (`crates/pina/src`) has no new findings; the 2026-09-18 verdict ("production-grade core") stands after this re-review.

---

## Detailed findings

### A1 — Staking: phantom deposits harvest real rewards, High (template), proven

- **Location:** `examples/staking_rewards_program/src/lib.rs:378-463` (`Deposit`), `:543-649` (`Claim`), `:637-645` (the payout `TransferChecked`).
- **Mechanism:** `Deposit` updates `staked_amount`, `total_staked`, and the reward checkpoint, then only ensures the user's _stake ATA exists_ (`CreateIdempotent`). **No stake tokens move anywhere** — there is no vault, no transfer, no balance check. `Claim` pays `pending + staked × Δindex` in **real reward tokens** from the pool's reward vault, signed by the pool PDA. Reward entitlement is therefore proportional to a number the user typed, not to tokens delivered.
- **Exploit (executed on SBF, committed as `phantom_deposit_harvests_real_rewards`):** open position → `Deposit(1_000_000)` with an empty stake ATA → admin raises the index to `REWARD_INDEX_SCALE` → `Claim`. Result asserted: attacker's reward ATA receives exactly `1_000_000` real tokens; the reward vault drops by the same; the attacker's stake ATA held zero throughout. Honest stakers' rewards are diluted by the phantom stake.
- **Acknowledgment status:** the readme lists "transfers into and out of the PDA-controlled stake vault" as deliberately out of scope — but the shipped _combination_ (unbacked stakes on the input side, real payouts on the output side) is precisely the "half-implemented value path" the 2026-09-18 audit flagged as the most dangerous template shape (its H2). The rewrite fixed the payout side and left the intake side open.
- **Severity:** High for any derived program; in-tree it is a documented scaffold.
- **Fix:** transfer stake tokens into a pool-owned stake vault on `Deposit` (and back on `Withdraw`), measure by vault delta like the escrow template does, and assert solvency (`vault balance ≥ Σ staked`) on every payout. Until then, rename the example or strip the reward side — the current shape teaches the Solend/Mango unbacked-entitlement class.
- **Incident class:** unbacked entitlement accounting (Solend 2021 reserve accounting; Mango bad-debt generation).

### A2 — Multisig: removed members keep spending, Medium, proven

- **Location:** `examples/multisig_program/src/lib.rs:2949-2968` (`SpendingLimitUse` membership check), `:1683-1694` (`RemoveMember` action), `:2769-2773` (what a consensus change invalidates).
- **Mechanism:** `SpendingLimitUse` checks the signer only against the spending limit's own frozen `members` roster. `RemoveMember` rewrites the multisig roster and bumps `stale_transaction_index` (invalidating _proposals_), but nothing touches spending-limit rosters. A removed member therefore retains vault-drawing rights until someone separately notices and removes each limit account.
- **Exploit (executed on SBF, committed as `removed_member_still_spends_through_spending_limit`):** two-member threshold-one multisig → config proposal `RemoveMember(leaving)` → activate → approve → `ConfigExecute` (assert: roster no longer contains the member, stale index bumped) → the removed member signs `SpendingLimitUse` and moves lamports out of the vault. Asserted: destination receives the funds.
- **Severity:** Medium — requires the multisig to remove a member (a normal, realistic operation) while a limit naming them still exists; loss bounded by the limit amount but repeatable per period.
- **Fix:** in `SpendingLimitUse`, additionally require `multisig.is_member(member)` (the `MultisigSnapshot` is already loaded for the PDA check — one lookup away), and/or make `RemoveMember` fail or invalidate limits that still name the removed key.
- **Incident class:** retained-access authority lifecycle (Pump.fun 2024 ex-employee authority; Drift 2026 "consent that outlived its context").

### A3 — Multisig import adopts fabricated rosters, Low-Medium, proven

- **Location:** `examples/multisig_program/src/lib.rs:2013-2015` (owner check against the caller-supplied program; discriminator also caller-supplied), `:1445` vs `:2022` (the legacy account's parsed `create_key` is discarded in favor of the signer's).
- **Mechanism:** nothing binds the legacy account to any derivation — not to its own program's PDA space, not to the parsed `create_key`, not to a known legacy program id. Any account of any program whose bytes parse as the legacy layout imports "successfully", and the fabricated roster becomes the new multisig's membership verbatim.
- **Exploit (executed on SBF, committed as `multisig_import_adopts_a_fabricated_legacy_roster`):** an arbitrary account owned by an attacker-chosen program, carrying a roster that includes a key the attacker does not control (a "famous" key), imports into a multisig seeded by the attacker's signing key. Asserted: the famous key is a full-permission member of the imported multisig.
- **Severity:** Low-Medium — no direct theft (the result is equivalent to `MultisigCreate`, and the creation fee is still charged), but any UI, indexer, or downstream program that treats an import as provenance ("this multisig continues that legacy one") is trusting a fully forged document. The reference Squads design derives the legacy PDA and checks the address.
- **Fix:** pin the accepted legacy program ids (or derive-and-compare the legacy address from the parsed `create_key` under the legacy program), and reject imports whose legacy `create_key` does not sign.
- **Incident class:** provenance spoofing / type cosplay of external state (Wormhole guardian-set confusion; Cashio collateral cosplay).

### A4 — Fixed-account PDA loaders accept shadow PDAs; no canonical option exists, Medium (downstream), proven

- **Location:** `crates/pina_macros/src/pda.rs:216-275` (`load_pda`/`load_pda_mut` verify only the address the stored bump derives), vs the compact family's `with_checked_pda` (`:335-365`) added in the M1 remediation.
- **Mechanism:** a fixed `#[pda]` schema's only generated loaders accept any address the stored bump derives — including a shadow account created at a noncanonical bump whose stored bump field matches that bump. The compact family has a canonical-search loader; **the fixed family has none**, so a program whose seeds do not bind a required signer has no way to prove the namespace is unique. Creation-side canonicality (`CreateProgramAccountWithBump` rejecting noncanonical bumps) is the only barrier, and `CreateProgramAccountWithUncheckedBump` is a first-class escape hatch.
- **Exploit (executed as a host test, committed as `fixed_account_load_pda_accepts_a_noncanonical_shadow_pda` + `fixed_account_shadow_and_canonical_load_through_the_same_call`):** a fixture seeds pair with a valid noncanonical bump; the shadow account loads through `load_pda`, and both the canonical and shadow accounts load through the same call — the loader cannot distinguish them.
- **Severity:** Medium for the framework product (documented behavior, creation-side enforcement keeps in-repo programs safe); the missing canonical loader is the actionable gap.
- **Fix:** generate `load_checked_pda`/`load_checked_pda_mut` for fixed schemas mirroring `with_checked_pda` (canonical search + stored-bump equality), and lint-load `load_pda` in handlers whose seeds do not bind a required signer — the same lint M1 recommended for `with_pda`.
- **Incident class:** sealevel-attacks #7 (bump canonicalization; Crema's fake tick array).

### A5 — Cross-enum discriminator collisions permit type cosplay, Medium (downstream), proven

- **Location:** `crates/pina_macros/src/discriminator.rs` (integer discriminants, no namespacing, no cross-enum registry); no rule in `pina_lints` covers it.
- **Mechanism:** discriminators are author-chosen `u8`–`u64` values. rustc rejects duplicates _within_ one enum, but two different account enums may declare the same value; when the two account types also agree on serialized size, `as_account::<T>` — owner check, discriminator check, exact-size check, all passing — accepts either account for both types. Nothing in the macro or the 20-lint catalog detects the collision.
- **Exploit (executed as a host test, committed as `cross_enum_discriminator_collision_permits_type_cosplay`):** a `VaultLedger` and an `AdminRegistry` behind two enums sharing value 31 with identical width — the vault bytes load as the registry, reading the attacker's key as `admin`. Control test confirms a different-size collision is rejected.
- **Severity:** Medium for the framework product (requires a program-author mistake, but the framework's job is to make the class impossible — Anchor's sha256-prefixed discriminators do); in-repo programs are unaffected (each uses one enum per program).
- **Fix:** a compile-time cross-enum registry (the entrypoint macro already sees the program's account set), or namespaced discriminators in the next breaking window; a `pina_lints` rule flagging two account-discriminator enums with intersecting values is the cheap 80%.
- **Incident class:** sealevel-attacks #3 (type cosplay; Cashio).

### A6 — Permissionless global config initialization, Medium (deployment)

- **Location:** `examples/multisig_program/src/lib.rs:1864-1889`.
- **Mechanism:** `ConfigInitialize` is open to any signer; the first initializer becomes the global `authority` and sets `treasury` and `creation_fee`. A sniper racing the deployment captures all three: every subsequent `MultisigCreate` pays the attacker's fee into the attacker's treasury, and only the attacker can update the config (or refuse to, gating all future creations behind fee changes).
- **Repro:** the committed e2e test `config_initialize_writes_the_global_pda` already proves an arbitrary key initializes the PDA — the race is that test run by an attacker first.
- **Fix:** initialize the config in the same transaction as the program deployment, or gate first init to a hard-coded deployer key that immediately rotates. Document the race in the example readme; this is the Audius init-front-run class.

### A7 — Permissionless staking pool initialization, Low

First initializer per `(stake_mint, reward_mint)` pair becomes pool admin and controls `SetRewardIndex` (`staking_rewards_program/src/lib.rs:247-324`). Same front-run class as A6, scoped to one pool; the canonical-bump creation comments show the authors considered shadow pools but not admin capture.

### A8 — Controlled multisigs: instant, timelock-free fund movement by the config authority, Low

`ConfigAuthorityExecute` (`multisig_program/src/lib.rs:2879-2916`) applies any validated action stream immediately, and `AddSpendingLimit` is such an action — so a controlled multisig's single `config_authority` key can mint itself a spending limit and drain vaults with no consensus and no timelock. This is arguably "what controlled mode means", but fund movement via the _config_ path deserves at least a doc warning; autonomous multisigs gate the same action behind threshold + timelock.

### A9 — VaultExecute protected set, Low (design)

`protected = [multisig_key, proposal_key]` (`multisig_program/src/lib.rs:2619`). Other proposals of the same multisig, spending-limit accounts, and the global program config may be passed writable into inner CPIs (nested execution of another already-approved proposal is reachable; a message can also invoke this program itself). Every nested path found still requires its own signer or prior threshold approval, so no exploit was constructible — but the invariant is implicit; new instructions added later inherit it silently. Mirrors the Squads reference design; document it.

### A10 — Vesting cancel clawback + schedule doc/code mismatch, Low

`Cancel` refunds the entire remaining vault balance (`vesting_program/src/lib.rs:463-476`) — clawing back the beneficiary's vested-but-unclaimed entitlement, and working pre-cliff. Issuer power, plausibly intended, but a copier expecting clawback-free vesting inherits it. Separately, `validate_schedule` (`:162-168`) accepts `start == cliff == end` and fully-elapsed schedules while `InvalidSchedule`'s doc (`:62`) claims both are rejected.

### A11 — Escrow has no maker cancel, Low

Only `Make` and `Take` exist; the maker's token A is locked until some taker appears (`escrow_program/src/lib.rs`). Availability, not theft; the template propagates it.

### A12 — No overflow-checks profile anywhere, Low

The root `Cargo.toml` has no `[profile]` section; no member can define one (cargo ignores member profiles). Release/SBF builds therefore wrap silently on overflow for _derived_ programs. All example math is `checked_*`/u128 (verified by sweep) and the `require_checked_asset_arithmetic` lint exists — but the Cetus-class guardrail costs one line: add `[profile.release] overflow-checks = true` to the root (examples build through it) and recommend it in the framework docs for downstream workspaces.

### A13 — Host tooling supply-chain residue, Low

Carried context, re-verified this pass: `pina generate`/`pina cpi` fetch npm packages at runtime via `npx -y -p <pinned-version>` (version-pinned, no integrity/lockfile pinning — registry compromise is developer-machine RCE); the lint-driver download verifies a sha256 that shares its origin with the artifact (env-overridable), so it guards transport corruption, not a hostile origin; `verify:security` passes with 1 low npm advisory; `RELEASE_TOKEN`'s scope is undocumented in-repo; SECURITY.md still lists the personal Gmail as a fallback channel (known L9). CI posture remains excellent (SHA-pinned actions, `persist-credentials: false`, no `pull_request_target`, OIDC publishing, zizmor zero-findings).

### A14 — prop_amm authority split, Informational

`Update` is gated on the hard-coded benchmark key `UPDATE_AUTHORITY` (`prop_amm_program/src/lib.rs:37-42, 105-111`); `RotateAuthority` updates a stored field `Update` never consults — cosmetic for price publication. Deliberate Anchor-benchmark port; flag as a template trap (remove `RotateAuthority` or wire it up).

### A15 — Documented escape hatches, Informational

Optional-account slots treat the program id as a filler, skipping the entire constraint set for that slot (documented, `crates/pina/src/traits.rs:1037-1041, 1112-1114`); immutable `#[pina(remaining)]` performs no checks (`take_remaining`); `require_explicit_discriminators_and_seed_namespaces` covers naming, not the A5 collision class.

---

## What was verified sound (so the next audit doesn't re-derive it)

- **Multisig consensus core** (never audited before): bitmask voting over a sorted unique roster, threshold/rejection-cutoff arithmetic, `stale_transaction_index` invalidation on consensus changes, timelock anchoring at approval, expiry, revoke-to-active with fresh timelock, ephemeral per-proposal signers, message account/key/writability/signer matching, `invoke_signed_with_bounds` privilege handling, protected-account rejection, `ProposalClose` rent collection. No finding beyond A2/A3/A6/A8/A9.
- **Remediated vesting template**: cliff gating via Clock, floored u128 vesting math, ATA re-derivation on both payout paths, solvency check, cancel-refund atomicity.
- **Escrow `Take`**: taker cannot receive token A without paying exactly `amount_b`; vault re-derived as the escrow's ATA; `TransferChecked` throughout; `assert_no_extensions` on mints; close flows zeroed.
- **Framework**: owner-before-deref ordering, exact-size checks, compact codec (Kani-proven, unchanged), CPI program pinning (`Program::try_new`, `assert_program`), lamport conservation primitives, `close_account_zeroed`, `UpdateResizableAccount` grow-before-write — re-spot-checked, consistent with the 2026-09-18 verification.
- **Tooling pass**: `verify:security` passes (cargo-deny advisories with the documented ignore list, zizmor clean, gitleaks clean, npm audit 1 low); `cargo clippy` clean on all touched packages.

---

## Adversarial suite committed in this branch

| Test                                                                                                       | Target                                                          | Tier                                      |
| ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | ----------------------------------------- |
| `crates/pina/tests/audit_adversarial.rs` — 4 tests                                                         | A4 (shadow PDA + ambiguity), A5 (type cosplay + length control) | host unit                                 |
| `examples/staking_rewards_program/tests/audit_adversarial.rs::phantom_deposit_harvests_real_rewards`       | A1                                                              | mollusk + SBF, real `spl_token`/`spl_ata` |
| `examples/multisig_program/tests/audit_adversarial.rs::removed_member_still_spends_through_spending_limit` | A2 (full governed removal → draw)                               | mollusk + SBF                             |
| `examples/multisig_program/tests/audit_adversarial.rs::multisig_import_adopts_a_fabricated_legacy_roster`  | A3                                                              | mollusk + SBF                             |

Reproduction:

```sh
# framework proofs (host)
devenv shell -- cargo test -p pina --test audit_adversarial

# program exploits (SBF)
devenv shell -- cargo-build-sbf --skip-tools-install --tools-version v1.54 \
  --manifest-path examples/staking_rewards_program/Cargo.toml \
  --sbf-out-dir target/deploy --features bpf-entrypoint
devenv shell -- cargo-build-sbf --skip-tools-install --tools-version v1.54 \
  --manifest-path examples/multisig_program/Cargo.toml \
  --sbf-out-dir target/deploy --features bpf-entrypoint
# stage spl_token.so / spl_ata.so into target/deploy (see vesting e2e docs)
devenv shell -- env SBF_OUT_DIR="$PWD/target/deploy" \
  cargo test -p staking_rewards_program -p multisig_program \
  --test audit_adversarial -- --include-ignored
```

All seven tests pass on this machine (2026-09-21). The three SBF tests are `#[ignore]`d following the examples' convention and skip gracefully when artifacts are absent.

---

## Incident cross-check (Phase 6)

| Finding                    | Nearest historical incident                                                 |
| -------------------------- | --------------------------------------------------------------------------- |
| A1 phantom stake           | Solend reserve accounting / Mango unbacked entitlement                      |
| A2 removed member spends   | Pump.fun 2024 (retained authority), Drift 2026 (consence outliving context) |
| A3 import forgery          | Wormhole guardian-set spoofing class, Cashio collateral cosplay             |
| A4 shadow PDA              | Crema 2022 (bump/provenance, sealevel #7)                                   |
| A5 discriminator collision | Cashio 2022 (type cosplay, sealevel #3)                                     |
| A6/A7 init front-run       | Audius CPIMP class                                                          |
| A12 overflow profile       | Cetus 2025 ($223M silent wrap)                                              |
| A13 supply chain           | Web3.js 2024 / Slope 2022 (developer-machine and key-material hygiene)      |

---

## Coverage, limits, and residual risk

Reviewed: all of `examples/multisig_program` (first audit), the rewritten staking/vesting/escrow money paths, the fixed-account loader/discriminator surface of `pina_macros`, `pina_lints` catalog, CI/release/ops layer (delta since 2026-09-18), `verify:security` tooling pass, and the seven-test adversarial suite above.

Run and passing on this machine: `verify:security`, focused `cargo test` for `pina` adversarial suites, staking + multisig e2e suites (`--include-ignored`, SBF), `cargo clippy` on touched packages, `dprint fmt`.

Not run in this pass (known-unverified, per the verify-before-push rule): `test:all` / `coverage:all` (full workspace + patch-coverage gate), the Surfpool example suites, the performance benchmark tier, Kani profiles, and `lint:all` beyond the touched packages. The branch adds only tests and this document — no shipped-crate code changes — so the unverified tiers are unlikely to be affected, but they remain unproven.

Residual risk: the multisig program's 3,800 lines received one deep pass plus targeted adversarial probes, not the two-pass cross-check the 2026-09-18 audit gave the core; A9's nested-CPI surface in particular deserves a dedicated stateful-fuzz campaign (the repo's `pina_fuzz` rig is the natural home).

---

---

## Remediation status (2026-09-21)

The findings were re-checked against `main` before remediation. Three had already been closed independently by the maintainer's own 2026-09-19 sweep (`fix(security): close the confirmed 2026-09-19 sweep findings`, #472) and by the Surfpool 1.6 harness migration (#482); the remaining items are fixed in this change set.

| Finding                                    | Status                                                                                                                                                                                         | Where                                         |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| A1 staking phantom deposits                | **Closed on main** (#472) — deposits and withdrawals move real stake tokens through the pool vault, delta-measured                                                                             | `examples/staking_rewards_program/src/lib.rs` |
| A2 removed members keep spending           | **Closed on main** (#472) — `SpendingLimitUse` re-checks the live roster                                                                                                                       | `examples/multisig_program/src/lib.rs`        |
| A3 fabricated imports                      | **Fixed here** — the legacy account must sit at the PDA its parsed `create_key` derives under the legacy program, and that key's holder must sign the import (new `legacy_create_key` account) | `examples/multisig_program/src/lib.rs`        |
| A4 fixed-account shadow PDAs               | **Fixed here** — `load_checked_pda` / `load_checked_pda_mut` generated for fixed schemas, mirroring the compact family's `with_checked_pda`                                                    | `crates/pina_macros/src/pda.rs`               |
| A5 discriminator collisions                | **Fixed here** — new deny lint `deny_colliding_account_discriminators` (account namespace only, width-aware), with UI fixtures                                                                 | `crates/pina_lints/src/lints/`                |
| A6 permissionless config init              | **Documented here** — deployment note in the module header and readme; the create remains first-writer-wins by design                                                                          | `examples/multisig_program/`                  |
| A7 permissionless pool init                | **Documented here** — readme notes the announcement front-run and same-transaction initialization                                                                                              | `examples/staking_rewards_program/readme.md`  |
| A8 config-authority fund movement          | **Fixed here** — `AddSpendingLimit` rejected on the authority path with `SpendingLimitRequiresProposal`                                                                                        | `examples/multisig_program/src/lib.rs`        |
| A9 VaultExecute protected set              | **Fixed here** — all program-owned writable non-signer message accounts are protected                                                                                                          | `examples/multisig_program/src/lib.rs`        |
| A10 vesting cancel clawback + schedule doc | **Closed on main** (#472) — Cancel settles vested-but-unclaimed to the beneficiary; Initialize rejects empty and elapsed windows                                                               | `examples/vesting_program/src/lib.rs`         |
| A11 escrow maker cancel                    | **Fixed here** — new `Cancel` instruction refunds and closes both accounts                                                                                                                     | `examples/escrow_program/src/lib.rs`          |
| A12 overflow-checks profile                | **Deliberately not applied** — measured and rejected; see the note below                                                                                                                       | `Cargo.toml` (unchanged)                      |
| A13 supply-chain residue                   | **Hardened/documented here** — lint-driver env overrides marked operator-only; the npx runtime fetch remains version-pinned by design (registry trust)                                         | `crates/pina_cli/src/lint_driver.rs`          |
| A14 prop_amm authority split               | **Fixed here** — `Update` accepts the stored authority alongside the benchmark key, so `RotateAuthority` is meaningful                                                                         | `examples/prop_amm_program/src/lib.rs`        |
| A15 documented escape hatches              | No action — documented behavior, unchanged by design                                                                                                                                           | —                                             |

---

## Note on A12: why the overflow-checks profile was measured and rejected

The finding recommended `[profile.release] overflow-checks = true` as the workspace-wide backstop for unchecked arithmetic (the Cetus class). It was implemented, then reverted after the performance tier measured it.

The setting inserts a check on every arithmetic and cast operation in every release build, so it raises compute units on _every_ program — not only the ones this change set touches. The 2026-09-21 CI run measured 72 blocking instruction-CU regressions: small, uniform increases across the whole example inventory, including programs with no source changes at all (`counter_program/initialize` +21 CU, `declare_id_program/initialize` +7 CU, `duplicate_mutable_accounts_program/*` +7 CU, and so on), with three incidental improvements. The repository's own policy treats every measured increase as negative, and a blanket CU tax on every derived program is not a cost this finding justifies.

The residual risk the setting would have covered is already addressed at the source: `require_checked_asset_arithmetic` is a deny-level lint that rejects unchecked arithmetic on asset paths, and every value-bearing path in the workspace uses `checked_*` or `u128` intermediates (verified by the earlier audit pass). The recommendation for downstream programs stands and is stated plainly here rather than enforced globally: **a program that adds unchecked arithmetic should enable `overflow-checks = true` in its own root manifest**, where it pays that cost only for its own code.

Verification of the revert: `hello_solana_program` (which contains no `#[pda]`, no accounts, and no source change in this branch) built to **5,656 bytes at both `aa81c8d1` and this branch head**, byte-identical, after the profile was dropped. The same program measured 7,568 bytes under the profile, which is the uniform +1,912 B the failing report showed across the inventory.
