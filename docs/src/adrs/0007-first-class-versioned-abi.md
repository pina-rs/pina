# ADR 0007: Make ABI migrations first-class

- Status: Proposed
- Date: 2026-09-09
- Owners: Pina maintainers
- Related: [ADR 0001](./0001-discriminator-first-layout.md), [ADR 0002](./0002-zero-copy-account-model.md), [ADR 0003](./0003-guard-backed-typed-account-loaders.md), [ADR 0004](./0004-no-std-and-no-allocator-boundary.md)

## Context

Solana programs cannot rewrite every program-owned account during deployment. Account data is loaded only when a transaction names its address, and every transaction has account, privilege, compute, size-growth, and fee constraints. A schema upgrade must therefore remain able to read old bytes and migrate each account on demand.

Account bytes are only one part of the compatibility contract. An old client also sends an old instruction payload and an old positional account list. Events written by earlier executables remain in immutable transaction logs. Treating these surfaces separately can produce combinations that were never valid, such as a new payload paired with an old account list.

The desired developer model is declarative:

```toml
[migrations]
version-type = "u8"
```

```rust
#[account(discriminator = AccountType::Profile, migrations)]
pub struct Profile {
	pub authority: Address,
	pub score: u64,
}
```

The developer declares that a contract is migratable, but does not choose or maintain its version. The current Rust source describes the desired state. Pina owns version allocation, ABI snapshots, generated structural transitions, drift checks, and historical dispatch. A developer supplies code only when a schema diff cannot determine the intended value.

## Decision

Pina will treat migrations as a versioned program ABI system. It will use one generated compatibility boundary at instruction dispatch, backed by a checked-in ABI history and a content-addressed publication history.

### Version envelope

Every opted-in wire contract uses this envelope:

```text
[existing discriminator][schema version][payload]
```

The discriminator remains at offset zero. The version is little-endian and is hidden from generated accessors and patches. Version zero is the first migratable representation.

`[migrations].version-type` is global for the program. Pina initially accepts `u8`, `u16`, and `u32`. It does not accept per-account or per-instruction overrides. The width may change while the program has no published migration-aware release. The first persistent publication freezes the width, byte order, and header offset for that program identity.

Each account, request contract, and event advances independently. A request contract combines its instruction payload and positional account list under one version. This prevents Pina from adapting two historical halves that were never released together.

Existing unversioned data is not silently treated as version zero. Its first payload bytes may be a valid version by accident. Adoption requires an explicit legacy bridge or a new discriminator.

### Current IDL and ABI history

The public IDL describes only the current program contract and includes the current generated version for opted-in types. It is not the migration database.

The checked-in Pina ABI history records the physical information needed to reconstruct every supported representation:

- ABI format and generator versions;
- contract kind and stable identity;
- discriminator bytes and width;
- migration version width;
- fixed or compact storage mode;
- ordered canonical wire types;
- fixed offsets and sizes;
- compact prefix widths, capacities, header offsets, tail order, and alignment;
- referenced enum representations and explicit discriminants;
- instruction accounts, positions, signer and writable requirements, known addresses, and PDAs;
- canonical schema and transition hashes.

Stable identity derives from contract kind and discriminator, not a Rust type name. Renaming a Rust type does not create a new on-chain identity.

The ABI history must come from the same closed schema grammar used by Pina's macros. The Codama IDL and unconstrained Rust type strings are not precise enough to be the long-term physical-layout authority. PinaPod may expose a stable physical-layout descriptor and fingerprint, but it does not own Solana migration policy.

### Drafts and publication

Local iteration has one replaceable draft head per changed contract. `pina migrations create` captures the current ABI. It replaces an unpublished draft without consuming another version. If the current head has appeared in a persistent release, it allocates the next version.

`pina build` never creates or changes migration history. It fails on ABI drift, an unresolved custom transition, a modified published schema or transition, version exhaustion, or a version-width mismatch. A successful build seals the executable hash to an ABI manifest hash.

Developers do not mark versions as published. `pina deploy` records publication after it proves that the deployed executable and runtime ABI manifest match the sealed artifact. Publication records bind:

