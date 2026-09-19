# Dynamic red-team pass: `float_accounts_program` (deep) + long-tail examples

Sweep: 2026-09-19, red-team executor (dynamic). All attacks ran against the **real SBF artifacts** in `target/surfpool/examples/*.so` deployed into an offline Surfnet through `pina_test::ProgramTest::start_with_artifact`.

Attack crate: `tmp/sweep/tail-attacks/` (scratch only, untracked). Consolidated evidence log: `tmp/sweep/tail-attacks/full-run.log` — 5 test batteries, 136 probe lines, `test result: ok. 5 passed`.

Targets: `float_accounts_program` (deep), `validation_program`, `system_accounts_program`, `sysvar_checks_program`, smoke on `events_program`, `custom_errors_program`, `todo_program`, `transfer_sol_program`, `profile_program`, `declare_program`, `declare_id_program`, `hello_solana_program`.

Verdict up front: **no high-severity finding in these targets.** The headline result is that the float pipeline accepts every non-finite bit pattern verbatim (fineness is never validated), plus one fail-open gap: the instruction migration-envelope version byte is not enforced for zero-field instructions. Everything else — PDA binding, re-init, ownership, signature, count, discriminator, boundaries — stopped the attacks at the expected check.

---

## Findings

### F-1 — Float finiteness is never validated; NaN/Inf/subnormals land on-chain bit-exact — CONFIRMED

- **Location**: `examples/float_accounts_program/src/lib.rs:99-101,131-133` (`f32::from_bits(args.data_f32.get())` / `f64::from_bits(args.data_f64.get())` — the decode path applies no finiteness predicate) and `examples/float_accounts_program/src/lib.rs:76-77,91-92` (`account.data_f32.set(data_f32.to_bits())` — the write path round-trips raw bits). Same pattern on both `Create` and `Update`.
- **Mechanism**: The instruction schema stores floats as `PodU32`/`PodU64` (bit patterns). `try_from_bytes` validates widths and the migration envelope, but nothing requires `is_finite()`. Any f32/f64 bit pattern the attacker picks — quiet NaN, signaling NaN with arbitrary payload, ±Inf, subnormals, −0.0 — decodes, passes, and is stored verbatim in account data.
- **Exploit (executed)**:
  1. Deploy `float_accounts_program.so`; authority = funded keypair.
  2. Send `Create` (`[0, 0, f32_bits, f64_bits]`) for each pattern: qNaN `0x7ff8000000000000`, sNaN-with-payload `0x7ff0000000000001`, `+Inf`, `-Inf`, min-subnormal `0x0000000000000001`, `-0.0` `0x8000000000000000`, max-finite, negative subnormal. All 8 accepted.
  3. Read back account data; stored bits match the requested bits exactly in every case (probe log `[F1 create …] bit-exact: true`).
  4. Send `Update` as the legitimate authority overwriting finite values with qNaN — also accepted (`[F3 update→NaN] f64 now 0x7ff8000000000000`).
- **Severity justification**: Low _for this example_ — the program performs no arithmetic, so no value can be corrupted in-program. The danger is downstream and contractual: anything that later reads `f64::from_bits(data[2..10])` inherits NaN/Inf; indexers and clients serializing floats can crash or produce poisoned aggregates, and a cloned template that _does_ compute with floats inherits the unvalidated input with Medium-High impact. −0.0 adds an identity trap: it compares `== 0.0` under float equality while carrying distinct bits (`0x8000000000000000`), so equality-based dedup and bit-based change detection disagree.
- **Fix**: Validate at the schema boundary — extend `#[pina(validate(...))]` with a `finite` rule for float-carried fields (or run `f64::from_bits(v).is_finite()` in the existing account/instruction validation hooks) and reject with a dedicated error. Until then, document the raw-bits contract on the account struct. See lint L-P1 below.

### F-2 — Instruction migration-envelope version byte is unenforced for zero-field instructions — CONFIRMED

