# How a migration flows

This page is the map of the whole migration system: what happens when a developer changes a contract, what the program does at runtime, and exactly what is expected from clients. Everything described here is the implemented behavior — [ADR 0008](../adrs/0008-migration-ux-and-legacy-adoption.md) collects the parts that are designed but not built yet.

An interactive version of this page is at [flow-interactive.html](./flow-interactive.html).

## The one-paragraph answer

**Migration happens on-chain, inside the program, on demand.** A client never migrates account data and never needs to know a migration exists. Every wire carrier — account, instruction payload, event — carries a version envelope right after its discriminator, every client writes the version it was generated with, and the program normalizes whatever arrives into its current representation inside the transaction that touched it. If the transaction fails, Solana rolls the migration back with everything else.

## Development-time flow

### Opting in

One contract opts in with the `migrations` token; a whole program opts in through `[migrations].auto`:

```toml
[migrations]
version_type = "u8"
auto = true # or ["accounts", "events", "instructions"], or a staged subset
```

`pina migrations make` records the policy in `migrations/manifest.json` and snapshots every contract of the listed kinds, so the manifest stays the checked-in source of truth that macros consult. A declaration the manifest does not record yet still fails the build with the `pina migrations make` remedy. Because a proc macro does not re-expand when `pina.toml` changes, a program with a policy also gets a build script emitting `cargo:rerun-if-changed=migrations/manifest.json`; `make` scaffolds it or reports the exact line when a hand-written build script must be edited. Explicit `migrations = false` overrides the policy for one contract, and removing an envelope the manifest already records fails closed as a wire-format change.

```text
                 ┌──────────────────────────────┐
                 │ developer changes a struct,  │
                 │ instruction, or event        │
                 └──────────────┬───────────────┘
                                │
                                ▼
                 ┌──────────────────────────────┐
                 │ `pina build` / macro drift   │
                 │ gate fails:                  │
                 │ "run pina migrations make"   │
                 └──────────────┬───────────────┘
                                │
                                ▼
                 ┌──────────────────────────────┐
        ┌────────│   `pina migrations make`     │─────────┐
        │        └──────────────┬───────────────┘         │
        │                       │                         │
        │ ambiguous change?     │ plain change            │ unpaired removal
        ▼                       ▼                         ▼
┌───────────────┐   ┌────────────────────────┐   ┌──────────────────┐
│ terminal:     │   │ automatic transition   │   │ fails with the   │
│ prompt per    │   │ generated (byte moves, │   │ exact flag:      │
│ field         │   │ zero-fill, relayout)   │   │ --assume-removed │
│               │   └───────────┬────────────┘   └────────┬─────────┘
│ no terminal:  │               │ type change / compact?  │
│ fails naming  │               ▼                         │
│ --rename a:b  │   ┌────────────────────────┐            │
│ or            │   │ manual TODO transition │            │
│ --assume-     │   │ developer fills body,  │            │
│  removed a    │   │ re-runs make to record │            │
└───────┬───────┘   │ its hash               │            │
        │           └───────────┬────────────┘            │
        └─────────────┬─────────┴─────────────────────────┘
                      ▼
        ┌──────────────────────────────┐
        │ `pina migrations check`      │
        │ drift, hashes, publication   │
        │ pins all verified            │
        └──────────────┬───────────────┘
                       ▼
        ┌──────────────────────────────┐
        │ `pina build`                 │
        │ program compiles with the    │
        │ manifest + transitions       │
        │ embedded (include! bytes)    │
        └──────────────┬───────────────┘
                       ▼
        ┌──────────────────────────────┐
        │ `pina deploy`                │
        │ 1. pending record written    │
        │    atomically (versions      │
        │    frozen — may already be   │
        │    live)                     │
        │ 2. remote command runs       │
        │    (solana program deploy or │
        │    --remote-command)         │
        │ 3. immutable receipt with    │
        │    pinned schema history     │
        └──────────────┬───────────────┘
                       ▼
        ┌──────────────────────────────┐
        │ `pina codama generate`       │
        │ TypeScript, Rust, Dart, CPI  │
        │ clients embed the CURRENT    │
        │ version as an omitted        │
        │ constant — callers never     │
        │ pass a version               │
        └──────────────────────────────┘
```

