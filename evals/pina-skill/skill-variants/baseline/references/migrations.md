# Migrations

Migration is opt-in and program-owned. Accounts, instruction payloads, and events are normalized into the current representation inside the transaction that touches them; clients never migrate and never pass a version. Use this file as the operational checklist; `docs/src/migrations/flow.md` in the Pina repository carries the full runtime decision tree.

## Envelope

Every opted-in contract writes:

```text
[discriminator][schema version][payload]
```

- The version sits immediately after the discriminator, is little-endian, and is never written by application source. Macros inject it from `migrations/manifest.json`.
- Version `0` is the first captured shape. `pina migrations create` allocates later versions; source code never declares a number.
- Versions are counted per contract, not per program: every account, instruction, and event owns an independent history that starts at `0`, so a program can hold one contract at version `3` and another at `0`.
- The width is program-wide through `[migrations].version_type` in `pina.toml`: `"u8"` (the default and the recommendation), `"u16"`, or `"u32"`. `"u64"` is rejected at configuration parse; discriminator width is a separate setting that does support `u64`.
- Prefer `u8`: 255 versions of one contract is not a realistic ceiling, and it is the cheapest envelope. Choose a wider width before the first release only when one contract is expected to exceed 255 versions.
- The width freezes at the first persistent publication, so changing it after release is breaking for every migration-aware contract.

## Opt in

Per contract, add the `migrations` token to the schema attribute:

```rust
#[account(discriminator = AccountType::Profile, migrations)]
#[instruction(discriminator = Instruction::Update, migrations)]
#[event(discriminator = EventKind::Changed, migrations)]
```

`migrations = true` is equivalent. `migrations = false` opts one contract out.

`#[discriminator(entrypoint)]` wires the reserved `Migrate` route on its own. The slot ladder is derived from `migrations/manifest.json` (one slot per enveloped account contract, in identity-sorted order, matching generated clients); an explicit `migrations(A, B)` list is an optional override for batching several accounts of one contract in a single sweep:

```rust
#[discriminator(entrypoint)]
pub enum ProgramInstruction {
	// …
}

// Optional ceiling, plus the same-contract batching override.
#[discriminator(entrypoint, migrations(State, State), migrations_max_lamports = 20_000)]
```

The route calls the resize executor, so the program needs `pina`'s `account-resize` feature; `pina init` scaffolds it, and the generated code names it when missing. `migrations_max_lamports` is optional: declaring one caps the reserved instruction's total rent transfers, while omitting it enforces no ceiling — safe because a transfer never exceeds the rent deficit of a growth the runtime caps at `MAX_PERMITTED_DATA_INCREASE`.

Exactly one discriminator enum per program may carry `entrypoint`; `pina build` fails closed when two declare it.

Whole kinds, through `pina.toml`:

```toml
[migrations]
version_type = "u8"
auto = true # every kind, or ["accounts", "events", "instructions"], or false
```

- `auto` accepts `true`, `false`, or a list of exactly `accounts`, `events`, and `instructions`. Unknown names, duplicates, and mixing a boolean with a kind list are configuration errors.
- Staging a subset is meaningful: instruction envelopes change payload bytes and ripple into CPI call sites, so `auto = ["accounts", "events"]` is a useful intermediate step. Event envelopes are immutable and projected rather than rewritten.
- `pina migrations create` records the resolved policy as `auto` in `migrations/manifest.json` (manifest format 4) and snapshots every contract of the listed kinds. Macros read the recorded policy, so a declaration covered by `auto` still fails the build with the `pina migrations create` remedy until it has a snapshot.
- When a policy is recorded, `create` scaffolds a `build.rs`:

```rust
fn main() {
	println!("cargo:rerun-if-changed=migrations/manifest.json");
}
```

Scaffolding is idempotent and never overwrites a hand-written build script; `create` prints the exact line to add instead, and `pina migrations check` fails until the directive is present. The directive exists because proc macros do not re-expand when `pina.toml` or the manifest changes, so a policy flip must trigger expansion from the manifest alone.

- Enabling `auto` on an already-launched program inserts an envelope into every listed contract. `create` records one history entry per newly enveloped contract and the change is a deliberate wire-format change; on a new program it is simply the version-zero baseline.
- `migrations = false` cannot remove an envelope the manifest already records, and dropping a kind from `auto` is rejected the same way. Stripping an envelope is a wire-format change that must be recorded deliberately.

## The loop