- cluster genesis hash and program identity;
- deployed executable hash and slot;
- complete ABI-history root;
- every current schema and transition hash;
- generator and ABI format versions;
- version width;
- the preceding publication receipt.

The deployed executable is the source of truth for what is live now. The publication ledger is the source of truth for what has ever been live, including releases later replaced or rolled back. A checked-in lock file mirrors verified receipts for offline builds, but is not an editable substitute for them.

An interrupted or ambiguous deployment freezes its candidate versions until recovery proves that the executable never became live. Loopback localnet deployments are ephemeral by default. Devnet, testnet, mainnet, and remote custom clusters count as publications.

A production-grade append-only registry is the eventual authority. A mutable metadata account may bootstrap the workflow, but Pina must describe its weaker tamper guarantees accurately.

### Generated compatibility boundary

The generated dispatcher, not an ordinary account loader, owns compatibility. Its conceptual flow is:

1. Read the instruction discriminator and request version.
2. Decode the exact historical payload and account-list contract.
3. Validate account count, positions, signer and writable privileges, known addresses, PDAs, and duplicate mutable aliases.
4. Map historical positions to stable logical account names.
5. Inspect all typed migratable accounts required by the route.
6. Preflight every migration without retaining account-data borrows.
7. Apply migrations in generated deterministic order.
8. Reload and validate every account in its current representation.
9. Adapt the historical request to the current command.
10. Apply current authorization and business invariants.
11. Run the current handler.

Application handlers see current types only. Historical instruction versions are attacker-selected input, so an adapter must not preserve obsolete weak authorization. Old bytes are decoded into a current command and then pass current checks.

The current-version hot path reads one version value and compares it with a generated constant. It does not scan history or trial-decode layouts.

### Account migration runtime

Generated account migrations use two phases:

```rust
pub trait MigratableAccount: private::Sealed {
	type Plan;

	const CURRENT_VERSION: u32;
	const VERSION_BYTES: usize;

	fn plan(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError>;

	fn apply(plan: Self::Plan, destination: &mut [u8]);

	fn validate_current(data: &[u8]) -> ProgramResult;
}
```

Planning validates the exact source schema and returns owned, detached state plus the final target length. The plan cannot borrow account data. This lets the dispatcher drop every old borrow before funding, resize, mutation, or CPI. Applying a preflighted plan is infallible; the executor writes the new version only after the destination representation validates.

Generated structural changes include exact field copies, safe reordering, supported widening, and defaults whose value is unambiguous. Renames, narrowing, semantic splits or merges, authority changes, and other ambiguous changes generate a typed unresolved transition. The build remains blocked until the developer implements it and provides semantic fixtures or invariants.

The default migration path retains every surplus lamport. It never uses the ordinary reallocation helper's shrink-refund policy. Growth may charge only an explicitly declared, writable signer or a fixed program treasury policy, subject to a generated maximum. The target and payer must not alias.

### Inline and dedicated execution

An ordinary instruction either completes all required migrations and its business handler atomically, or returns an error. It never returns success after migration without running the requested operation. Returning an error rolls migration writes back with the transaction.

Inline migration is unavailable when any required condition is missing:

- a stale account is readonly;
- growth needs lamports and no authorized payer was supplied by the historical request;
- growth exceeds the runtime's per-instruction limit;
- the bounded chain exceeds the supported compute or stack budget;
- a custom transition needs an account absent from the historical request;
- the current operation requires a new account or stronger privilege.

A stable migration instruction can advance a bounded amount of work and return success. Generated migration-aware clients may invoke it repeatedly, then retry the original operation. An old client cannot acquire this retry behavior after release. For that reason, Pina classifies compatibility at migration creation time instead of promising that every old request remains transparent.

### Instruction account-list compatibility

Pina can normally adapt account reordering, removal of an unused historical account, addition of an optional account, reduced privilege requirements, and a deliberately retained legacy external program route.

Pina cannot invent a missing required account, signer, writable privilege, PDA, or replacement external program. A change from the SPL Token program to Token-2022 is not an automatic migration if the historical request did not supply the new program and accounts. The migration must retain a safe historical route, explicitly retire the old request, or use a new instruction discriminator.

