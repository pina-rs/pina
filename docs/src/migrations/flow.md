# How a migration flows

This page is the map of the whole migration system: what happens when a developer changes a contract, what the program does at runtime, and exactly what is expected from clients. Everything described here is the implemented behavior — [ADR 0008](../adrs/0008-migration-ux-and-legacy-adoption.md) collects the parts that are designed but not built yet.

An interactive version of this page is at [flow-interactive.html](./flow-interactive.html).

## The one-paragraph answer

**Migration happens on-chain, inside the program, on demand.** A client never migrates account data and never needs to know a migration exists. Every wire carrier — account, instruction payload, event — carries a version envelope right after its discriminator, every client writes the version it was generated with, and the program normalizes whatever arrives into its current representation inside the transaction that touched it. If the transaction fails, Solana rolls the migration back with everything else.

## Development-time flow

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
assume-removed = []
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
- the transfer is capped by the `max_lamports` budget the program passes to the executor. An undersized budget fails the whole transaction with `MigrationBudgetExceeded` — nothing is half-migrated — but the account also stays stale until the budget is raised, so size it deliberately.

A planning figure for that budget: rent exemption costs about **6,960 lamports per byte** (3,480 lamports per byte-year at the two-year exemption threshold), so growing an account by _N_ bytes needs roughly `N × 6,960` lamports of head room on top of what the account already holds. `pina migrations make` prints a warning with the exact growth and estimate whenever a transition grows an account or keeps a compact (capacity-driven) layout.

Two consequences follow from the payer model:

1. **An old client cannot fund growth.** A client generated before the instruction gained its optional `migrationPayer` slot submits without a payer; if the account it touches now needs rent, that transaction fails with `MigrationRequired`. The fix is a current client (or a payer-carrying migration path) — by design, rent is never taken from an account the client did not offer.
2. **The compute bill lands on the touching transaction.** A stale account pays its ladder's compute cost inside whichever transaction finds it, and `MAX_INLINE_STEPS` bounds how long that ladder may be. Accounts that are too expensive to migrate inline fail with `MigrationUnavailable` rather than silently burning the budget; the reserved `Migrate` instruction below is the out-of-band path.

### The reserved Migrate instruction

Pina reserves the all-ones value of every instruction discriminator width (`0xff`, `0xffff`, `0xffff_ffff`, `0xffff_ffff_ffff_ffff`) for a framework migration instruction; `#[discriminator]` rejects user variants that claim it at compile time. A program with migratable accounts wires the reserved route before parsing its own instruction enum:

```rust,ignore
if is_migrate_instruction(data) {
    return process_migrate(program_id, accounts);
}
```

Its account layout is `[payer, systemProgram, accountA, accountB, …]`. Slot 0 is a writable payer funding every rent deficit (or the program address when the invocation needs no funding), slot 1 is the system program the rent transfers invoke, and each later slot is a program-owned, self-describing migratable account. A slot holding the program address (the placeholder generated clients write for an omitted optional account) or an index past the end of the list is skipped, so a client sends only the accounts it needs. `MigrateContext` validates ownership, rejects duplicated account slots, migrates each slot at most once, and runs each slot through the same `MigrateAccount` executor — the same step, growth, and lamport caps as the inline path.

That makes the client flow explicit: when an account is stale and the business instruction cannot carry a payer, prepend `[Migrate { payer }, …real instructions]` in the same transaction — the payer authorizes exactly the migration cost, and the real instruction observes current data or the whole transaction fails.

### Generated client helpers

Generated clients turn that flow into a one-call routine. Next to each migratable account module the TypeScript, Dart, and Rust clients emit:

- a `<Account>MIGRATION_VERSION` constant — the schema version the client was generated from;
- the generic `needsMigration` envelope check is that per-account helper (`stateNeedsMigration` for a `State` account, and so on);
- `<account>NeedsMigration(bytes)` — a cheap envelope check that returns true only when the bytes name this account's discriminator and a version older than the client's schema. Future versions and foreign discriminators return false; the decoder explains those when the account is decoded.

The clients also emit a `Migrate` instruction composer (TypeScript `getMigrateInstruction`, Dart `getMigrateInstruction`, Rust `Migrate::new().instruction()`). Every migratable slot is optional: omitted slots become program-address placeholders and trailing omitted slots are truncated, so a client sends only the accounts it needs. The intended catch → migrate → retry loop:

```ts
const { data } = await fetchEncodedAccount(rpc, address);
if (stateNeedsMigration(data)) {
	await send(getMigrateInstruction({ state: address, payer }).make());
}
// now decode `state` and send the real instruction
```

`migrateIfNeeded` — fetching, checking, and migrating in one call over an RPC handle — is designed in [ADR 0008](../adrs/0008-migration-ux-and-legacy-adoption.md).

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

## Seeing it live

The whole of this page runs for real in the ten-deployment walkthrough:

```bash
pnpm walkthrough:migrations            # full run, ~3 minutes
pnpm walkthrough:migrations -- --from-step 6 --to-step 8 --keep
```

It scaffolds a program, evolves it through ten published generations (add field, rename through the question flow, type change via a manual transition, compact growth, payload growth, appended optional accounts, event growth, acknowledged removal, full-ladder replay), deploys each one to an isolated Surfpool network through `pina deploy`, regenerates every client each time, and proves that step-1 clients keep submitting successfully against the step-9 program with their data preserved.