Run from the program directory:

```sh
pina migrations create    # snapshot source changes and generate adjacent transitions
pina migrations check     # non-mutating build/CI gate; the same drift checks `pina build` enforces
pina migrations status    # version + publication state per contract, plus a cost preview
pina migrations sync      # create -> build -> generate for unambiguous changes
```

- `create` writes `migrations/manifest.json`, `migrations/publications.json`, and `migrations/transitions/<contract>/vN_to_vM.rs`, then prints any manual transition paths and growth warnings. It replaces an unpublished draft in place; once the version appears in a publication receipt or a pending deployment it appends the next version instead.
- `check` fails on source drift, a missing snapshot, an unfinished or hash-changed transition, a changed published schema, a version-width mismatch, or a missing build-script rerun directive. `check --json` stays a status-only array.
- `status` runs the same drift checks as `check`, prints `kind name vN (draft|published|publication pending)`, and ends with the cost preview below. `status --json` keeps the status array unchanged under `statuses` and adds a `costPreview` object.
- `pina migrations inspect <ADDRESS> [--url <RPC>] [--json]` reads one on-chain account and reports its stored version against the manifest plus the pending adjacent hops with byte sizes and approximate rent delta. It exits non-zero when the account is stale or from the future.
- `pina migrations reconcile [--abandon]` resolves an ambiguous pending deployment. Do not start a different deployment while one is pending.
- `pina test --compatibility` consumes the checked-in historical fixtures. Commit the manifest, publication ledger, and transition files; never generate them during a build.

Ambiguous renames (a field disappears while a same-typed field appears) fail with the exact flags:

```sh
pina migrations create --rename value:points    # preserve the stored bytes
pina migrations create --assume-removed value   # discard them; the new field starts zeroed
pina migrations create --manual value           # write the conversion by hand
```

`--manual <field>` names an **added** field and makes the transition manual, which is how a conversion such as merging two fields into one is written: pair it with `--rename`/`--assume-removed` to say which stored fields feed the new value, then complete the generated stub. It also legalizes a rename whose type changed, which a generated byte copy cannot express. The answer is recorded, so repeated `create` runs keep the manual draft instead of regenerating an automatic transition over your body.

Answers persist in `[migrations.answers]` in `pina.toml` (`rename = ["value:points"]`, `assume_removed = []`, `manual = ["value"]`), so fresh clones and CI replay a local decision. With `--json`, `--no-interactive`, or no terminal, an unanswered question is a hard failure: the question array prints on stdout, the human-readable error on stderr, exit status 1.

## Cost preview

`pina migrations status` ends with a static planning estimate, never a quote. It derives from the checked-in manifest and, when present, `pina profile`'s per-symbol estimates of the compiled SBF artifact, and it prints the two models alongside the figures so they stay interpretable:

- Per account contract: current size (compact schemas quote their declared capacity), the bytes a version-0 day-one account grows, and that growth's rent deficit at the same ~6,960 lamports per grown byte the `create` warning uses.
- Per instruction process: the worst-case ladder a stale account it names can trigger — the oldest version within `MAX_INLINE_STEPS` (8) of current, one adjacent transition per step — with its step count, rent, and static CU estimate. A history with more than eight transitions quotes a ladder that starts partway up and adds a note that a day-one account instead fails with `MigrationUnavailable`; the day-one growth figure still counts every pending byte.
- Program-wide: the touching transaction funding the most rent (size `max_lamports` from it) and, independently, the instruction with the longest worst-case ladder (size `MAX_INLINE_STEPS` from it). They need not be the same instruction, and each names its own.

The static CU figure sums `pina profile`'s estimates of the generated `vN_to_vM` `migrate` functions and excludes executor overhead (resize, rent transfer, validation) and runtime branch or loop effects. It is never a misleading zero: a missing or unprofilable artifact, or a transition symbol without an estimate, prints `CU unavailable: <reason>` instead. A figure that cannot be estimated serializes as `{ "status": "unavailable", "reason": "..." }`.

Instruction processes link to account contracts by account-slot name, and only a writable, non-signer slot can hold an account the executor migrates. A writable, non-signer slot that names no checked-in account contract produces an explicit note — the symptom of a renamed account or a slot typo; signer and read-only slots are skipped without noise. A note also flags any single step whose growth exceeds `MAX_PERMITTED_DATA_INCREASE` (10,240 bytes).

## Source of truth: the manifest