- **Location**: generated `#[instruction]` decode path (`crates/pina_macros/src/instruction.rs`, zero-payload branch); observed on `examples/events_program/src/lib.rs` (all three instructions have empty payloads, `InitializeInstruction` etc.).
- **Mechanism**: Every instruction carries `[discriminator, version, payload…]`. For instructions with fields the version byte is checked (`float_accounts` `Create` with version `1` → `custom program error: 0xfffffff7` (InvalidInstructionData) — probe `[F12d]`). For **zero-field** instructions the version byte is not validated: `events_program` accepts data `[0, 1]`, `[1, 7]`-style envelopes with any version byte (probe `[EV4 version-byte=1] ACCEPTED`).
- **Exploit (executed)**: `program.send(&[0u8, 1], vec![])` against the deployed `events_program.so` — transaction succeeds where the account-side envelope (`require_current_migration_envelope`) would reject.
- **Severity justification**: Low. With an empty payload there is no data to reinterpret today, but it is fail-open: a future migration that changes _anything_ about an instruction that currently has no fields (e.g. gains a field) cannot rely on the version byte to gate legacy senders, because legacy senders were never pinned to version 0 on-chain. Fail-open in the same family as deep-audit L6, but a distinct code path (instruction-side, zero-field only).
- **Fix**: In the generated `try_from_bytes`, check the version byte for zero-field instructions too (reject `version != current`), or assert the whole data length equals the envelope and reject non-current versions. Pin with a UI test that sends `[disc, 1]` for an empty instruction.

### F-3 — `validate_empty` is data-only; a funded, data-empty account reaches the create CPI — CONFIRMED (blocked by system program)

