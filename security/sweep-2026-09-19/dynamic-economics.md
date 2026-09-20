# Dynamic economics re-verification — security sweep 2026-09-19

Dynamic (on-artifact) re-verification of the rewritten `examples/vesting_program` and `examples/staking_rewards_program` economics, plus red-team probes against the new implementations. Companion to `deep-audit-2026-09-18.md` (H1/H2) and `deep-audit-2026-09-19-cross-check.md` (the immunity claims tested here).

Method: standalone attack crate at `tmp/sweep/econ-attacks/` (not part of the workspace), three `#[ignore]` tests run against the deployed SBF artifacts with `pina_test::ProgramTest::start_with_artifact`, real SPL mint / ATA / MintTo flows, exact balance assertions around every instruction, deterministic `Keypair::new_from_array` seeds, and `time_travel_to_timestamp_millis` for the vesting schedule. Final combined run: **3 passed, 0 failed (188 s, `--test-threads=1`)**.

Artifacts under test (worktree `target/surfpool/examples/`):

| Artifact                                                                      | sha256                                                             |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `vesting_program.so` (`FEa5fqN6NACrhWUZSBdGKybJKNxkdw8cdLvRvTARsFHh`)         | `50f42a851d0e85b7b8a9d5baeda0814168646db2dcd0f8ba5d9fb591b3eca2a2` |
| `staking_rewards_program.so` (`9MBwKBjzTLtLe8PkHVhi5CfGxKo8gCYbMEg5NMt1tcvr`) | `5fea5150dd5aeca101537e5729176f26f715fc51f9af8ca3b066d90b7bd5345b` |

Reproduce:

```bash
cd <worktree root>
devenv shell -- cargo test --manifest-path tmp/sweep/econ-attacks/Cargo.toml -- --ignored --nocapture --test-threads=1
```

---

## Verdict

The H1/H2 fixes hold under dynamic attack. **Every vesting claim and cancel behaves exactly as the schedule ledger dictates, with real token movement and no inflation or theft vector found.** The staking rewrite's index/checkpoint/release machinery is sound (checkpoint, monotonicity, overflow fail-closed, canonical-position integrity all verified), but the template retains one economically live hole: **deposit credits stake without custody, and the claim path pays real rewards for it — executed end-to-end as a full vault drain (F-1).** That hole is the direct consequence of the documented "transfers into/out of the stake vault are out of scope" scope cut, and it is now the largest residual template risk.

---

## Verification table

### Vesting (`vesting_program.so`)

Schedule used unless noted: `start=1_800_000_000`, `cliff=+100 s`, `end=+300 s`, `total=1_000_000_000` (6 dp), vault funded by `MintTo`.

| #   | Claim                                            | Verdict              | Balance evidence (all on the deployed artifact)                                                                                                                                                                                                                                                                |
| --- | ------------------------------------------------ | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| V1  | Claim before cliff fails                         | **PASS**             | Travel to `+50 s`: rejected `CliffNotReached` (custom 3); `claimed_amount = 0`; beneficiary ATA not created                                                                                                                                                                                                    |
| V2  | Claim at cliff pays correct pro-rata real tokens | **PASS**             | At `cliff` exactly: claim `333_333_333` (= floor(1e9 × 100/300)); beneficiary ATA `0 → 333_333_333`; vault `1_000_000_000 → 666_666_667`                                                                                                                                                                       |
| V3  | Double-claim cannot exceed vested                | **PASS**             | Second claim of `400_000_000` (under total, over the curve) rejected `ClaimTooLarge` (custom 1); `claimed_amount` stays `333_333_333`; ATA unchanged — the curve bound, not just the total bound, is enforced                                                                                                  |
| V4  | Claim after end pays exactly total               | **PASS**             | Travel past `end`: claim `666_666_667` → beneficiary ATA `1_000_000_000`, vault `0`; a further claim of `1` rejected `ClaimTooLarge`                                                                                                                                                                           |
| V5  | Cancel refunds unclaimed balance; vault closes   | **PASS**             | On the donation schedule (remaining `250_000_000` in vault): admin ATA `0 → 250_000_000`; vault account deleted (fetch errors); post-cancel claim fails                                                                                                                                                        |
| V6  | Non-beneficiary claim fails                      | **PASS**             | Attacker-signed claim rejected by the stored-beneficiary address assertion; attacker ATA still absent                                                                                                                                                                                                          |
| V7  | Donation cannot inflate entitlement              | **PASS**             | Vault overfunded to `1_250_000_000`: claim of `1_000_000_001` rejected `ClaimTooLarge`; claim of `1_000_000_000` pays exactly `1_000_000_000`; the `250_000_000` surplus stays in the vault until Cancel returns it to the admin (documented clawback)                                                         |
| V8  | Underfunded vault fails cleanly                  | **PASS**             | Vault at `250_000_000` of a `1_000_000_000` schedule: claim rejected `InsufficientVaultBalance` (custom 4); `claimed_amount = 0`; no payout ATA created — the ledger update reverts with the failed transfer                                                                                                   |
| V9  | ATA spoofing on claim payout                     | **PASS (blocked)**   | Claim naming the attacker's token account as `beneficiary_ata` fails at the ATA CPI: `CreateIdempotent` → "Associated address does not match seed derivation" (`Provided seeds do not result in a valid address`); balances unchanged                                                                          |
| V10 | Shadow schedule at a noncanonical bump           | **PASS with caveat** | Creation is **accepted** (vesting uses `CreateProgramAccountWithUncheckedBump`, `src/lib.rs:238`); the shadow state is fully functional. Contained: it requires the admin's signature, and its entitlement is bounded by its own separately funded vault — a duplicate, not an amplification. See Observations |

