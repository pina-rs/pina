# ADR 0008: Migration ergonomics, client-driven migration, and legacy adoption

- Status: Proposed
- Date: 2026-09-10
- Owners: Pina maintainers
- Related: [ADR 0007](./0007-first-class-versioned-abi.md)

## Context

ADR 0007 established the versioned ABI, on-demand account migration, and the publication ledger. Dogfooding the full developer loop while advancing the migrations example to a second generation confirmed the core experience:

- opting in is one annotation and one `pina.toml` setting;
- forgetting `pina migrations make` fails the build with the exact remedy;
- drafts are replaceable, published history is pinned, and `deploy` records publication automatically;
- inline migration works with an explicitly capped payer.

Three gaps remain between that experience and the intended mental model — "turn migrations on, then stop thinking about them":

1. **A stale account whose business instruction carries no authorized payer cannot migrate.** Every instruction that touches a migratable account must thread a `migration_payer` slot through its process contract, or the instruction fails with `MigrationRequired`. ADR 0007 describes a stable migration instruction that clients invoke before the real operation; that instruction was never implemented.
2. **Clients have no migrate-first flow.** Generated clients know each account's current version (the IDL already carries it as an omitted constant) but never act on it.
3. **Programs launched before migrations cannot adopt them.** ADR 0007 explicitly refuses to interpret unversioned bytes as version zero, so an existing program that adds `migrations` strands every live account.

## Decision

### Reserved migration instruction

Pina reserves one instruction discriminator per program — the all-ones value of the program's instruction discriminator width — for a framework-generated `Migrate` instruction. The `#[discriminator]` macro rejects a user variant that claims the reserved value. Because no Pina program is live today, the reservation is not a breaking change; it is part of the migrations feature contract from its first release.

When a program declares at least one migratable account, the framework generates:

```rust,ignore
pub mod __pina_migrate {
	// data: [reserved discriminator] (no payload)
	// accounts:
	//   [0] payer — writable signer, optional when no step needs funding
	//   [1..] program-owned accounts to migrate, each self-describing
	//         through its account discriminator
	pub fn process(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult;
}
```

The handler dispatches each account by its leading discriminator to the matching `MigratableAccount` implementation and runs the same executor the inline path uses, with the same step, growth, and lamport caps. Accounts whose discriminator matches no migratable contract, accounts not owned by the program, and duplicated mutable accounts fail closed. The developer wires one match arm in the program entrypoint (`Migrate => __pina_migrate::process(...)`) so instruction routing stays explicit, and `pina migrations make` fails the build until that arm exists.

This is the instruction clients prepend when an account is stale: the payer authorizes exactly the migration cost, then the business instruction runs without migration plumbing in its own account list. Business instructions keep their inline-migration behavior; the dedicated instruction is the escape hatch for payers that the historical request could not name.

### Client migrate-first flow

Generated clients gain a `migrateIfNeeded` helper per program:

1. fetch the program-owned accounts named by the operation;
2. compare each account's version envelope against the client's frozen current version (already embedded in the IDL as an omitted constant);
3. build a transaction of `[Migrate { payer }, ...real instructions]` when at least one account is stale, otherwise send the real instructions alone.

The helper never migrates silently at rest: migration happens inside a transaction the caller signs, with the payer the caller chose, and the real operation follows in the same transaction so it observes current data or the whole transaction fails.

### Legacy adoption

A program that launched without migrations adopts them with `#[account(discriminator = ..., migrations, legacy)]`:

- The first `pina migrations make` records a `legacyBase` schema — the pre-migration wire layout without the version envelope — alongside version zero, whose payload is the legacy payload unchanged.
- The generated planner recognizes legacy bytes before any versioned layout: exact-shape validation of the legacy base runs last, after every versioned current and historical layout has had its exact chance. Two layouts accepting the same bytes is an ambiguity error at generation time, so runtime detection stays deterministic.
- The legacy-to-v0 transition is generated: insert the version envelope (version zero) and shift the payload; growth follows the ordinary rent and payer rules.
- Instructions and events adopt the same way. Unversioned historical instruction data is shorter by the version width, which the existing exact-length historical replay already distinguishes.

Existing accounts migrate on their next touch with no client change; a client that keeps sending legacy instruction data keeps working until the program retires that history explicitly.

## Security consequences

- The reserved discriminator removes a value from every program's instruction space; the discriminator macro enforces the reservation at compile time.
- `Migrate` is a privileged-sized surface: it mutates only program-owned accounts, caps lamports per invocation, and refuses accounts that fail ownership or discriminator checks before any mutation.
- Client `migrateIfNeeded` decisions are advisory; the on-chain boundary re-validates every version and shape, so a stale or malicious client cannot downgrade data by skipping migration.
- Legacy detection must be unambiguous by construction: generation fails on any byte-shape overlap between the legacy base and a versioned layout.

## Open questions

- Should `Migrate` accept a maximum-lamports argument, or is the generated per-program cap sufficient? (Current inline path uses a compile-time cap.)
- Should `migrateIfNeeded` batch multiple stale accounts into one `Migrate` per transaction, or one per account to bound per-transaction rent exposure?
- Does legacy adoption need an off-chain bulk-migration command (`pina migrations sweep`) for programs that want to pre-migrate instead of migrating on touch?

## Implementation order

1. Reserved discriminator enforcement in `#[discriminator]` and the `Migrate` handler generated for migratable programs.
2. `migrateIfNeeded` in the Rust client, then TypeScript and Dart.
3. `legacy` adoption: manifest `legacyBase`, planner detection, generated legacy-to-v0 transition, and an adopted-after-launch example test.