Once a version appears in a receipt or pending record it is frozen: `make` appends the next version instead of rewriting it, and any edit to a pinned schema or transition hash fails every later check.

### Persisted answers

Disambiguation answers do not have to travel as flags every run. A `[migrations.answers]` table in `pina.toml` persists them for the whole checkout:

```toml
[migrations.answers]
rename = ["value:points"]
assume_removed = []
```

`make` consults the table before prompting, command-line flags override it per field, and a flag that contradicts a persisted rename (for example `--assume-removed value` when the file renames `value`) fails closed. Fresh clones and CI therefore replay an answer made locally without anyone re-deriving flag lists, and `--json` failures print a machine-actionable envelope carrying the message plus the exact outstanding questions.

## Runtime flow inside the program

This is the generated dispatcher's decision tree for one instruction invocation:

```text
                transaction arrives
                        │
                        ▼
        ┌───────────────────────────────┐
        │ read discriminator, dispatch   │
        │ to the instruction             │
        └───────────────┬───────────────┘
                        ▼
        ┌───────────────────────────────┐
        │ inspect instruction version    │
        │ (one read after the disc)      │
        └───────┬───────────┬───────────┘
        current │    stale   │   future/corrupt
                ▼           ▼               ▼
      ┌──────────────┐ ┌──────────────────┐ ┌────────────────┐
      │ validate &   │ │ normalize:       │ │ reject now —   │
      │ run handler  │ │ zero workspace,  │ │ no trial       │
      │ (hot path:   │ │ walk adjacent    │ │ decoding       │
      │ no history   │ │ payload steps,   │ └────────────────┘
      │ scans)       │ │ validate each,   │
      └──────┬───────┘ │ commit current   │
             │         │ version marker    │
             │         └────────┬─────────┘
             │                  ▼
             │    ┌───────────────────────────────┐
             │    │ for each migratable account   │
             │    │ the route touches:            │
             │    │                               │
             │    │  version == current?          │
             │    │    yes → validate, done       │
             │    │    no  → PLAN                 │
             │    │      (validate exact source   │
             │    │       shape, owned detached   │
             │    │       state, no borrows)      │
             │    │          │                    │
             │    │          ▼                    │
             │    │    need growth & rent?        │
             │    │      yes → explicit capped    │
             │    │      payer transfers deficit  │
             │    │          │                    │
             │    │          ▼                    │
             │    │    RESIZE → APPLY (infallible)│
             │    │    → VALIDATE destination     │
             │    │    → WRITE version → re-      │
             │    │    VALIDATE                   │
             │    │          │                    │
             │    │          ▼                    │
             │    │    next adjacent version      │
             │    │    until current              │
             │    └──────────────┬────────────────┘
             │                   │
             └────────┬──────────┘
                      ▼
        ┌───────────────────────────────┐
        │ run current handler with      │
        │ current types only            │
        └───────────────────────────────┘

The version envelope supports `u8`, `u16`, and `u32` encodings (program-wide, `u8` by default) — never `u64`.

  failure BEFORE first mutation  → ordinary ProgramError
  failure AFTER first mutation   → instruction ABORTS (never a
                                   catchable error) so the transaction
                                   rolls back every resize, transfer,
                                   and byte write together
```

Accounts in one instruction migrate independently — a mixed set (`Profile@v0`, `Journal@v1`, …) each climb their own ladder atomically within the same invocation.

## What is expected from each side

|                     | Client (generated)                                                   | Program (generated + handler)                                                                                                   |
| ------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Version envelope    | writes its frozen version automatically; caller never sees it        | reads it before any payload decode                                                                                              |
| Account data        | never migrates, never rewrites; **refuses to decode** other versions | migrates on demand, on-chain, in-transaction                                                                                    |
| Instruction payload | encodes args in its version's shape                                  | normalizes stale payloads into current args                                                                                     |
| Rent for growth     | supplies an explicit payer account when the instruction declares one | transfers only the deficit, capped, never refunds                                                                               |
| Old clients         | keep sending their old bytes unchanged                               | are the reason the whole system exists                                                                                          |
| Breaking cases      | —                                                                    | fail closed: unknown/future versions, privilege changes, unprovable process changes are rejected or require a new discriminator |