- **Location**: `crates/pina/src/impls.rs:114-127` (`validate_empty` calls only `is_data_empty()`); consumed at `examples/float_accounts_program/src/lib.rs:104` (`self.account.assert_empty()?`) and identically in `todo_program` (`self.todo.assert_empty()?`) and `profile_program` (`self.profile.assert_empty()?`).
- **Mechanism**: `assert_empty` never inspects lamports. An attacker-funded account with zero data passes the program's initialization guard, and only the system program's `create_account` CPI stops the duplicate create (`custom program error: 0x0`, AccountAlreadyInUse — probes `[F6]`, `[F7]`). Re-creating over an initialized account _is_ stopped by the program itself (`AccountAlreadyInitialized`, probes `[F5]`, `[TD5]`).
- **Exploit (executed)**: `program.fund(&target, 500_000_000)` then send `Create` with `target` in the account slot: guard passes, system CPI fails.
- **Severity justification**: Low. On this instruction set the system program is a correct backstop, so there is no exploit; the risk is structural — any future path that initializes state without a system create (realloc-in-place, direct-lamport flows like `transfer_sol_program`'s `send_owned`) would find `assert_empty` guarantees weaker than the name suggests.
- **Fix**: Either make `validate_empty` also require `lamports() == 0`, or add `assert_empty` documentation plus a lint (L-P3) when it guards a create-account CPI. Cheapest correct move: check lamports in `validate_empty` — no current example legitimately initializes a lamports-carrying empty account.

### F-4 — `system_accounts_program` accepts any system-owned wallet as `wallet` (parity, no state) — CONFIRMED, by design

- **Location**: `examples/system_accounts_program/src/lib.rs:36-41`.
- **Mechanism**: The only checks are `assert_signer` on authority and `assert_owner(system::ID)` on wallet. Any system-owned account (every plain wallet) passes both, including the authority's own wallet (`authority == wallet` accepted, probe `[SA1]`, `[SA4]`).
- **Exploit (executed)**: sign `Initialize` passing an arbitrary funded wallet as `wallet` — accepted.
- **Severity justification**: Informational. The instruction mutates nothing, so acceptance is harmless; it is the Anchor-parity shape. Recorded because it demonstrates the pattern is safe _only_ while the handler stays side-effect free — the moment a lamport or data write is added, the missing relationship checks (writable payer, distinctness) become load-bearing.
- **Fix**: None required for parity; add a comment pinning "no side effects" as the invariant that makes the thin validation safe.

### F-5 — Read-only metas on the fee payer are un-probeable: fee payer is implicitly writable — CONFIRMED (runtime semantics, not a program bug)

- **Location**: runtime behavior, exposed through `examples/float_accounts_program/src/lib.rs:57-62` (`authority: &'a AccountView` — no writability requirement on the payer).
- **Mechanism**: The tx fee payer is implicitly writable regardless of the account meta. Sending `Create` with the payer marked read-only still succeeded end-to-end: the system CPI debited rent and created the account (probe `[F8 read-only payer] ACCEPTED — payer lamports 8984327520 ->
  8983106480, account exists: true`). The same semantics defeated a read-only-audit attack in `validation_program` until the probe used a non-fee-payer account (`[V9c]` then blocked with InvalidAccountData from `validate_writable`, `crates/pina/src/impls.rs:50-63`).
- **Exploit (executed)**: `Create` with `AccountMeta::new_readonly(payer, true)` in the authority slot — accepted, lamports moved.
- **Severity justification**: Informational. No bypass — the debit the runtime permits is exactly the debit the program intended. Two residual notes: (a) the example never _asserts_ payer writability, so a non-fee-payer payer would die inside the system CPI with a raw InvalidArgument instead of a clean program error; (b) red teams should not score "readonly fee payer accepted" as a finding — it is Semantics, not a bug.
- **Fix**: Declare the payer `&'a mut AccountView` (or `assert_writable`) so misuse fails at the framework boundary; keep the runtime behavior out of the threat model.

### F-6 — Extra trailing accounts are silently ignored — CONFIRMED (events_program, zero-account entrypoint)

- **Location**: `examples/events_program/src/lib.rs:249-259` (entrypoint destructures no accounts).
- **Exploit (executed)**: send a valid `Initialize` instruction with one bogus writable account appended — accepted (probe `[EV7]`).
- **Severity justification**: Informational. Ignoring extra keys is standard for accounts-free programs; flagged because instruction _providers_ that forward account lists (CPI renderers, proxies) must not assume strict length checking exists here.
- **Fix**: None; optionally document.

---

## Blocked-attack log (stopping check per attack)

| Probe                | Program         | Attack                                                                                        | Stopping check (layer)                                             | Observed error                                                |
| -------------------- | --------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------- |
| F4                   | float_accounts  | Update by non-authority signer                                                                | `apply_update` authority compare (program)                         | custom 0 (`AuthorityMismatch`)                                |
| F5                   | float_accounts  | Re-`Create` over initialized account                                                          | `assert_empty` (program)                                           | `AccountAlreadyInitialized`                                   |
| F6/F7                | float_accounts  | Create over funded/zero-data account; slot swap                                               | system `create_account` (CPI)                                      | custom 0x0 (AlreadyInUse)                                     |
| F9                   | float_accounts  | Update with account meta read-only                                                            | `&mut AccountView` writability (framework)                         | `InvalidAccountData`                                          |
| F10                  | float_accounts  | Update against system-owned account                                                           | owner check in `validate_type` (framework)                         | `InvalidAccountOwner`                                         |
| F11a/b               | float_accounts  | Wrong account count (1 or 3)                                                                  | count check (runtime/framework)                                    | `insufficient account keys` / custom 0xfffffffe               |
| F12a                 | float_accounts  | Discriminator 9                                                                               | `parse_instruction` (entrypoint)                                   | `InvalidInstructionData`                                      |
| F12b/c               | float_accounts  | 1-byte / empty instruction data                                                               | exact-width decode (framework)                                     | `InvalidInstructionData`                                      |
| F12d                 | float_accounts  | Envelope version byte = 1 on `Create`                                                         | instruction envelope check (framework)                             | custom 0xfffffff7                                             |
| F13                  | float_accounts  | Spoofed system-program slot                                                                   | `assert_address(&system::ID)` (program)                            | `InvalidAccountData`                                          |
| V1a–V1h              | validation      | Amount boundaries 0/99/1001/1e6/1e6+1/u64::MAX                                                | field rules (2) then policy range (6)                              | custom 2 / custom 6                                           |
| V2a/V3a/V3b          | validation      | Memo len 2 / 65 / 255                                                                         | min_len rule (3) / capacity decode (framework)                     | custom 3 / `InvalidInstructionData`                           |
| V4a/V4b/V4c          | validation      | Approvals equal / count 0 / count 3                                                           | field `exact_len` before cross-field hook — no panic               | custom 4 (`InvalidApprovals`)                                 |
| V4d                  | validation      | Approvals count 5 (capacity 4)                                                                | Vec capacity decode (framework)                                    | `InvalidInstructionData`                                      |
| V5a–V5e              | validation      | min>max, min 0, max 1e6+1, approvals 0/5                                                      | instruction field rules + cross-field hook                         | custom 1 / 2 / 4                                              |
| V6                   | validation      | Wrong bump on re-init                                                                         | `assert_empty` (program, first)                                    | `AccountAlreadyInitialized`                                   |
| V7                   | validation      | Non-PDA policy account with canonical bump                                                    | `assert_canonical_bump`/create seeds (program)                     | seeds do not result in valid address                          |
| V8                   | validation      | Foreign authority's policy PDA                                                                | `PolicyState::load_pda` seeds (framework)                          | seeds do not result in valid address                          |
| V9a/V9b/V9c          | validation      | audit = system / audit = policy / audit read-only                                             | runtime native-writable rule / account hook / `validate(writable)` | `InvalidAccountData`                                          |
| V10a–V10c            | validation      | Unsigned authority, unknown discriminator, empty data                                         | runtime signer rule; `parse_instruction`; decode                   | `missing required signature` / `InvalidInstructionData`       |
| SA2/SA3/SA5/SA6/SA7  | system_accounts | Unsigned authority, sysvar wallet, bad disc, 1 account, empty data                            | runtime signer; `assert_owner`; decode; count                      | corresponding native/custom errors                            |
| SY2/SY3/SY4/SY5/SY6  | sysvar_checks   | Swapped sysvars, wallet as clock, short count, bad disc, empty data                           | `assert_sysvar` = owner(`SYSVAR_ID`) **and** address               | `InvalidAccountData` / `InvalidAccountOwner` / count / decode |
| EV5/EV6              | events          | Unknown discriminator, empty data                                                             | `parse_instruction` + decode                                       | `InvalidInstructionData`                                      |
| CE1–CE9              | custom_errors   | All 7 discriminants return pinned codes; bad disc/empty data                                  | program error mapping                                              | custom 6000/6123/6124/6126/6127/6129/6128; decode             |
| HS2–HS5              | hello_solana    | Unsigned user, bad disc, 0 accounts, empty data                                               | runtime signer; decode; count                                      | native errors                                                 |
| DI2/DI3              | declare_id      | Unknown variant, empty data                                                                   | decode                                                             | `InvalidInstructionData`                                      |
| DP1/DP2/DP3          | declare_program | Unsigned authority; wrong external address; correct-but-nonexistent external                  | runtime signer; address compare; `assert_executable`               | native / `IncorrectProgramId` / `InvalidAccountData`          |
| TD5–TD10             | todo            | Re-init, wrong bump, non-PDA, unsigned owner, bad disc, Initialize-on-update-path             | `assert_empty`; seeds; runtime signer; decode; count               | `AccountAlreadyInitialized` / seeds / signer / decode / count |
| TS1/TS9              | transfer_sol    | Direct lamport drain of foreign wallet (`send_owned`)                                         | ownership check inside `send_owned`                                | `InvalidAccountOwner`                                         |
| TS3                  | transfer_sol    | CPI amount = u64::MAX                                                                         | explicit lamports compare (program)                                | custom 0 (`InsufficientFunds`)                                |
| TS5                  | transfer_sol    | Self-transfer (from == to)                                                                    | system program                                                     | custom 0xfffffff9                                             |
| TS6/TS7/TS8/TS10     | transfer_sol    | Unsigned sender, spoofed system slot, read-only recipient, bad disc                           | runtime signer; `assert_address`; `&mut` writability; decode       | native errors                                                 |
| PF3#8/PF4/PF5        | profile         | 9th tag, remove idx 8, remove idx u64::MAX                                                    | `try_push` bound / `remove` bounds — no panic on `usize::MAX`      | custom 1 (`TagOverflow`) / custom 2 (`TagNotFound`)           |
| PF6/PF7/PF8/PF9/PF10 | profile         | Name len 33 > capacity, unsigned authority, foreign profile, wrong bump, non-existent profile | decode; runtime signer; PDA seeds; seeds; owner                    | decode / signer / seeds / `InvalidAccountOwner`               |

Client-layer blocks (`sign program transaction: not enough signers`, `keypair-pubkey mismatch`) mark attacks that cannot even be constructed without the required signature; the runtime refuses them before the program runs.

---

## Positive properties pinned by probes (worth keeping as tests)

1. **Validation order inside `#[instruction]`**: per-field rules run before the cross-field hook. A raw `approvals` count of 0 returns `InvalidApprovals` (custom 4) — the hook's `approvals[1]` index is never reached, so no panic-on-malformed-input path exists (probes V4b + host decode probe).
2. **`assert_sysvar` is two-factor**: owner must be `SYSVAR_ID` _and_ address must match, so both swapped-sysvar and attacker-owned-fake attacks fail (`crates/pina/src/impls.rs:217-220`).
3. **Direct lamport movement is ownership-gated**: `send_owned` refuses non-program-owned senders — the classic drain attempt on `transfer_sol_program` dies with `InvalidAccountOwner` (TS1).
4. **PDA identity binds the signer everywhere it matters**: wrong bump, non-PDA slot, and foreign-authority PDA all fail seed verification (TD6/TD7/PF8/PF9/V7/V8) — including the stored-bump loaders used by `todo`/`profile` (`load_pda_mut`).
5. **Bounded collections fail closed**: profile tags overflow at capacity 8 with a custom error and `usize::MAX` index removal returns `TagNotFound` rather than panicking (PF3#8/PF4/PF5).

---

## Lint proposals

- **L-P1 `float_finiteness`** (pina_macros/pina lint): a `#[instruction]` or `#[account]` field carrying `f32`/`f64` bits must either declare a finiteness rule (`validate(finite)`) or opt out explicitly (`validate(allow_non_finite)`). Escalate to `warn` (deny-able) when the crate also performs float arithmetic, comparisons, or float→int conversions on the value, where NaN/Inf/subnormals change control flow. Motivated by F-1/F-2.
- **L-P2 `zero_payload_envelope`** (pina_macros): a `#[instruction]` with an empty payload must still validate the migration-envelope version byte, or the struct must carry an explicit `#[pina(envelope(unchecked))]`. Motivated by F-2.
- **L-P3 `empty_guard_strength`** (pina/pina_macros): when `assert_empty` (or `validate(empty)`) guards a field that is subsequently used as a create/allocate target, require the lamports-aware variant (or emit a note that only data is checked). Motivated by F-3.
- **L-P4 `payer_writability`** (pina_macros): an `Accounts` field used as the payer/`from` of a create-account or lamport CPI should be declared `&mut AccountView` (or carry `assert_writable`), so a mis-declared payer fails at the framework boundary instead of inside a CPI. Motivated by F-5's residual (a).

---

## Method / reproduction

```sh
cd /Users/ifiokjr/Developer/projects/pina-rs/pina/worktrees/security-sweep-2026-09-19
devenv shell -- cargo test --manifest-path tmp/sweep/tail-attacks/Cargo.toml -- \
  --ignored --nocapture --test-threads=1
```

Five batteries: `float_accounts_deep_battery` (13 probes), `validation_program_battery` (30), `system_and_sysvar_battery` (13), `longtail_smoke_a` (23), `longtail_smoke_b` (26). Full stdout in `tmp/sweep/tail-attacks/full-run.log`. Deterministic keypairs only (`Keypair::new_from_array`), no tracked files touched. Harness caveats worth remembering for the next sweep: `ProgramTest::send_instruction` does not sign the payer (use `send_with_signers(ix, &[])`), `AccountMeta::new(pubkey, …)`'s second argument is the _signer_ flag, and the typed `initialize` builders run validation at build time — adversarial payloads must be crafted as raw bytes (padded to full field capacity or they die on length checks instead of rules).
