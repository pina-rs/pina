# Migrate to automatic migrations

Automatic migrations let a program keep reading the bytes it already wrote. Instead of treating every schema change as a breaking deploy, Pina gives each opted-in account and event a framework-owned version field, snapshots the history into a checked-in manifest, and migrates accounts on demand when a stale one is touched. Instructions are recorded as snapshots that stop a wire-breaking change at build time, and gain a version field only when they opt in.

This guide upgrades a program to that workflow and opts it in wholesale with `[migrations].auto`, so new contracts are versioned without per-declaration annotations. It also lists the compatibility work that the change brings: the envelope is a wire-format change for accounts and events, and the generated clients change shape.

> **Before you start:** this guide assumes the program is new, or that you are prepared to change the wire format of its accounts, instructions, and events deliberately. If the program is already live and its accounts were written _without_ an envelope, read [Adopting on a live program](#adopting-on-a-live-program) before you start.

## What the envelope changes

Every enveloped contract gains one field immediately after its discriminator:

```text
before: [discriminator][payload]
after:  [discriminator][schema version][payload]
```

| Contract kind | Enveloped when                                                   | What the version describes                                |
| ------------- | ---------------------------------------------------------------- | --------------------------------------------------------- |
| Account       | opted in by `migrations` or `auto`                               | The account's stored layout                               |
| Instruction   | only with the `migrations` token; `auto` records a snapshot only | The instruction payload plus its process account contract |
| Event         | opted in by `migrations` or `auto`                               | The emitted log payload                                   |

An instruction covered by `auto` without the token keeps `[discriminator][payload]`. The manifest records one snapshot of it (`"envelope": false`), the build fails when the struct drifts from that snapshot, and after publication its payload is fixed: a new payload needs a new discriminator. [How a migration flows](./flow.md#instructions-a-snapshot-unless-they-opt-in) covers the rules.

The version is little-endian, stored per contract, and hidden from generated accessors, patches, and instruction arguments. Each account, instruction, and event carries its own history starting at version `0`, so a program with one account at version `3` and another still at `0` is normal. Only the width is program-wide: `version_type` in `pina.toml` picks it once for the program: `u8` (255 versions per contract, the default and the recommendation), `u16`, or `u32`. There is deliberately no `u64`; see [Core Concepts](../core-concepts.md).

Because the version is part of the bytes, changing it after publication is a breaking change for every migration-aware contract. That is why the width is chosen once, and why the manifest is checked in.

## Step 1: Upgrade the dependency

```toml
[dependencies]
pina = { version = "0.17", default-features = false, features = ["derive"] }
```

Keep whatever feature set your program already uses. The breaking changes in this release are listed in [What changes for clients](#what-changes-for-clients) and in the [changelog](https://github.com/pina-rs/pina/blob/main/changelog.md).

A program that already has a `migrations/manifest.json` at `abiVersion` `0.20` recorded every instruction with an envelope. Read [Upgrading from ABI 0.20](./abi-versioning.md#upgrading-from-abi-020) before running `create`, so a published instruction keeps its wire format.

## Step 2: Turn it on

Declare the policy in `pina.toml`:

```toml
[migrations]
version_type = "u8"
auto = true # or ["accounts", "events", "instructions"], or a staged subset
```

`auto` accepts `true` (every kind), `false` (the default), or a list of `accounts`, `events`, and `instructions`. Any other name is a configuration error.

Staging is a real option because the kinds cost different amounts. Covering instructions does not change their payload bytes: `auto` records each instruction as a snapshot without an envelope. An instruction you opt in with `#[instruction(discriminator = X, migrations)]` does gain the envelope, which changes its payload bytes and ripples into CPI call sites, so opt instructions in one at a time while your callers catch up.

The policy is _declared here but recorded in the manifest_: the macros read the checked-in `migrations/manifest.json`, never `pina.toml`, because a proc macro does not re-expand when an unrelated toml file changes. That is also why the next step is mandatory rather than optional.

## Step 3: Snapshot the history

```sh
pina migrations create
```

This records the resolved policy in `migrations/manifest.json`, snapshots the current schema of every contract of the listed kinds, and generates an adjacent transition for each change it can prove structurally. Commit the manifest with your source: it is the hash-chained source of truth the build consults.

A declaration the manifest does not record yet fails the build with the `pina migrations create` remedy, so you cannot forget a contract by accident.

Adding `migrations` to a program that is already published is a bulk wire-format change: for each newly enveloped contract, `create` records the envelope as a migration step rather than pretending the bytes already had one. A snapshot-only instruction adds no byte, so it does not count.

## Step 4: Keep the build honest

When a policy is recorded, `create` scaffolds a build script so a policy flip re-expands every contract without a source edit:

```rust
fn main() {
	println!("cargo:rerun-if-changed=migrations/manifest.json");
}
```

The scaffold is idempotent and never overwrites a hand-written build script — when it cannot safely write one, it prints the exact line to add. Run `pina migrations check` in CI: it fails on drift, on incomplete manual transitions, and on a missing rerun directive.

## Step 5: Answer what the diff cannot

Structural changes generate themselves (added, removed, reordered, or reseated fields). Semantic ones do not: a rename that changes meaning, a type narrowing, a split or merge of a field, or an authority change stops the build until you implement the transition and supply fixtures. That is the intended safety property — Pina will not guess what a value should become.

Run `pina migrations create` after every schema change; when it reports an unresolved transition, implement it there. Fixed-account transitions are total and infallible once the exact source shape is validated; manual instruction transitions run in scratch space, so they can reject a value before anything is written. An instruction transition that adds an argument is always manual, because a zero-filled default is indistinguishable from a value the client sent. Events have no transitions: a changed event appends a version with its own schema.

## Step 6: Know the bill before you ship

```sh
pina migrations status
pina migrations status --json
```

The status output now carries the cost preview: per contract, the current size, the pending growth for a day-one account, and the approximate rent deficit at the established ~6,960 lamports per grown byte; per instruction, the worst-case adjacent-step ladder a stale account can trigger with its static compute estimate; and a program-wide summary of the most expensive touching transaction. Sizing `max_lamports` and `MAX_INLINE_STEPS` stops being guesswork.

When a transition grows an account, `create` states the estimated deficit and names the on-chain error a too-small budget produces. Each budget failure is separately diagnosable on-chain, and each error's rustdoc names its remedy:

| Condition                                   | Error                            | Remedy                                                                          |
| ------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------- |
| Workspace smaller than `WORKING_SIZE`       | `MigrationWorkspaceExceeded`     | Supply at least the generated `WORKING_SIZE`; it must stay within 1,024 bytes   |
| A step grows past the runtime realloc limit | `MigrationAccountGrowthExceeded` | Publish intermediate versions; no lamport budget can raise 10,240 bytes         |
| Rent deficit above the lamport budget       | `MigrationLamportBudgetExceeded` | Raise the program's `max_lamports` constant, or pass a larger sweep budget      |
| Ladder longer than the inline step limit    | `MigrationUnavailable`           | Rebalance the history so no live account is more than `MAX_INLINE_STEPS` behind |

## Step 7: Regenerate the clients

Regenerate the IDL and every client after the program surface changes. `pina generate` refreshes the project IDL as part of the same run:

```sh
pina generate --client rust --client typescript --client dart
pina generate --client cpi
pina generate --client cli-rust
```

Generated clients write the current envelope version automatically, so they need no new arguments. Three client-side behaviors are new and worth reviewing in the diff:

- **Event decoders enforce the envelope.** Each earlier version of an event is its own generated event, `<Event>V<n>`, with that version's codec. The program-level log parser (`parse<Program>EventsFromLogs` in TypeScript and Dart) routes each record by discriminator and version and throws on a version no generated event describes; regenerate the client to decode it. Nothing is projected into the current shape. Prefer the generated log entry points over decoding event bytes by hand.
- **Account decoders and the read-only view.** Account loaders still reject a foreign version with an actionable error. When a program only ever _reads_ a migratable account, `try_from_bytes_versioned` returns a `Current` or historical view without migrating; you then handle each representation your history exposes.
- **Migration-aware helpers.** Generated clients carry the per-account `needsMigration` checks and the reserved `Migrate` composer, so a client can send the sweep before the real instruction described in [How a migration flows](./flow.md#the-sweep-instruction).

## Step 8: Roll out

1. Deploy the program. A stale _enveloped_ account is migrated by whichever instruction touches it writably; a read-only touch fails with `MigrationRequired` until one writable touch happens.
2. For read-heavy accounts, send the sweep instruction first — it migrates every listed account that is behind and is cheap to send speculatively. The full pattern, including the payer rules, is in [How a migration flows](./flow.md).
3. Ship the regenerated clients. A client generated from an older release keeps working: it writes its own frozen version, the program migrates the accounts it touches, and an instruction that opts into migrations has its payload normalized before the handler runs.

## Adopting on a live program

This is the case the automated flow deliberately does not decide for you.

Pina refuses to interpret unversioned bytes as version zero, because the first bytes of an old payload could look like a valid version by accident. So a program whose accounts were written before the envelope existed cannot simply switch `auto` on: the existing accounts have no version field, and no generated transition can read a layout nobody recorded. Instructions do not have this problem: `auto` records them as snapshots without an envelope, which match the bytes live clients already send.

You have two honest options:

1. **A new discriminator.** Ship the versioned layout under a fresh account or instruction discriminator. Nothing is silently reinterpreted, and both layouts can coexist while you move accounts over.
2. **A deliberate legacy bridge.** Record the unversioned layout as the baseline and write the transition into the enveloped shape by hand, then migrate accounts through a writable path you control.

The framework ergonomics for the second option are specified in [ADR 0008](../adrs/0008-migration-ux-and-legacy-adoption.md) and are not built yet. Until they are, treat adoption on a live program as a migration project rather than a configuration flip — and confirm the exact account bytes you expect with `pina migrations status` before enabling the policy in production.

## What changes for clients

- The wire format of every enveloped contract gains the version field. Anything that parses raw account data, event records, or the payload of an instruction that opts into migrations without going through the generated client must be updated. A snapshot-only instruction's payload is unchanged.
- Generated event decoders changed shape: decoded events include the envelope's discriminator and migration version, and each historical version is its own `<Event>V<n>` event decoded with its own schema. Decoded records carry no source-version or was-migrated flag, because nothing is converted.
- The on-chain budget error was split into the three distinguishable codes above. The legacy aggregate code keeps its original value so binaries compiled before the split stay decodable, but client error mappings should learn the new codes.

## Verify the result

- `pina migrations check` in CI, to fail on drift or a missing rerun directive.
- `pina test --compatibility` to exercise every historical version of every contract against checked-in fixtures.
- `pina migrations status` to review the cost picture before deploying.

Once those pass and the manifest is committed, the next schema change flows through `create` instead of through a breaking deploy.