## Paying for growth

Migration that only moves bytes is free beyond compute. Migration that makes an account **bigger** must also make it rent-exempt at its new size, and that lamports transfer happens inside the touching transaction:

- the program transfers only the **deficit** (rent-exempt minimum at the new size minus what the account already holds), never more;
- the payer is the instruction's declared migration payer — a transaction signer, or one of the program's own PDAs signing through `invoke_signed`;
- the transfer is capped by the `max_lamports` budget the program passes to the executor. An undersized budget fails the whole transaction with `MigrationLamportBudgetExceeded` — nothing is half-migrated — but the account also stays stale until the budget is raised, so size it deliberately. The error names the budget to raise, and `pina migrations make` quotes the same deficit figure before you deploy.

A planning figure for that budget: rent exemption costs about **6,960 lamports per byte** (3,480 lamports per byte-year at the two-year exemption threshold), so growing an account by _N_ bytes needs roughly `N × 6,960` lamports of head room on top of what the account already holds. `pina migrations make` prints a warning with the exact growth and estimate whenever a transition grows an account or keeps a compact (capacity-driven) layout, and adds the remedy for the failure the grown account would hit on chain.

The executor keeps the workspace, realloc-growth, and lamport budgets as separate codes, so each failure names one fix:

| Failure condition                                                                                | Code                             | Remedy                                                                                                      |
| ------------------------------------------------------------------------------------------------ | -------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| The stale account is more than `MAX_INLINE_STEPS` versions behind                                | `MigrationUnavailable`           | Publish smaller, more frequent versions; migrate accounts before they fall further behind.                  |
| The normalization workspace cannot hold the generated transition                                 | `MigrationWorkspaceExceeded`     | Keep the generated workspace at or below `MAX_MIGRATION_WORKSPACE` (1,024 bytes).                           |
| One instruction would grow the account by more than `MAX_PERMITTED_DATA_INCREASE` (10,240 bytes) | `MigrationAccountGrowthExceeded` | Publish intermediate versions so each transition migrates in a separate transaction; no budget raises this. |
| The rent deficit exceeds the program's `max_lamports` budget                                     | `MigrationLamportBudgetExceeded` | Raise the program's `max_lamports` constant to cover the quoted deficit.                                    |

Builds before the split reported the workspace, account-growth, and lamport-budget failures as `MigrationBudgetExceeded` (`0xFFFF_FFF5`); `MigrationUnavailable` was already its own code and was not part of that split. The aggregate code stays reserved so published binaries remain decodable.

The growth check covers the whole supported ladder, not one hop: the executor captures the account size before the first step, so a v0 account walking two individually sub-limit transitions can still cross `MAX_PERMITTED_DATA_INCREASE` in one instruction. `pina migrations make` warns on that cumulative worst case, and publishing an intermediate version only resets the cap when it migrates in its own transaction.

Two consequences follow from the payer model:

1. **An old client cannot fund growth.** A client generated before the instruction gained its optional `migrationPayer` slot submits without a payer; if the account it touches now needs rent, that transaction fails with `MigrationRequired`. The fix is a current client (or a payer-carrying migration path) — by design, rent is never taken from an account the client did not offer.
2. **The compute bill lands on the touching transaction.** A stale account pays its ladder's compute cost inside whichever transaction finds it, and `MAX_INLINE_STEPS` (at most 8 adjacent transitions) bounds how long that ladder may be. Accounts further behind than that fail with `MigrationUnavailable` rather than silently burning the budget; the remedy is rebalancing the history into more frequent, smaller versions. The reserved `Migrate` instruction below is the out-of-band route for carrying a payer, but it enforces the same step limit.

### The reserved Migrate instruction