### Staking (`staking_rewards_program.so`)

Note: time does not drive accrual in this design — the admin-controlled `SetRewardIndex` is the clock. "Drip" below = `SetRewardIndex`; scale = `1e12` (one index unit = one reward token per staked token).

| #  | Claim                                               | Verdict                    | Balance evidence                                                                                                                                                                                                                                                                                                |
| -- | --------------------------------------------------- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S1 | Stake → drip → claim pays real tokens per the index | **PASS**                   | Deposit `1_000`, drip index `0 → 1_000_000_000_000`: claim transfers exactly `1_000` reward tokens; reward vault `1_000_000_000 → 999_999_000`                                                                                                                                                                  |
| S2 | Repeated claim cannot re-accrue (checkpoint)        | **PASS**                   | Second claim without a drip rejected `NothingToClaim` (custom 6); user ATA stays `1_000`; vault stays `999_999_000`                                                                                                                                                                                             |
| S3 | Withdraw returns principal                          | **PASS (ledger only)**     | Withdraw `600`: position staked `1_000 → 400`, `total_staked → 400`. Caveat: no token ever moved — user stake ATA stays `1_000`, stake vault stays `0` throughout the whole test. See F-1                                                                                                                       |
| S4 | Two positions for one wallet cannot double-accrue   | **PASS (blocked)**         | Second `OpenPosition` for the same `(pool, owner)` at a noncanonical bump rejected by `CreateProgramAccountWithBump`'s canonical search; the shadow position account never exists. (The seeds also bind the owner, so an attacker cannot open a victim-keyed position at all.)                                  |
| S5 | Admin drip updates index; non-admin drip fails      | **PASS with caveat**       | Admin drip sets `reward_index` bytes to `1_000_000_000_000`; attacker drip rejected — but with pina `InvalidAccountData`, **not** the advertised `StakingError::Unauthorized` (see F-2). Index regression rejected `RewardIndexRegressed` (custom 5); index unchanged after both                                |
| S6 | Index overflow / huge drip doesn't corrupt accrual  | **PASS**                   | Position staked `u64::MAX/2`, drip to `u64::MAX`: claim fails closed (u128 intermediate overflow → error); vault stays `1_000_000_000`; `pending_rewards` stays `0`. Residual: the index can never be lowered (monotone), so this is an admin self-DoS only                                                     |
| S7 | Dust: claim → restake → claim mints no value        | **PASS**                   | Drip of index `1` (accrual floors to 0) → claim rejected `NothingToClaim`; drip to a full unit → claim pays exactly `1_000`; restake `+500`, drip one more unit → claim pays exactly `1_500`; total `2_500` = stake-weighted entitlement with zero created value (floor dust is lost to the pool, never minted) |
| S8 | Unbacked stake draws real rewards                   | **FAIL — exploited (F-1)** | Attacker deposits `1_000_000_000_000` while owning **zero** stake tokens (deposit is bookkeeping-only); admin drips index to `1_000_000_000`; attacker claims `1e12 × 1e9 / 1e12 = 1_000_000_000` — the **entire reward vault** moves to the attacker's ATA; vault `0`                                          |

