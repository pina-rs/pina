# Migrations

Migration is opt-in and program-owned. Accounts, instruction payloads, and events are normalized into the current representation inside the transaction that touches them; clients never migrate and never pass a version. Use this file as the operational checklist; `docs/src/migrations/flow.md` in the Pina repository carries the full runtime decision tree.

## Envelope

Every opted-in contract writes:

```text
[discriminator][schema version][payload]
```

- The version sits immediately after the discriminator, is little-endian, and is never written by application source. Macros inject it from `migrations/manifest.json`.
- Version `0` is the first captured shape. `pina migrations make` allocates later versions; source code never declares a number.
- The width is program-wide through `[migrations].version-type` in `pina.toml`: `"u8"` (default), `"u16"`, or `"u32"`. `"u64"` is rejected at configuration parse; discriminator width is a separate setting that does support `u64`.
- The width freezes at the first persistent publication, so changing it after release is breaking for every migration-aware contract.

## Opt in

Per contract, add the `migrations` token to the schema attribute:

```rust
#[account(discriminator = AccountType::Profile, migrations)]
#[instruction(discriminator = Instruction::Update, migrations)]
#[event(discriminator = EventKind::Changed, migrations)]
```

`migrations = true` is equivalent. `migrations = false` opts one contract out.

Whole kinds, through `pina.toml`:

```toml
[migrations]
version-type = "u8"
auto = true # every kind, or ["accounts", "events", "instructions"], or false
```

- `auto` accepts `true`, `false`, or a list of exactly `accounts`, `events`, and `instructions`. Unknown names, duplicates, and mixing a boolean with a kind list are configuration errors.
- Staging a subset is meaningful: instruction envelopes change payload bytes and ripple into CPI call sites, so `auto = ["accounts", "events"]` is a useful intermediate step. Event envelopes are immutable and projected rather than rewritten.
- `pina migrations make` records the resolved policy as `auto` in `migrations/manifest.json` (manifest format 4) and snapshots every contract of the listed kinds. Macros read the recorded policy, so a declaration covered by `auto` still fails the build with the `pina migrations make` remedy until it has a snapshot.
- When a policy is recorded, `make` scaffolds a `build.rs`:

```rust
fn main() {
	println!("cargo:rerun-if-changed=migrations/manifest.json");
}
```

Scaffolding is idempotent and never overwrites a hand-written build script; `make` prints the exact line to add instead, and `pina migrations check` fails until the directive is present. The directive exists because proc macros do not re-expand when `pina.toml` or the manifest changes, so a policy flip must trigger expansion from the manifest alone.

- Enabling `auto` on an already-launched program inserts an envelope into every listed contract. `make` records one history entry per newly enveloped contract and the change is a deliberate wire-format change; on a new program it is simply the version-zero baseline.
- `migrations = false` cannot remove an envelope the manifest already records, and dropping a kind from `auto` is rejected the same way. Stripping an envelope is a wire-format change that must be recorded deliberately.

## The loop

Run from the program directory:

```sh
pina migrations make      # snapshot source changes and generate adjacent transitions
pina migrations check     # non-mutating build/CI gate; the same drift checks `pina build` enforces
pina migrations status    # current version and publication state per contract
pina migrations sync      # make -> build -> generate for unambiguous changes
```

- `make` writes `migrations/manifest.json`, `migrations/publications.json`, and `migrations/transitions/<contract>/vN_to_vM.rs`, then prints any manual transition paths and growth warnings. It replaces an unpublished draft in place; once the version appears in a publication receipt or a pending deployment it appends the next version instead.
- `check` fails on source drift, a missing snapshot, an unfinished or hash-changed transition, a changed published schema, a version-width mismatch, or a missing build-script rerun directive.
- `status` prints `kind name vN (draft|published|publication pending)` and confirms the history is consistent; it runs the same checks as `check`. Growth and rent figures are not part of `status`: `make` prints the estimated deficit per growing transition (about 6,960 lamports per grown byte), and `inspect` prints them per pending hop.
- `pina migrations inspect <ADDRESS> [--url <RPC>] [--json]` reads one on-chain account and reports its stored version against the manifest plus the pending adjacent hops with byte sizes and approximate rent delta. It exits non-zero when the account is stale or from the future.
- `pina migrations reconcile [--abandon]` resolves an ambiguous pending deployment. Do not start a different deployment while one is pending.
- `pina test --compatibility` consumes the checked-in historical fixtures. Commit the manifest, publication ledger, and transition files; never generate them during a build.