Pina reserves the all-ones value of every instruction discriminator width (`0xff`, `0xffff`, `0xffff_ffff`, `0xffff_ffff_ffff_ffff`) for a framework migration instruction; `#[discriminator]` rejects user variants that claim it at compile time. A program with migratable accounts wires the reserved route before parsing its own instruction enum:

```rust,ignore
if is_migrate_instruction(data) {
    return process_migrate(program_id, accounts);
}
```

Its account layout is `[payer, systemProgram, accountA, accountB, …]`. Slot 0 is a writable payer funding every rent deficit (or the program address when the invocation needs no funding), slot 1 is the system program the rent transfers invoke, and each later slot is a program-owned, self-describing migratable account.

**The ladder is derived, not declared.** `#[discriminator(entrypoint)]` wires the route on its own: the slots are read from `migrations/manifest.json`, one per enveloped account contract, in the manifest's identity-sorted order — exactly the order generated clients compose. Declaring a contract list is optional and only needed to batch several accounts of the _same_ contract in one sweep, because the manifest records contracts rather than account instances:

The route calls the resize executor, so a program that serves migrations needs `pina`'s `account-resize` feature. `pina init` scaffolds it; the generated code names it when it is missing.

```rust,ignore
#[discriminator(entrypoint)]
pub enum Instruction { /* … */ }

// Optional ceiling, and optional slot override for same-contract batching.
#[discriminator(
	entrypoint,
	migrations(State, State),
	migrations_max_lamports = MAX_INLINE_MIGRATION_LAMPORTS,
)]
```

`migrations_max_lamports` is optional and off by default. Declaring one caps the total lamports the reserved instruction may transfer; leaving it out enforces no ceiling, which is safe because a transfer is never more than the rent deficit of a growth the runtime already caps at `MAX_PERMITTED_DATA_INCREASE`. Declare one to refuse an expensive migration rather than to permit it. A slot holding the program address (the placeholder generated clients write for an omitted optional account) or an index past the end of the list is skipped, so a client sends only the accounts it needs. `MigrateContext` validates ownership, rejects duplicated account slots, migrates each slot at most once, and runs each slot through the same `MigrateAccount` executor — the same step, growth, and lamport caps as the inline path.

That makes the client flow explicit: when an account is stale and the business instruction cannot carry a payer, prepend `[Migrate { payer }, …real instructions]` in the same transaction — the payer authorizes exactly the migration cost, and the real instruction observes current data or the whole transaction fails.

### Reading without migrating

Migration is a mutation, and the runtime forbids writes and resizes on a read-only account. A program that only reads a stale account therefore has no migration path in that instruction. Generated account code now offers a read-only alternative: `<Account>::try_from_bytes_versioned(bytes)` validates the exact representation named by the version envelope and borrows it immutably. It never rewrites, resizes, or clears anything, and it never requires a writable borrow; foreign discriminators, unknown versions, future versions, and malformed lengths all fail closed.

The generated `<Account>Versioned` enum has one variant per historical version plus `Current`, so the caller must handle every stored representation:

```rust,ignore
let data = account.try_borrow()?;
match State::try_from_bytes_versioned(&data)? {
    StateVersioned::V0(view) => read_v0(view),
    StateVersioned::V1(view) => read_v1(view),
    StateVersioned::Current(view) => read_current(view),
}
```

**This is a deliberate trade-off, not a replacement for migration.** A program that reads historical layouts must handle two (or more) representations in its business logic, which is exactly the branching the migration system exists to remove. Treat the view as the complement for read-heavy accounts whose one-time writable touch is genuinely hard to schedule; the reserved `Migrate` instruction remains the primary fix. The accessor is generated only for accounts declared with `migrations`.

### Generated client helpers

Generated clients turn that flow into a one-call routine. Next to each migratable account module the TypeScript, Dart, and Rust clients emit:

- a `<Account>MIGRATION_VERSION` constant — the schema version the client was generated from;
- the generic `needsMigration` envelope check is that per-account helper (`stateNeedsMigration` for a `State` account, and so on);
- `<account>NeedsMigration(bytes)` — a cheap envelope check that returns true only when the bytes name this account's discriminator and a version older than the client's schema. Future versions and foreign discriminators return false; the decoder explains those when the account is decoded.

