# Dynamic authentication attacks — multisig_program and role_registry_program (2026-09-19)

Red-team execution report. Both targets were deployed as **real SBF artifacts** (`target/surfpool/examples/{multisig_program,role_registry_program}.so`) against the offline Surfnet via `pina_test::ProgramTest::start_with_artifact`. Every scenario below was actually executed tonight by the harness in `tmp/sweep/auth-attacks/` (standalone crate; run with `devenv shell -- cargo test --manifest-path tmp/sweep/auth-attacks/Cargo.toml -- --ignored --nocapture --test-threads=1`; final full-suite run: **7 passed, 0 failed**).

- `multisig_program` id `5BeQ7VMZHYdnUD6PyrMd29WQo2DLfo7N2NDXCDQZ5MQc`
- `role_registry_program` id `3B7roNNQLnW43Par9AfTuVzEqZx7yPtXRA9K3Ev7RHyX`

Line numbers refer to `examples/multisig_program/src/lib.rs` and `examples/role_registry_program/src/lib.rs` in this worktree.

Labels: **CONFIRMED** = reproduced against the deployed artifact this sweep; **SUSPECTED** = reasoned from source/framework code, not separately executed. Nothing in this file duplicates the known items in `security/deep-audit-2026-09-18.md` (L10 RotateAdmin one-step rotation is re-verified only as cheap evidence, marked as such).

---

## Wire formats used (for reproduction)

Instruction data is `[discriminator, version_byte(0), payload…]` (the migration envelope adds a version byte even to zero-field instructions). Multisig account payload (envelope 2 bytes, compact header 126): `threshold@99..101`, `timelock@101..105`, `ttl@105..109`, `transaction_index@109..117`, `stale_transaction_index@117..125`, roster length u16 `@125..127`, roster bytes `@128..`. Proposal payload (header 106): `index@67..75`, `status@78`, `status_at@79..87`, `expires_at@87..95`, `approved_mask@95..99`. Config actions: `[u8 count]` then tagged payloads, e.g. `SetTimeLock` = `[01, 03, seconds:u32le]`, `RemoveMember` = `[01, 01, key:32]`, `AddSpendingLimit` = `[01, 06, create_key:32, vault_index:u8, mint:32,
amount:u64le, period:u8, members_len:u8, members…, dest_len:u8, dests…]` (must fit the 128-byte `actions` proposal cap).

---

## E1 — ConfigExecute skips the proposal-expiry guard (multisig) — CONFIRMED

**Location:** `examples/multisig_program/src/lib.rs:2806-2877` (`impl ProcessAccountInfos for ConfigExecuteAccounts`). Every other time-sensitive path checks `is_expired(proposal.expires_at, now)` — ProposalActivate `:2329`, ProposalApprove `:2362`, ProposalReject `:2414`, and the sibling executor VaultExecute `:2566` — but ConfigExecute checks only kind, `STATUS_APPROVED`, staleness (`:2833`), and the timelock (`:2840`).

**Mechanism:** `expires_at` is stamped at proposal creation (`now + ttl`); the TTL exists so approved consent cannot linger forever. Because the execution path for config proposals never re-checks expiry, an APPROVED config proposal becomes executable at any arbitrary later time, while an identically-aged vault proposal correctly dies with `ProposalExpired`.

**Exploit (reproduced; M3):**

1. Create a threshold-1 multisig, timelock 0, ttl 600.
2. `ProposalCreate` a `KIND_CONFIG` proposal carrying actions `[01 03 2A 00 00 00]` (`SetTimeLock{42}`); `expires_at = t₀+600` is stored.
3. `ProposalActivate` + `ProposalApprove` → `STATUS_APPROVED`.
4. `time_travel_to_timestamp_millis((t₀+600+3600)*1000)` — one hour past expiry.
5. `ConfigExecute` (data `[0B, 00]`, accounts `multisig/mut, proposal/mut,
   member/ro+signer, rent_payer/w+signer, system, clock`) → **transaction succeeds**; multisig `timelock` now reads `42`, proposal `STATUS_EXECUTED`. Harness log: `[M3] EXPLOITED: config proposal executed 3600s past expiry`.