Ambiguous renames (a field disappears while a same-typed field appears) fail with the exact flags:

```sh
pina migrations make --rename value:points    # preserve the stored bytes
pina migrations make --assume-removed value   # discard them; the new field starts zeroed
```

Answers persist in `[migrations.answers]` in `pina.toml` (`rename = ["value:points"]`, `assume-removed = []`), so fresh clones and CI replay a local decision. With `--json`, `--no-interactive`, or no terminal, an unanswered question is a hard failure: the question array prints on stdout, the human-readable error on stderr, exit status 1.

## Source of truth: the manifest

- `migrations/manifest.json` is the only policy source procedural macros consult; they never read `pina.toml`. It is checked in and hash-chained, which keeps builds deterministic and reproducible.
- A missing, undecodable, or stale manifest fails macro expansion with the `pina migrations make` remedy; the build is blocked rather than silently degraded.
- Do not hand-edit the manifest, the publication ledger, or generated transition files. If `migrations/publications.json` is lost while the manifest records advanced versions, every later check fails because published history must stay pinned; restore the ledger from version control.
- ABI document formats are independent of on-chain versions. Manifest format 3 freezes the PinaPod codec and physical descriptor; format 4 adds the `auto` policy. Publication-ledger format 3 adds the pending deployment record. An ABI document upgrade never consumes an on-chain migration version.

## Generated vs manual transitions

`make` generates automatic transitions for direction-safe fixed-layout changes: exact field copies, safe reordering, zero-fill for unambiguous additions, plus the generated size constants and length guard.

Type changes, compact layouts, and ambiguous moves produce a manual file containing `TODO(pina-manual-migration)`. Replace the body:

- Keep the generated length guard for a fixed transition. Compact transitions also have `target_size` and `working_size`; those inspect already-validated historical bytes and must return a valid destination allocation without mutating the account.
- A manual account `migrate` is total for the accepted source and must fully initialize every active destination byte. It cannot reject once funding or resizing may have happened: a failure there aborts the instruction instead of returning a catchable error. Validate rejectable value constraints in `target_size`/`working_size`, which run before any mutation.
- Instruction and event transitions run in scratch space and may reject invalid semantic values before dispatch.

An unfinished `TODO`, or a transition that no longer matches its recorded SHA-256, blocks macro expansion until the body is filled and `make` re-runs to record the hash. Published transition code and its schemas are immutable; fix a defect with a new version.

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
| `MigrationLamportBudgetExceeded` | The cumulative rent deficit of the planned steps exceeds the `max_lamports` the program passes to `MigrateAccount` or `MigrateContext`.                                                                                     | Raise the program's `max_lamports` constant (for example `MAX_INLINE_MIGRATION_LAMPORTS`) to cover the quoted deficit. `make` prints the estimated deficit for each growing transition.               |

Also expect `MigrationRequired` (a stale account was touched without a migration path) and `InvalidMigrationVersion` (the stored version is unknown or newer than the program).

Builds before the split reported the workspace, growth, and lamport failures as the aggregate `MigrationBudgetExceeded` (`0xFFFF_FFF5`). That code stays reserved so published binaries remain decodable; clients must decode the aggregate code and the three split codes.

## Honest limits

- Data written before the envelope existed is not interpreted as version zero: its first payload bytes could be a valid version by accident. Adopting migrations on a live program is a deliberate wire-format change that needs a new discriminator or a bridge. ADR 0008 tracks the unbuilt `legacy` ergonomics, so do not promise a launched program a transparent adoption.
- Shrinking never refunds lamports, and surplus bytes stay in the account.
- Generated `needsMigration` helpers and the `Migrate` composer implement the catch -> migrate -> retry loop. A one-call `migrateIfNeeded` RPC helper is designed but not built.
- Account history usually cannot be pruned safely because a program cannot enumerate its accounts; retiring account history needs an external completeness proof.