The clients also emit a `Migrate` instruction composer (TypeScript `getMigrateInstruction`, Dart `getMigrateInstruction`, Rust `Migrate::new().instruction()`). Migratable **events** get the same envelope treatment on the read path: transaction logs are immutable, so instead of migrating them each client emits a log entry point (`parse<Program>EventsFromLogs` in TypeScript and Dart, `project_from_bytes` in Rust) that enforces the version envelope and, when the checked-in migration manifest proves the transition is automatic, projects historical bytes into the current shape. The decoded record reports `sourceVersion` and `wasMigrated`, mirroring the runtime's `CurrentEventData::source_version`. Manual transitions are the documented limit: generated clients cannot represent them, so those log versions fail closed with a message naming the transition. Every migratable slot is optional: omitted slots become program-address placeholders and trailing omitted slots are truncated, so a client sends only the accounts it needs. The intended catch → migrate → retry loop:

```ts
const { data } = await fetchEncodedAccount(rpc, address);
if (stateNeedsMigration(data)) {
	await send(getMigrateInstruction({ state: address, payer }).make());
}
// now decode `state` and send the real instruction
```

`migrateIfNeeded` — fetching, checking, and migrating in one call over an RPC handle — is designed in [ADR 0008](../adrs/0008-migration-ux-and-legacy-adoption.md).

## The sweep instruction

A typed reader refuses a stale account: `as_account`, `as_account_mut`, and `validate_account_data` all inspect the version envelope and return `MigrationRequired` for anything older than the program's current schema. The refusal happens wherever the handler loads the account, and it does not care whether the account was passed writable or read-only. Migration itself does care: `MigrateAccount` asserts writability and program ownership before it inspects a single byte, because the executor may rewrite, resize, and fund the account. An instruction that treats a migratable account as read-only therefore has no way to bring it current, and keeps failing until some other transaction migrates the account once. The **sweep instruction** is that other transaction: one instruction whose only job is putting migratable accounts in a writable position and running the executor on each.

### The instruction

Write one sweep per program with its own discriminator. Every migratable account appears as an optional, writable view, the payer funding growth appears once, and the system program the rent transfers invoke is declared:

```rust,ignore
#[discriminator]
pub enum SwapInstruction {
	Swap = 0,
	SweepAccounts = 1,
}

#[derive(Accounts)]
pub struct SweepAccounts<'a> {
	/// Writable signer funding every rent deficit, usually the fee payer.
	#[pina(validate(signer))]
	pub payer: Option<&'a mut AccountView>,
	/// The system program every growth transfer invokes.
	pub system_program: &'a AccountView,
	/// Every migratable account this program owns, writable and optional.
	pub state: Option<&'a mut AccountView>,
	pub profile: Option<&'a mut AccountView>,
	pub vault: Option<&'a mut AccountView>,
}

impl<'a> ProcessAccountInfos<'a> for SweepAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let SweepAccounts {
			payer,
			system_program,
			state,
			profile,
			vault,
		} = self;
		system_program.assert_address(&system::ID)?;
		let payer = payer.map(|account| &*account);

		if let Some(state) = state {
			MigrateAccount {
				account: state,
				payer,
				program_id: &ID,
				max_lamports: MAX_MIGRATION_LAMPORTS,
			}
			.invoke::<State>()?;
		}
		// The same call for profile and vault: one independent invoke each.

		Ok(())
	}
}
```

The handler is a straight line of independent calls. Each `invoke::<T>()` is complete on its own: it validates the account, plans the adjacent transitions, and commits the current version. An absent account arrives as `None` and the call is skipped, so nothing is validated, charged, or written for it. A missing middle slot still needs the program-address filler the account parser expects; trailing absent slots can be omitted.

The payer must be writable and a transaction signer, or a PDA signing through `invoke_signed`. `max_lamports` caps what one call may transfer from it, so a sweep pays at most the sum of its calls' caps — set each one deliberately.