- `migrations/manifest.json` is the only policy source procedural macros consult; they never read `pina.toml`. It is checked in and hash-chained, which keeps builds deterministic and reproducible.
- A missing, undecodable, or stale manifest fails macro expansion with the `pina migrations create` remedy; the build is blocked rather than silently degraded.
- Do not hand-edit the manifest, the publication ledger, or generated transition files. If `migrations/publications.json` is lost while the manifest records advanced versions, every later check fails because published history must stay pinned; restore the ledger from version control.
- ABI document formats are independent of on-chain versions. Manifest format 3 freezes the PinaPod codec and physical descriptor; format 4 adds the `auto` policy. Publication-ledger format 3 adds the pending deployment record. An ABI document upgrade never consumes an on-chain migration version.

## Generated vs manual transitions

`create` generates automatic transitions for direction-safe fixed-layout changes: exact field copies, safe reordering, zero-fill for unambiguous additions, plus the generated size constants and length guard. Byte offsets and `SOURCE_SIZE` always come from the **stored** schema, so a field removed from the middle of a layout leaves the fields after it readable at their original offsets. Type changes, compact layouts, ambiguous moves, and a `--manual` answer produce a manual file containing `TODO(pina-manual-migration)`. Replace the body:

- Keep the generated length guard for a fixed transition. Compact transitions also have `target_size` and `working_size`; those inspect already-validated historical bytes and must return a valid destination allocation without mutating the account.
- A manual account `migrate` is total for the accepted source and must fully initialize every active destination byte. It cannot reject once funding or resizing may have happened: a failure there aborts the instruction instead of returning a catchable error. Validate rejectable value constraints in `target_size`/`working_size`, which run before any mutation.
- Instruction and event transitions run in scratch space and may reject invalid semantic values before dispatch.

An unfinished `TODO`, or a transition that no longer matches its recorded SHA-256, blocks macro expansion until the body is filled and `create` re-runs to record the hash. Published transition code and its schemas are immutable; fix a defect with a new version.

## Runtime behavior

- The current-version hot path is one envelope comparison with a generated constant; no history scan.
- A stale account fails the current-only loaders (`as_account`, `as_account_mut`, generated `try_from_bytes`) with `MigrationRequired`. The instruction that touches it migrates it explicitly, or a client prepends the reserved `Migrate` instruction:

```rust
MigrateAccount {
	account,
	payer: Some(&payer),
	program_id: &ID,
	max_lamports: MAX_INLINE_MIGRATION_LAMPORTS,
}
.invoke::<State>()?;
```

Each account climbs its own ladder one adjacent step at a time: plan from the exact historical layout without retaining borrows, fund the rent deficit, resize, apply, validate the destination, then commit the version last. Accounts in one instruction migrate independently. A failure after the first mutation aborts the instruction so Solana rolls back every resize, transfer, and byte write. The payer is the instruction's declared migration payer, a signer or a program PDA signing through `invoke_signed`; growth transfers only the deficit, capped by `max_lamports`, and never refunds. A client generated before the instruction gained its optional migration payer cannot fund growth, and its transaction fails with `MigrationRequired`.

- Historical instruction payloads project forward through generated `T::with_current_instruction_data(data, |current| ...)`: the workspace is cleared, each adjacent step runs and validates, and the current version marker is committed last. Unknown and future versions are rejected without trial decoding. Handlers always see current types.
- Events are never rewritten on chain. `normalize_event_data` and generated `with_current_event_data` project historical log bytes into caller-owned scratch space and report `source_version()` (which schema actually emitted the log) and `was_migrated()`. Keep golden log bytes in `pina_test::HistoricalEvent`.
- The instruction process contract stays compatible only when existing account slots form an identical prefix and every new slot is appended at the end and optional. Any other change (reorder, insertion, removal, rename, signer/writable/PDA/known-address change, new required account) needs a new instruction discriminator.
- A rolled-back binary does not roll back accounts. Accounts written at version N are future versions for a binary whose current is N-1 and are rejected; rollback means redeploying an executable built from the current ABI history.

## Read-only accounts

Migration asserts writability and program ownership, so a stale account an instruction only reads cannot be migrated there. Write one sweep instruction whose migratable accounts are optional writable views, plus a payer and the system program, and call `MigrateAccount::invoke::<T>()` per present account; then send `[sweep(...), real_instruction(...)]` in one transaction, sweep first, because the real instruction's load fails with `MigrationRequired` if it runs against stale bytes. The reserved `Migrate` instruction is the framework-owned instance of this pattern and generated clients compose it.