---

## Findings

### F-1 — Unbacked deposits draw real rewards: permissionless drain of the reward vault (executed)

- **Location:** `examples/staking_rewards_program/src/lib.rs:378-463` (`Deposit` — credits `staked_amount`/`total_staked` from the instruction argument alone; never reads `user_stake_ata` and never touches `stake_vault`), paying out at `:637-645` (`Claim`'s `TransferChecked` from the reward vault). The stake vault created at `:301-309` is read by no instruction in the program.
- **Mechanism:** `Deposit` mints staking credit out of thin air. `Claim` converts `staked_amount × index_delta ÷ REWARD_INDEX_SCALE` into a real token payout signed by the pool PDA. Nothing ties `staked_amount` to tokens in custody, so anyone can acquire arbitrary share weight for free and out-drain every honest position pro rata, up to the vault's balance (a payout larger than the vault fails closed via the SPL transfer).
- **Exploit (executed end-to-end on the artifact):**
  1. Attacker funds a wallet and calls `OpenPosition` for `(pool, attacker)` — canonical position, all checks pass.
  2. `Deposit 1_000_000_000_000` with an empty (auto-created) stake ATA and **zero stake tokens owned** — accepted; `staked_amount = 1_000_000_000_000`, `total_staked += 1e12`.
  3. Admin drips `SetRewardIndex(1_000_000_000)`.
  4. Attacker calls `Claim`: payout `= 1e12 × 1e9 ÷ 1e12 = 1_000_000_000`; the pool PDA signs the `TransferChecked`; attacker's reward ATA receives the full `1_000_000_000` reward vault; vault `→ 0`. On a shared pool this takes the share of every honest staker whose claim comes later (their payout then fails at the vault).
- **Severity:** Medium (template). The readme marks transfers into/out of the stake vault "deliberately out of scope" and the crate header says it is not a staking product, so this is a known scope cut rather than a regression — but with the new real payout path the combination is a working drain that ships to anyone who copies the template, and the cross-check document's immunity argument (donation/inflation class) did not examine the unbacked-deposit class.
- **Fix:** make deposit take custody — `TransferChecked` from `user_stake_ata` into `stake_vault` on `Deposit` (assert its derived address, as `Withdraw` already does for the user ATA) and back out on `Withdraw`; keep the existing bank-and-checkpoint logic, which is otherwise correct. Add a Surfpool assertion `stake_vault == pool.total_staked` to the suite so the backing invariant is pinned.

### F-2 — `SetRewardIndex` rejects a wrong admin with `InvalidAccountData`, not the advertised `Unauthorized`

- **Location:** `examples/staking_rewards_program/src/lib.rs:656-660` (`self.admin.assert_address(&pool_state.admin)` — pina's `validate_address` returns `ProgramError::InvalidAccountData`).
- **Mechanism:** `StakingError::Unauthorized` (3) is only produced by `assert_position_access`; the admin-mismatch check on the privileged instruction surfaces the generic pina loader error instead. The IDL/error enum therefore advertises an error code no path returns for this condition.
- **Exploit:** none — fail-closed. Dynamic evidence: attacker-signed drip rejected with `InstructionError(0, InvalidAccountData)`; `reward_index` unchanged.
- **Severity:** Low (error-identity / observability for indexers and clients).
- **Fix:** `.map_err(|_| StakingError::Unauthorized.into())` on the admin assertion, or a dedicated variant; add the error-code assertion to the Surfpool suite.

### Observations (no finding)

- **Vesting shadow schedule (unchecked bump):** `Initialize` uses `CreateProgramAccountWithUncheckedBump` (`src/lib.rs:238-244`), and a noncanonical-bump duplicate for the same `(admin, beneficiary, mint)` **was created on-chain** in the probe and paid out from its own vault. Containment held: the seeds bind the admin (signer), the beneficiary must sign claims, and each state's entitlement is bounded by its own vault, so a duplicate cannot amplify anyone's entitlement — matching the code's own comment. The staking side (pool and position creation) uses the canonical-search `CreateProgramAccountWithBump` and rejected the same class outright. If the vesting example ever grows a per-pair uniqueness invariant, it inherits the shadow-account hazard M1 documents.
- **Stale doc:** `staking_rewards_program/src/lib.rs:11-13` still says "Deposit, withdraw, and claim do not transfer tokens" while `Claim` now transfers (`:637-645`). The readme is accurate; the module doc is not.

---

## Blocked attacks

| Attack                                                                   | Result                                                                     | Why it failed                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------ | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ATA spoofing: redirect vesting claim payout to an attacker token account | Blocked — demonstrated                                                     | ATA `CreateIdempotent` derivation check ("Associated address does not match seed derivation"); instruction reverts, balances unchanged                                                                                                                                               |
| ATA spoofing on the staking claim payout                                 | Blocked by the same gate (same builder/CPI shape; not separately executed) | `CreateIdempotent` with `wallet = user` pins the destination                                                                                                                                                                                                                         |
| Shadow pool at a noncanonical bump                                       | Blocked                                                                    | `CreateProgramAccountWithBump` canonical search; also pinned tonight by the shipped suite test `rejects_a_shadow_pool_at_a_noncanonical_bump`, which passes on this artifact                                                                                                         |
| Shadow / duplicate position (double-accrual class)                       | Blocked — demonstrated                                                     | Same canonical creation check; plus the owner key in the seeds must sign                                                                                                                                                                                                             |
| Cancel-then-claim race                                                   | Blocked — demonstrated                                                     | `cancelled` flag → claim fails after cancel; the vault is closed, so every later account check fails too                                                                                                                                                                             |
| Position close + reopen replay                                           | N/A                                                                        | No close instruction exists for positions                                                                                                                                                                                                                                            |
| Donation inflation of entitlements (vesting or staking)                  | Blocked — demonstrated                                                     | Vesting entitlement is the schedule ledger (`claimed ≤ vested`), staking accrual is the index ledger; neither reads a vault balance to price a claim. Donated surplus flows to the admin on vesting `Cancel` (documented clawback) and sits in the staking vault until stakers claim |
| Reentrancy-style read-only residue (pre-CPI vault balance assumptions)   | Not exploitable                                                            | Sealevel CPI is non-reentrant; each instruction's ledger update and transfer commit or revert atomically (verified by the underfunded-claim case: ledger unchanged after failure)                                                                                                    |
| Token-2022 transfer-fee confusion                                        | Blocked statically; dynamic probe not run (time)                           | Every release-path mint read is `.assert_no_extensions()` (vesting `src/lib.rs:397`, staking `src/lib.rs:633`); vault/ATA bindings pin the token program per instruction                                                                                                             |
| Reward-index regression (re-claim released rewards)                      | Blocked — demonstrated                                                     | `SetRewardIndex` monotonicity → `RewardIndexRegressed`; per-position `reward_debt` checkpoint makes a no-drip re-claim `NothingToClaim`                                                                                                                                              |
| Overflow via huge index + huge stake                                     | Blocked — demonstrated                                                     | `u128` intermediate + `u64::try_from` fails the claim closed; vault and position untouched                                                                                                                                                                                           |

---

## Lint proposals

1. **`require_backed_deposit`** — flag handlers that credit an internal ledger amount without reading (balance or delta) a program-owned custody account for the same asset; stronger form: flag program-owned token accounts that are created in `Initialize` but never appear in any value-bearing instruction (exactly `stake_vault`'s shape in the staking template). Would have caught F-1 mechanically.
2. **Error-reachability check** — flag custom error variants that no handler can return for the condition their doc describes (here: `Unauthorized` advertised for the admin check on `SetRewardIndex`, which surfaces `InvalidAccountData`); cheapest form: suggest `map_err` whenever `assert_address` compares a signer against a stored authority.
3. **Doc-invariant sync** — warn when module-level docs state "does not transfer" style capability claims in a module whose instruction set contains `TransferChecked` CPIs (vesting/staking template drift already shows this rot in the staking header comment).