### The client sends it first

Prepend the sweep, then send the real instruction in the same transaction:

```text
[sweep(state, profile, payer), swap(...)]
```

First, not last: instructions run in order and any failure aborts the whole transaction. The real instruction loads the stale account and fails with `MigrationRequired`, so a sweep placed after it never runs. The sweep can carry every account the transaction might touch; the real instruction then observes current bytes, or the whole transaction — migrations included — rolls back. A sweep sent alone, with no business instruction, is a maintenance transaction that pre-migrates accounts ahead of future use. That standalone form is the only trailing sweep worth sending.

### What happens per account

| Account state    | What the sweep call does                                                                                                                                               |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Absent           | Skipped by the handler; nothing is validated, charged, or written                                                                                                      |
| Already current  | Ownership, writability, and discriminator checks, a version-envelope inspection and current-layout validation; no writes, no rent, no CPI — safe to send speculatively |
| Stale, same size | Planned from the exact historical layout, rewritten in place, destination validated, current version committed                                                         |
| Stale, growing   | The payer transfers only the rent deficit for the size the step needs, capped by `max_lamports`; the account is resized, rewritten, and the version committed          |
| Stale, shrinking | Rewritten and shrunk; every lamport stays in the account, there is no refund today                                                                                     |
| Any step fails   | The instruction fails and the transaction rolls back, including sweeps that already completed in the same instruction                                                  |

The failures are sharp and in front of the first mutation:

- a growing step with no payer fails with `MigrationRequired`; a deficit above `max_lamports` fails with `MigrationBudgetExceeded` — the account stays stale, nothing is half-migrated;
- a read-only or foreign-owned account fails the writability or ownership assertion before the version is even read, and a wrong discriminator fails with `InvalidAccountData`;
- a version newer than the program's current schema fails with `InvalidMigrationVersion` instead of guessing at the layout.

### Why it is safe to expose

Every account is owner-checked and writability-checked before anything is inspected. Transition planning validates the exact historical shape and accepts only adjacent steps the published history proves; the destination representation is validated before the current version marker is committed, and an invariant failure after the first byte or lamport moves aborts the instruction instead of returning an error the caller could catch. The payer can only add lamports — the deficit, never more than `max_lamports` — and shrinking never moves lamports at all. Because each caller supplies the payer it wants to charge, a sweep cannot spend an account its sender did not offer: nobody can be made to fund someone else's migration.

### Forward compatibility

The reserved `Migrate` instruction above is the framework-owned instance of this pattern. `MigrateContext` runs the same `[payer, systemProgram, …accounts]` layout through the same `MigrateAccount` executor, and `run_optional` treats a slot holding the program address — or an index past the end of the list — as absent, so it is also a sweep. Generated clients emit a `Migrate` composer and the per-account `needsMigration` checks, so the standard sweep needs no hand-composed metas; the one-call `migrateIfNeeded` wrapper is tracked in #339. A hand-written sweep stays compatible with that route because it drives the same executor and produces exactly the same account state. Prefer it when the handler needs policy the generated route does not carry — extra gates, per-account caps, or a different payer rule — and prefer the reserved instruction when it does not.

## The four scenarios

### 1. Current client → current program (the hot path)

```text
client encodes v_N payload ──► program sees version == current
                                   │
                                   ▼
                          validate current shape
                                   │
                                   ▼
                          handler runs — zero
                          migration work done
```

One version-byte comparison is the entire overhead.

### 2. Old client → updated program (the migration path)

```text
old client encodes v_0 payload
        │
        ▼
program sees 0 < current ──► stale
        │
        ▼
normalize payload: v_0 → v_1 → … → v_N
(zeroed workspace, each step validated,
 current marker committed last)
        │
        ▼
migrate every stale account the route touches
(plan → fund → resize → apply → validate → commit,
 one adjacent step at a time)
        │
        ▼
current authorization + business logic run
with current types
        │
        ▼
success: one transaction did everything
failure anywhere: everything rolls back
```

The old client cannot tell that anything happened. This is the property the whole design protects: **backwards compatibility is the default, and the program owns it.**