**Severity:** Low-to-Medium. The payload itself was legitimately approved at threshold, so this is not a threshold bypass; the impact is that a consent-freshness control (proposal TTL, which the module doc advertises as "stale consent cannot linger forever") is silently unenforced on the path that rewrites governance. Combined with E2 the consent is both unrevokable and unbounded in time. For a treasury whose members approved a config change under conditions that later changed (e.g., pending member exit), stale execution is a real governance hazard, hence not Informational.

**Fix:** add the same guard to `ConfigExecute` after the timelock check:

```rust
if is_expired(proposal.expires_at, now) {
    return Err(MultisigError::ProposalExpired.into());
}
```

plus a regression test executing an expired config proposal (expect `ProposalExpired`). Consider also documenting that `ProposalCancel` and `ProposalClose` intentionally lack the guard (harmless transitions).

---

## E2 — VaultExecute skips the stale-transaction guard: frozen, re-bound, unrevokable consent (multisig) — CONFIRMED

**Location:** `examples/multisig_program/src/lib.rs:2537-2753` (`VaultExecuteAccounts::process`). `load_member_proposal` returns `stale_transaction_index`, and every other member instruction enforces `proposal.index <= stale_transaction_index` (`:2325`, `:2357`, `:2409`, `:2464`, and ConfigExecute `:2833`) — VaultExecute alone does not; a comment at `:2552-2553` says this is deliberate ("the recorded consent already met the old threshold"). Two interactions make the deliberate choice unsafe:

- `RemoveMember` compacts the roster with `copy_within` (`:1688-1691`), but the proposal's `approved_mask` bits are **roster positions**, so after removal the recorded approvals re-bind to different members.
- `ProposalRevoke` is refused on stale proposals (`:2464`), so once a consensus change lands, nobody — not even an approver who is still a member — can revoke; the approval is frozen while the proposal stays `STATUS_APPROVED` (and if `ttl = 0`, it never expires).

**Exploit (reproduced; M4):**

1. Threshold-2 multisig, members sorted `[A, B, C]`, all `PERMISSIONS_ALL`, timelock 0, ttl 0. Fund the vault.
2. Vault proposal V1 (index 1) paying the vault to an attacker address; `Activate` + `Approve(A)` + `Approve(B)` → `approved_mask = 0b011`, `STATUS_APPROVED`.
3. Config proposal V2 (index 2) with actions `[01 01 <B:32>]` (`RemoveMember(B)`), approved by A + C and executed via `ConfigExecute`. Multisig now stores `stale_transaction_index = 2` and roster `[A, C]`.
4. `ProposalRevoke` by A on V1 → **blocked**, `StaleProposal` (custom 9). The on-chain proposal still shows `status = 2 (APPROVED)`, `approved_mask = 0b011` — position 1 now names C, who never approved V1.
5. `VaultExecute` V1 by C → **succeeds**; the vault pays out. Harness log: `[M4] EXPLOITED: V1 executed although index 1 <= stale_transaction_index 2`.

**Severity:** Medium. The stated rationale ("consent met the old threshold") does not hold after the mask re-binds: the executed consent is attributed to members who never gave it, and the configured invalidation mechanism (`stale_transaction_index`, advertised in the module doc as "stale-transaction invalidation on consensus changes") is skipped exactly where money moves. The practical attack needs a threshold-legal roster change first, so it is not a stranger exploit — but it defeats the veto/revocation window that the rest of the design carefully maintains, and it makes approved vault payloads unrevokable and (with ttl 0) immortal.

**Fix (either side closes it):**

