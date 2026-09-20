# Migration attack matrix — dynamic scenarios (2026-09-19)

Companion to `migration-static.md`. Each scenario is executable by an agent that has never read the code. All scenarios target the shipped example program unless marked PROBE (scratch crate).

## Shared setup

- **Program source**: `examples/migrations_program` (workspace root `/Users/ifiokjr/Developer/projects/pina-rs/pina`).
- **Program ID**: `GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS`.
- **Build**: run inside `devenv shell`. `pina test` sets `PINA_SBF_ARTIFACT` for the Surfpool harness; a bare artifact works too: `cargo build-sbf -p migrations_program --features bpf-entrypoint` (SBF tier wrapper: `docs/agents/testing-and-sbf.md`).
- **Harness**: `pina_test::ProgramTest::start()` (offline Surfnet), `install_historical_account`, `send_instruction_with_signers`, `expect_historical_rejection_with_rollback`, `snapshot_accounts`/`account`. See `crates/pina_test/src/lib.rs:331-373` (`HistoricalAccount::new(version, address, owner, data)` — rent-exempt by default, `.with_lamports(n)` to override) and `:1259` (`expect_historical_rejection_with_rollback`).
- **Wire formats** (all little-endian; byte 0 = discriminator, byte 1 = migration version where enveloped):
  - `Update` v0: `[00, 00, value:8]` (10 B) · v1: `[00, 01, value:8]` (10 B) · current v2: `[00, 02, value:8, memo:2]` (12 B).
  - `Relay` (unenveloped): `[01, value:8]` (9 B).
  - `State` account: v0 42 B `[01, 00, authority:32, value:8]` · v1 43 B (adds `enabled:1` at offset 42) · v2 44 B (adds `revision:1` at offset 43). Current-size invariant: `State::SIZE == 44`.
  - `ManualState`: v0 3 B `[02, 00, amount:1]` · v1 4 B `[02, 01, amount:2]` · v2 compact `[02, 02, len:1, digits...]` (3+len).
  - `CompactState`: v0 `[03, 00, name_len:1, name...]` · v1 `[03, 01, name_len:1, tags_len:2, name...]`.
  - Reserved migrate instruction: data exactly `[FF]` (1 byte). Metas: slot 0 payer (writable, must sign when funding is needed; substitute the **program's own address** to omit), slot 1 `11111111111111111111111111111111` (system program), then the derived ladder: slot 2 `State`, slot 3 `ManualState`, slot 4 `CompactState`, slot 5 a second `State`. Any later slot holding the program address or missing is "omitted".
- **Determinism**: use fixed `Keypair::new_from_array` seeds for payer/authority/targets (benchmark & sweep convention).

Expected outcomes below cite the runtime gate that decides them, so the executor can attribute a failure to the right check.

---

## A. Forced migration by a non-authority third party (M1 — permissionless reserved route)

**Question**: can an attacker who is not the account authority migrate someone else's account, and does that change business state?

1. Deploy the program. Create/fund attacker payer `A` and victim authority key `V`.
2. Install the victim account at `S` (owner = program ID) as a historical fixture: `HistoricalAccount::new(0, S, program_id, v0_state_bytes)` where `v0_state_bytes = [01, 00, <V:32>, <100u64>]` (42 B; fixture auto-rent-exempts 42 B).
3. Snapshot `S` (bytes + lamports).
4. Send instruction: data `[FF]`; metas `[A (writable, signer), system_program, S (writable), program_addr (placeholder), program_addr, program_addr]`.
5. **Exploit outcome (expected, by design)**: tx succeeds. `S` is now 44 B: `[01, 02, <V>, <100>, 00, 00]` — `enabled = 0`, `revision = 0`. `A`'s balance dropped by the rent-exemption delta for 2 bytes (~6,960 lamports); `S` gained exactly that.
6. **Assertions**: authority bytes unchanged; `value` unchanged; no lamports left `S`; `A` is the only lamport loser; sending attacker's own `Update` against `S` afterwards still fails the authority check (`InvalidAccountData`) — migration did not grant anything.
7. **Verdict to record**: migration is permissionless but semantics-preserving (transitions are pure). If any assertion other than "authority unchanged" fails, escalate — that would break the purity invariant.

Variants:

- **A2 — omitted payer**: same but slot 0 = program address → `MigrationRequired` (no payer for the rent deficit), account untouched.
- **A3 — victim pays**: payer = `V` with signing → same end state; documents the cost the route charges its own users.

## B. Reserved-route authorization and layout guards (blocked attacks)

Each ends with the named error and **zero byte/lamport changes**; assert with `expect_historical_rejection_with_rollback` or snapshot compare.

| ID  | Setup variation vs scenario A                                                   | Expected error                                                                      | Gate                                                  |
| --- | ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------- |
| B1  | Program invoked via CPI from another program with wrong executing id            | `IncorrectProgramId`                                                                | `entrypoint.rs:478` pre-migrate program-id check      |
| B2  | Same account `S` in slots 2 **and** 5                                           | `DuplicateMutableAccount`                                                           | duplicate-address scan `migration.rs:1136-1142`       |
| B3  | Slot 0 payer = slot 2 target `S` (both writable, `S`'s key signed)              | `DuplicateMutableAccount`                                                           | `validate_funding_payer` `migration.rs:925-927`       |
| B4  | `S` owner = attacker key instead of program                                     | `InvalidAccountOwner`                                                               | `assert_owner`                                        |
| B5  | `S` marked read-only                                                            | writable assertion                                                                  | pre-planning `assert_writable`                        |
| B6  | Slot 2 bytes = future version `[01, 03, ...]` (44 B, plausible fields)          | `InvalidMigrationVersion`                                                           | `StoredVersion::Future` before any decode             |
| B7  | Slot 2 bytes = corrupted current `[01, 02, <V>, <100>, 02, 00]` (`enabled = 2`) | `InvalidAccountData`, **catchable, no effect**                                      | `AlreadyCurrent` path validates before returning `Ok` |
| B8  | Data `[FF, FF]` (2 bytes) instead of `[FF]`                                     | instruction rejected (falls through to `parse_instruction` → unknown discriminator) | width-exact trigger `migration.rs:76`                 |
| B9  | Payer writable but not a signer, growth needed                                  | transfer fails / signer assert                                                      | `payer.assert_signer` when `funding > 0`              |
| B10 | Two `[FF]` instructions in one transaction                                      | first migrates, second returns `AlreadyCurrent` (success, no-op)                    | idempotent re-entry                                   |

## C. Interleaving: stale accounts vs business instructions (blocked)

1. **C1 — v0 State into current `Update` under attacker authority.** Install v0 `S` (authority `V`, value 100). Send `Update` data `[00, 00, <999u64>]` (v0 payload; normalization is free) with metas: authority = attacker (signer), `migration_payer` = attacker (writable, signer), `system_program`, `state` = `S` (writable). Expected: migration executes (42→44 B), then `state.authority != attacker` → `InvalidAccountData` → **whole tx rolls back**: `S` must be byte-identical 42 B v0 with original lamports. Gate: handler order + tx atomicity. (Surfpool twin of existing Mollusk test `authorization_failure_after_migration_rolls_back_bytes_length_and_lamports` — keep as a real-runtime regression.)
2. **C2 — v0 payload cannot forge new fields.** Same but authority = `V`: succeeds; assert stored `enabled = 1` (handler sets it), `revision = 1`, and `memo` semantics: the v0 request normalized with `memo = 0`. No input path sets `memo` from a v0 payload.
3. **C3 — stale State passed to a handler that only reads.** `Update` with only authority+referrer (all migratable slots omitted) must succeed as a no-op without touching `S`. Verifies the early-return shape cannot be turned into a partial-supply bypass: any partial supply (e.g. state without payer) → `NotEnoughAccountKeys`.

## D. Width changes, residue, and funding (M4/M5)

1. **D1 — shrink with residue truncation.** Install `CompactState` v0 with a spare tail: `[03, 00, 03, 'A','d','a', EE]` (7 B). Reserved route slot 4. Expected: final `[03, 01, 03, 00, 00, 'A','d','a']` (8 B), **no `EE` byte anywhere** in the account data — the executor's shrink-to-target truncated it.
2. **D2 — ManualState ladder economics.** Install `ManualState` v0 `[02, 00, FF]` rent-exempt for 3 B via `.with_lamports(Rent.minimum_balance(3))` (use the fixture default). Payer `A` holds exactly 1 lamport. Expected: funding for the v1 growth (4 B) fails → `InstructionError`, and `S` is byte-identical v0 (pre-effect boundary — the transfer failure is the first effect and is catchable/rolled back).
3. **D3 — full ladder with sufficient funding.** Same as D2 with a funded payer: final 6 B `[02, 02, 03, '2','5','5']`; assert `A` lost exactly the 3-byte→6-byte exemption delta and nothing more (no overfunding — M5).
4. **D4 — PROBE (requires scratch crate `tmp/sweep/scratch-migration/residue-probe/`): uninitialized grown region.** Copy the example, add a `State` v2→v3 history whose manual transition appends `bonus: u64` at offset 44 but **does not write it**, with no validation on `bonus`. Pre-seed a v2 account whose pre-shrink history contained `0xFF`-filled tail bytes (install 46 B v0 fixture so realloc residue is attacker-known), migrate on the reserved route, then read raw account data. **Exploit outcome if M4 is real**: `bonus == f(residue)` instead of a defined default. **Safe outcome (executor already fixed)**: `bonus == 0`. Record which — this decides whether the M4 fix must land before any downstream program ships a manual additive transition. Do all work under `worktrees/security-sweep-2026-09-19/tmp/sweep/scratch-migration/`; do not touch tracked files.

## E. History-depth stranding (M2 — PROBE, scratch crate)

1. Copy the example into `tmp/sweep/scratch-migration/depth-probe/`; extend one account's history to 10 versions via repeated `pina migrations make` cycles.
2. Install a v0 fixture; send `[FF]`.
3. **Exploit outcome**: `MigrationUnavailable` (10 steps > `MAX_INLINE_STEPS = 8`); the account can never migrate — assert it is stuck across repeated calls.
4. Also assert the client-side workaround: drive `[FF]` twice — each call advances ≤ 8 adjacent steps, so 8-stale accounts recover but 9+-stale never do. Record the exact stranded depth observed.

## F. Close/recreate and replay (blocked)

1. **F1 — migrate-after-close.** Install v0 fixture `S`, then close it via the runtime (transfer all lamports to system owner). Send `[FF]` with `S` writable. Expected: `InvalidAccountOwner` (owner is now the system program); no resurrection.
2. **F2 — recreate-with-attacker-bytes.** After F1, `A` funds a new system account at `S` with 44 bytes of attacker data impersonating `[01, 02, <V>, ...]`. Expected: still system-owned → `InvalidAccountOwner` on the migrate route; even if a program bug re-inits it, `initialize` rewrites the discriminator/version and validates — attacker bytes cannot survive as a valid v2 `State` unless the program copies them (out of framework scope).
3. **F3 — replay.** Re-send the exact successful tx from scenario A. Expected: `AlreadyCurrent` success, no byte or lamport change (migrations are idempotent; the hash-chained ledger is host-side only and plays no on-chain role — see migration-static.md M7).

## Execution notes for the agent

- Use `expect_historical_rejection_with_rollback` (it snapshots and asserts zero state change) for every B/C "blocked" row; use manual snapshot compare for success rows.
- Errors surface as `TransactionError::InstructionError` wrapping the `PinaProgramError` codes (`MigrationRequired`, `InvalidMigrationVersion`, `MigrationUnavailable`, `MigrationLamportBudgetExceeded`, `DuplicateMutableAccount`).
- Existing Mollusk coverage (`examples/migrations_program/tests/e2e.rs`) already proves: rollback on authorization failure, trailing-byte smuggling, foreign owner, read-only rejection, alias rejection, future-version rejection, mid-chain funding rollback, budget preflight, mixed-version sets, self-CPI relay. The matrix adds the **reserved-route permissionless behavior (A)**, **reserved-route guard battery (B)**, **real-runtime shrink/residue checks (D)**, and **depth stranding (E)** — prioritize A, B2/B3/B6, D1, D4.
- Anything run from a scratch crate stays under `worktrees/security-sweep-2026-09-19/tmp/sweep/scratch-migration/`; tracked files are read-only for this sweep.