See the repository's sweep documentation for the full pattern: https://github.com/pina-rs/pina/blob/main/docs/src/migrations/flow.md#the-sweep-instruction

If a writable touch genuinely cannot be scheduled, generated accounts expose a read-only alternative: `Account::try_from_bytes_versioned(bytes)` returns an `AccountVersioned` enum with one variant per historical version plus `Current`. It never mutates, resizes, or requires a writable borrow, and fails closed on foreign discriminators, unknown or future versions, and malformed representations. The caller must handle every stored representation, so prefer migrating whenever a writable touch is schedulable.

## Budget failures

| Error                            | Raised when                                                                                                                                                                                                                 | Remedy                                                                                                                                                                                                |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `MigrationUnavailable`           | A stale account is further behind than the generated `MAX_INLINE_STEPS` (at most 8 adjacent transitions), the plan is not exactly the next adjacent step, or a reserved-instruction slot index is past the 63-slot bitmask. | Rebalance the history into more frequent, smaller versions and migrate accounts before they fall further behind. The reserved `Migrate` route is the out-of-band path but enforces the same step cap. |
| `MigrationWorkspaceExceeded`     | A generated transition's `WORKING_SIZE` is below its `CURRENT_SIZE`, or a caller-supplied workspace is shorter than `WORKING_SIZE`.                                                                                         | Give the workspace at least `WORKING_SIZE` bytes. Generated `with_current_*` helpers already size it at the compile-time maximum, which stays at or below `MAX_MIGRATION_WORKSPACE` (1,024 bytes).    |
| `MigrationAccountGrowthExceeded` | One step would grow the account past the runtime reallocation limit, `MAX_PERMITTED_DATA_INCREASE` (10,240 bytes), over the account's size at instruction start.                                                            | Publish intermediate versions so the change grows across separate transactions; no lamport budget raises this limit.                                                                                  |
| `MigrationLamportBudgetExceeded` | The cumulative rent deficit of the planned steps exceeds the `max_lamports` the program passes to `MigrateAccount` or `MigrateContext`.                                                                                     | Raise the program's `max_lamports` constant (for example `MAX_INLINE_MIGRATION_LAMPORTS`) to cover the quoted deficit. `create` prints the estimated deficit for each growing transition.             |

Also expect `MigrationRequired` (a stale account was touched without a migration path) and `InvalidMigrationVersion` (the stored version is unknown or newer than the program).

Builds before the split reported the workspace, growth, and lamport failures as the aggregate `MigrationBudgetExceeded` (`0xFFFF_FFF5`). That code stays reserved so published binaries remain decodable; clients must decode the aggregate code and the three split codes.

## Version exhaustion

Versions are counted per contract, not per program: each account, instruction, and event owns an independent history starting at `0`, so a `u8` program gives every contract its own 255-version budget. `pina migrations status` prints what is left (`account State v3 (published, 252 version(s) remaining)`), and `status --json`/`check --json` carry it as `versionsRemaining`.

The width is program-wide and freezes at the first persistent publication, so it cannot be widened after release. Pre-launch, the only widening path is deleting `migrations/` and re-running `create` with the wider setting, which re-baselines history; there is nothing deployed to stay compatible with yet.

Reaching the ceiling is terminal for that contract. `create` fails closed with `VersionExhausted` and the on-chain path rejects out-of-range versions instead of truncating, so no version number is ever reused. The remedy is a successor contract: a new discriminator with a fresh version-0 history, plus a bridge instruction that reads the exhausted account through the current loaders and writes the successor, with clients sweeping accounts lazily. History cannot be pruned, because a program cannot enumerate its own accounts.

## Honest limits

- Data written before the envelope existed is not interpreted as version zero: its first payload bytes could be a valid version by accident. Adopting migrations on a live program is a deliberate wire-format change that needs a new discriminator or a bridge. ADR 0008 tracks the unbuilt `legacy` ergonomics, so do not promise a launched program a transparent adoption.
- Shrinking never refunds lamports, and surplus bytes stay in the account.
- Generated `needsMigration` helpers and the `Migrate` composer implement the catch -> migrate -> retry loop. A one-call `migrateIfNeeded` RPC helper is designed but not built.
- Account history usually cannot be pruned safely because a program cannot enumerate its accounts; retiring account history needs an external completeness proof.