- Make VaultExecute enforce staleness too and invalidate `STATUS_APPROVED` proposals on consensus change (e.g., `commit_multisig` writes `stale_transaction_index`; VaultExecute then rejects them like every other path), or
- If "consent outlives roster edits" is the product decision, store the approvals as a **set of member keys** (or key→bit map frozen at approval time) instead of positional mask bits, and keep revocation available for stale proposals by key. Also re-evaluate `approved_mask`/status on `commit_multisig` so identity drift cannot accumulate silently.

Add a regression test mirroring M4 (approve → remove an approver → expect either `StaleProposal` on execute or correctly re-bound, revokable consent).

---

## E3 — A removed member keeps drawing on the spending limit (multisig) — CONFIRMED

**Location:** `examples/multisig_program/src/lib.rs:2955` (`SpendingLimitUse` checks the signer against `limit.members`, the limit's own roster) vs the multisig roster used everywhere else. `RemoveMember` (`:1683-1694`) updates only the multisig; the action grammar has `AddSpendingLimit`/`RemoveSpendingLimit` for whole accounts but **no action to edit a limit's member or destination roster**.

**Mechanism:** spending-limit authority is captured at limit creation and never revoked by membership changes. A member removed from the multisig (e.g., for cause, or a departed employee key) can keep drawing the vault allowance until governance separately notices and deletes the whole limit — and re-creating the limit without them resets `last_reset`/`remaining_amount` from scratch. With a `PERIOD_DAY/WEEK/MONTH` limit the draw **resets forever**.

**Exploit (reproduced; M5):**

1. Threshold-2 multisig A/B/C. Config proposal adds a spending limit (create key `0x71…`, vault 0, SOL, amount 1000, `PERIOD_ONE_TIME`, `members = [B]`, destinations unrestricted).
2. B draws 400 via `SpendingLimitUse` (data `[amount:u64le, 09]`; accounts `multisig/ro, limit/mut, member/ro+signer, vault/mut, destination/mut,
   clock, pid, pid, pid, system`) → pays out.
3. Config proposal removes B from the multisig roster; executes cleanly.
4. B's vote on a fresh proposal → **blocked**, `NotAMember` (the roster is enforced on the vote path).
5. B's `SpendingLimitUse` again → **succeeds**; another 400 leaves the vault. Harness log: `[M5] EXPLOITED: B (removed from the multisig) drew another
   400; the allowance roster was never revised`.

**Severity:** Medium. Permission revocation gaps are the classic insider failure (incident class "admin/role removal incomplete"). The amount is bounded by the allowance per period, but the bound renews, the destination list is attacker-friendly if unrestricted, and removal-for-cause is precisely when the key must stop working.

**Fix:** either validate the drawer against the **multisig roster's** VOTE (or a dedicated permission) in addition to the limit roster, or give `RemoveMember` a companion requirement: the action stream that removes a member must also remove them from every spending-limit roster that names them (the executor already has the limit accounts available). At minimum, document the coupling in the example and add a regression test.

---

## E4 — role_registry: duplicate role entries via non-canonical bump — CONFIRMED (admin-gated)

**Location:** `examples/role_registry_program/src/lib.rs:232` (`AddRole` uses `CreateProgramAccountWithUncheckedBump`; same at `:186` for `Initialize`). The framework's checked builders reject non-canonical bumps (`crates/pina/src/cpi.rs:46-63` `canonical_pda`, used at `:433` and `:869` with `Some(bump)`); the unchecked variant only requires the address to match `seeds + supplied bump`, so any off-curve bump creates a second live account for the same seeds.

**Mechanism:** role uniqueness is enforced only by PDA occupation at the canonical bump. A second `RoleEntry` for the same `(registry, role_id)` can be created at a non-canonical bump with **a different grantee and different permissions**, and is fully manageable (`UpdateRole`/`DeactivateRole` accept it — they never re-derive the canonical address). Indexers and clients that resolve `find_program_address(b"role-entry", registry, role_id)` see only the canonical record.

**Exploit (reproduced; R1):**

1. Initialize the registry (admin-funded).
2. `AddRole role_id=1, permissions=u64::MAX` at the canonical bump.
3. Find `bump' > canonical` whose `create_program_address` succeeds (`bump' = 2` here); `AddRole role_id=1, grantee=G2, permissions=0xFFFF,
   bump=bump'` → **succeeds**. Two accounts now both claim role 1.
4. `UpdateRole` on the shadow entry succeeds — two divergent records for one role in one registry. Harness log: `[R1] EXPLOITED: role 1 exists twice …`.

**Severity:** Low. Every step requires the **admin's signature**, so this is not a stranger exploit; it is a uniqueness-invariant break (buggy retrying clients, key-padding confusion) plus indexer shadowing, and `role_count` inflates past the number of distinct roles. Note the multisig program is _not_ affected: all its compact creates use the checked builder.

**Fix:** use `CreateProgramAccountWithBump` (canonical-enforcing) for both `Initialize` and `AddRole`; the code comments currently argue the noncanonical case is benign, but "duplicate records for the same logical role" is exactly the confusion unchecked bumps invite. If the CU argument for unchecked bumps is kept, re-derive and compare the canonical address in the handler and reject mismatches.

---

## E5 — role_registry: zero-address rotation bricks the registry permanently — CONFIRMED (adjacent to known L10)

**Location:** `examples/role_registry_program/src/lib.rs:317-329` (`RotateAdmin` writes `registry_config.admin = *self.new_admin.address()` with no signature or plausibility requirement on `new_admin`).

**Exploit (reproduced; R1):** `RotateAdmin` with `new_admin =
Pubkey::default()` succeeds. Every later admin instruction fails: the stored admin can never sign, so `AddRole`/`UpdateRole`/`DeactivateRole`/ `RotateAdmin` all die at `assert_address` (runtime `invalid account data for instruction`, program log "account address is invalid"). There is no recovery path; role entries are frozen forever.

**Severity:** Low (the program holds no funds), and the root cause — one-step rotation with no new-admin signature — is already recorded as known L10 in `security/deep-audit-2026-09-18.md`. This run adds the concrete zero-address-brick evidence and confirms the blast radius (total, irreversible liveness loss from a single signature). Counted as covered, not new.

**Fix:** require `new_admin` to be a signer on `RotateAdmin` (two-step or accept-sig rotation), which also eliminates the zero-address brick. Reuse the existing lesson 11 material.

---

## E6 — role_registry: permission validation is dead code; deactivate is permanent — CONFIRMED (two small items)

**Location:** `examples/role_registry_program/src/lib.rs:205-255` (`AddRole` stores `args.permissions` unvalidated; same in `UpdateRole` `:257-286`), and `Init:112-128` / lifecycle (`DeactivateRole` `:288-315`; no reactivate instruction exists).

**Findings (both reproduced in R1):**

- `AddRole` accepts `permissions = u64::MAX` and `permissions = 0` verbatim. `RegistryError::InvalidPermissions` ("The requested permission bits are empty or outside the supported set") is **never produced by validation** — its only use is the unrelated registry-mismatch check in `UpdateRole`/`DeactivateRole`, so the error's documented meaning is a lie on the wire.
- `DeactivateRole` is a one-way tombstone: updates then fail with `RoleInactive`, and re-`AddRole` of the same id fails with `AccountAlreadyInitialized` — the `role_id` is burned permanently and `role_count` never decreases. Liveness foot-gun, Low.

**Severity:** Informational-to-Low. Nothing is Escalated (no consumer is shipped), but the example teaches error-code semantics that do not hold.

**Fix:** validate `permissions` against the supported mask at `AddRole`/ `UpdateRole` (returning `InvalidPermissions` for real), give it a distinct error for the registry-mismatch case, and either add `ReactivateRole` or document deactivation as permanent (and stop counting tombstones in `role_count`).

---

## E7 — Vault-bump canonicality is convention, not invariant (multisig) — SUSPECTED (documented trade-off, not executed)

**Location:** `examples/multisig_program/src/lib.rs:2152-2170` — the client supplies `vault_bump`/`ephemeral_bumps`; creation verifies only that the seeds derive a _valid_ PDA, and execution signs with exactly those seeds.

**Mechanism:** a proposal may be created whose vault is a non-canonical-bump PDA. This is intentional and documented (`multisig-example-hard-edges.md` item 13a: any derived address is program-controlled), and funds only sit at the address someone deliberately funds. Recorded here so the sweep shows the edge was considered: no exploit identified beyond self-consistent "shadow vault" usage; execution binds to the stored bump, so no substitution is possible post-creation. No action beyond the docs ask in hard-edges 13a.

---

## Blocked attacks (each executed against the artifact unless marked static)

| #   | Attack                                                                                                      | Outcome                         | Stopping check                                                                                                                                                                                                        |
| --- | ----------------------------------------------------------------------------------------------------------- | ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1  | Execute a DRAFT proposal (0 approvals)                                                                      | Blocked                         | `InvalidProposalStatus` (status ≠ APPROVED), `VaultExecute` `:2554`                                                                                                                                                   |
| B2  | Execute at 1/2 approvals                                                                                    | Blocked                         | same status gate; threshold only sets status at approve time `:2375-2379`                                                                                                                                             |
| B3  | Same wallet approves twice                                                                                  | Blocked                         | `AlreadyVoted` (`approved_mask & bit != 0`) `:2367-2369`                                                                                                                                                              |
| B4  | Non-member votes                                                                                            | Blocked                         | `NotAMember` (roster binary search) `:2275`                                                                                                                                                                           |
| B5  | Replay a vault proposal after execution                                                                     | Blocked                         | status `EXECUTED` ≠ APPROVED `:2554`; payout balance unchanged                                                                                                                                                        |
| B6  | Cancel someone else's draft                                                                                 | Blocked                         | `Unauthorized` (creator check) `:2513-2515`                                                                                                                                                                           |
| B7  | Timelock bypass at eta−1 s                                                                                  | Blocked                         | `TimeLockNotReleased` (`elapsed < timelock`) `:2561`                                                                                                                                                                  |
| B8  | Execute exactly at eta                                                                                      | Allowed (by design)             | guard is `<`, so `elapsed == timelock` releases — boundary is tight, not off-by-one                                                                                                                                   |
| B9  | Vault execute after expiry                                                                                  | Blocked                         | `ProposalExpired` `:2566`                                                                                                                                                                                             |
| B10 | Activate a proposal after its ttl                                                                           | Blocked                         | `ProposalExpired` `:2329-2331`                                                                                                                                                                                        |
| B11 | Revoke a frozen (stale) approval                                                                            | Blocked                         | `StaleProposal` `:2464-2466` — _this is what makes E2 dangerous_                                                                                                                                                      |
| B12 | Non-canonical bump shadow multisig / proposal / spending limit                                              | Blocked (static + framework)    | `CreateCompactProgramAccountWithBump` → `canonical_pda(..., Some(bump))` rejects non-canonical bumps (`crates/pina/src/cpi.rs:46-63`, `:869`)                                                                         |
| B13 | Reentrancy: vault message CPIs back into VaultExecute/ConfigExecute for the same proposal                   | Blocked (static)                | `ProtectedAccount` rejects multisig/proposal as writable message accounts `:2656-2658`; CPI cannot escalate readonly→writable, and both instructions need `&mut` proposal (and ConfigExecute needs writable multisig) |
| B14 | Threshold lowered mid-flight to auto-execute an ACTIVE proposal                                             | Blocked (static reasoning + B2) | config change marks the proposal stale; approve on stale is `StaleProposal` `:2357`, and execute requires `STATUS_APPROVED`, which only legitimate approvals set                                                      |
| B15 | 16-member roster liveness (packet-size DoS)                                                                 | Cleared                         | create with 16 trailing members fits one 1232-byte packet; activate + 8 approvals + execute all succeed (M6). Earlier apparent crash was the test harness's 16 rapid airdrops, not the program                        |
| B16 | Cross-registry role update (role entry from registry 1 + registry 2 config)                                 | Blocked                         | `InvalidPermissions` (registry-binding check) `role_registry src/lib.rs:278-280`                                                                                                                                      |
| B17 | Non-admin role update                                                                                       | Blocked                         | `admin.assert_address(&registry_config.admin)` → runtime `InvalidAccountData`                                                                                                                                         |
| B18 | Duplicate role at the canonical bump                                                                        | Blocked                         | system create fails: `AccountAlreadyInitialized`                                                                                                                                                                      |
| B19 | Registry init front-run                                                                                     | Blocked (static)                | `Initialize` requires the admin signature; the PDA is seeded by the admin, so a front-runner can only create their own registry                                                                                       |
| B20 | Vault-proposal signer forgery (message-declared signer that is neither tx signer, vault, nor ephemeral PDA) | Blocked (static)                | `:2660-2666` signer/marker check                                                                                                                                                                                      |

---

## Lint proposals

1. **`require_symmetric_time_guards`** — when one instruction advances a state machine to an effect-bearing state, flag any sibling that reaches the same state without re-running the same freshness guards. Concretely: ConfigExecute reaches `STATUS_EXECUTED` without the `is_expired` guard VaultExecute runs (E1). A cheap heuristic: for each `ProgramError` guard name (`ProposalExpired`, `StaleProposal`, `TimeLockNotReleased`), list the functions that read the corresponding field and flag sets that differ.
2. **`require_stale_index_on_effects`** — any instruction that consumes a proposal to produce cross-account effects must check `stale_transaction_index` (or prove in a comment why not). Would have flagged VaultExecute's deliberate skip and forced the E2 decision to be written down next to a mitigation for mask re-binding.
3. **`require_canonical_bump_for_keyed_accounts`** (warn) — flag `CreateProgramAccountWithUncheckedBump` when the seeds are document-unique (the `(registry, role_id)` case, E4); default-deny with an explicit opt-out comment.
4. **`require_new_authority_signature`** — one-step rotation to an unsigned authority (the L10 family, E5); `RotateAdmin`'s `new_admin` should carry a signer requirement.
5. **`require_used_validation_errors`** — flag custom error variants whose documented meaning ("empty or outside the supported set") is never produced by the path that accepts the input (E6). Dead validation is worse than no validation because it reads as enforced.
6. **Docs/checklist item rather than lint — "revocation completeness"**: when a membership action exists (RemoveMember), enumerate derived authority stores (spending-limit rosters) and require the action stream to handle them (E3). Fits the existing guardrails doc next to lesson 11.
7. Keep hard-edges 13a's bump-as-argument convention, but pair it with the warn lint from (3) so new examples opt into non-canonical vaults consciously (E7).

---

## Coverage notes

- Scenarios in the attack matrix not reachable tonight are recorded in the blocked table with their static gates (B12, B13, B14, B19, B20); nothing matrix-listed was left unexamined.
- The spending-limit SPL/token path, `MultisigImport` from a live foreign program, and proposal-close rent accounting were not dynamically exercised (no token mint in the harness; import needs a foreign-owned fixture the state cheatcode refuses, per hard-edges item 9 — the owner check itself is dynamically proven in `examples/multisig_program/tests/surfpool`).
- Raw byte offsets used by the harness are asserted against live accounts in M1 (`threshold`, roster) and M3 (`timelock`, status), so the recorded outcomes do not depend on the generated view API.