### CPI and mixed versions

Each program migrates only accounts it owns. Before a CPI, the caller migrates its own stale accounts and releases all typed guards. The callee's generated boundary migrates its own stale accounts. After a CPI that received writable accounts, the caller reloads those accounts before further use because the callee may have changed their data and length.

For a current account interacting with a stale account, the dispatcher migrates the stale account before it constructs either current typed view. The handler therefore observes one coherent current model.

Account-local structural migrations cannot read or mutate unrelated accounts. A custom migration may declare read-only dependencies that are included in the historical request. Pina orders an acyclic dependency graph statically. Cycles and cross-account write invariants require an explicit coordinated migration instruction.

### Events

Events are not migrated on-chain. New code emits only the current event representation. Historical decoders remain in tooling and generated clients. A latest-shape projection preserves provenance and distinguishes a field that was absent historically from a field that was emitted with a default value.

### Compatibility verification

`pina test --compatibility` and `pina_test` consume checked-in historical fixtures, not fixtures regenerated from current code. The required suite covers:

- every historical account version to the current version;
- fixed, compact, fixed-to-compact, growth, shrink, and unchanged-size transitions;
- current no-op, future-version rejection, truncation, malformed lengths, and maximum capacities;
- rent deficit, surplus retention, unauthorized funding, aliasing, and arithmetic overflow;
- historical instruction payload and account-list replay against the latest SBF artifact;
- mixed-version sets, duplicate aliases, missing privileges, and account reordering;
- old caller programs performing CPI into the latest callee;
- reload behavior after writable CPI resize;
- rollback after failures injected at each migration phase;
- event golden-byte decoding;
- tampered histories, transitions, manifests, widths, and rollback releases;
- compute, stack, and binary-size budgets for the oldest supported maximum-size state.

Native property tests cover parsers and state transitions. Miri covers borrow-guard lifetimes. Kani covers header arithmetic, bounds, and state machines. Mollusk and Surfpool exercise real SBF resize, rent, CPI, rollback, and old-client behavior.

## Security consequences

Migration expands the program's input surface to every supported historical layout. Generated code must reject unknown and future versions without fallback, validate the exact historical layout before reading fields, use checked length arithmetic, and validate the complete current representation after writing it.

Published transition code is immutable along with its source and destination schemas. Editing a live transition could let two accounts reach the same destination version under different semantics. A defect is repaired with a new version.

Account decoder history usually cannot be pruned safely because a Solana program cannot enumerate all of its accounts and prove that no old version remains. Instruction versions may be explicitly retired. Retiring account history requires an external completeness proof or an acknowledged risk of stranding old state.

An upgrade authority can bypass Pina and deploy arbitrary code. Publication guarantees therefore end at the program's upgrade governance boundary.

## Consequences

- Developers manage desired schemas and semantic exceptions, not version integers.
- Version bytes add permanent storage and request overhead to opted-in contracts.
- Historical decoders and transitions increase SBF size and compute cost.
- The runtime stays `no_std` and allocator-free.
- PinaPod remains responsible for byte-layout validity; Pina owns versioning, rent, dispatch, publication, and compatibility policy.
- Some changes are provably incompatible with old transactions. Pina reports these changes instead of hiding them behind a generated `V2` endpoint or unsafe fallback.

## Rejected alternatives

### Version fields written by developers

This duplicates state already known by the migration history and permits source constants to drift from deployed bytes.

### Per-contract version widths

This saves at most a few bytes while making generic inspection, generated dispatch, configuration, and publication harder to reason about. A program-global width is a simpler permanent contract.

### Loader-only migration

A loader cannot adapt historical instruction payloads or positional account lists. It may also retain a borrow when resize or CPI needs exclusive access.

### Client-only migration

This does not support old clients or on-chain CPI callers and lets a program receive stale accounts without a safe transition path.

### Migration policy in PinaPod

PinaPod should describe and validate physical representations. Solana ownership, privileges, rent, deployment receipts, and instruction dispatch belong in Pina.

### Automatic downgrade

Downgrade expands the attack surface and can discard information. Rollback must deploy an executable that still understands the latest published ABI rather than rewriting accounts to an older schema.