### 3. Old client → rolled-back program (the honest limit)

Rolling back the _binary_ to a previous executable does not roll back accounts. Accounts written by version N carry `N`; a program compiled when `current == N-1` sees those as **future** versions and rejects them without trial decoding. The supported rollback is redeploying an executable built from the current manifest history (an old _binary_ with the current _ABI_), which keeps every live account readable. This is a deliberate security stance, not an implementation gap: silently guessing at newer layouts would let a rolled-back program misinterpret post-rollback data.

### 4. Where the migration lives: program or client?

**Implemented: the program.** Inline, on-demand, per-account — the transaction that touches a stale account performs its migration before the handler runs, funded by the payer that transaction already declared.

**Implemented: the reserved prefix.** A program wires the reserved `Migrate` instruction (see "The reserved Migrate instruction" above) so a client can prepend `[Migrate { payer }, …real instructions]` when an account is stale and the business instruction declares no payer. Still designed, not built (ADR 0008, tracked in #339): the generated `migrateIfNeeded` helper that fetches accounts, compares the version constant it already embeds, and prepends the migration only when needed. Until that ships, any instruction that can touch a migratable account during a growth step must also declare an optional `migration_payer` slot, and current clients pass it.

## Version exhaustion

Run out of versions and nothing can fix it afterwards. Two facts decide how much this matters.

**Versions are counted per contract, not per program.** Every account, instruction, and event owns an independent history that starts at `0`, keyed by its own discriminator in `migrations/manifest.json`. A program can hold one account at version `3` and another still at `0`; they do not share a counter, and exhausting one says nothing about the rest. So the budget is "255 versions of _this one contract_", not "255 versions of the program".

**The width is program-wide and freezes at the first publication.** `[migrations].version_type` chooses one width for every contract, and once a version appears in a publication receipt it cannot be changed: `make` and `check` both fail with `VersionTypeChanged`, and receipts pin the manifest hash. Before the first release the width is still yours to choose — delete the `migrations/` directory and re-run `make` with the wider setting to re-baseline. After the first release there is no widening path.

That combination makes `u8` the right default. 255 versions of a single account type is not a realistic lifetime for a program that migrates sensibly, and it costs one byte per enveloped account; `u16` costs two and is worth choosing up front only if you expect a single contract to exceed 255 revisions.

`pina migrations status` reports the remaining budget per contract so drift toward the ceiling is visible:

```text
account State v3 (published, 252 version(s) remaining)
```

### When a contract does reach its ceiling

`pina migrations make` fails closed with `VersionExhausted` rather than wrapping. There is no silent reuse of version numbers, and the on-chain side rejects out-of-range versions via `try_from_u32` instead of truncating, so a wrapped version can never be written or misread.

The remedy is a **successor contract**, not a larger counter:

1. Declare a new discriminator variant for the successor account and give it a fresh version-0 history. It is a new contract identity, so it gets its own full budget.
2. Add a bridge instruction that loads the exhausted account through the current loaders and writes the successor account, then closes or drains the old one.
3. Migrate accounts lazily: the bridge runs once per account, driven by a client sweep, the same way the reserved `Migrate` route works.

Old history cannot be pruned to make room. A program cannot enumerate its own accounts — Solana offers no such primitive — so retiring history requires an external proof that every account of that type has been converted. Plan the successor before the ceiling, because the bridge needs the old layout to still be readable by the binary you ship.

## Seeing it live

The whole of this page runs for real in the ten-deployment walkthrough:

```bash
pnpm walkthrough:migrations            # full run, ~3 minutes
pnpm walkthrough:migrations -- --from-step 6 --to-step 8 --keep
```

It scaffolds a program, evolves it through ten published generations (add field, rename through the question flow, type change via a manual transition, compact growth, payload growth, appended optional accounts, event growth, acknowledged removal, full-ladder replay), deploys each one to an isolated Surfpool network through `pina deploy`, regenerates every client each time, and proves that step-1 clients keep submitting successfully against the step-9 program with their data preserved.
