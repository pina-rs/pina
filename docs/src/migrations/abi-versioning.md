# ABI document versioning

Pina's ABI document is the checked-in `migrations/manifest.json` and the publication ledger at `migrations/publications.json`. Both carry an `abiVersion` that belongs to Pina itself and has nothing to do with any user contract's on-chain version.

This page covers how that version is chosen, how older documents are normalized, what the document stores versus derives, and how its JSON Schemas are generated and published. For the runtime decision tree that governs _on-chain_ migrations, read [How ABI migrations flow](./flow.md). For the decision record behind this design, read [ADR 0009](../adrs/0009-abi-document-versioning.md).

## Three version axes

A Pina program carries three unrelated kinds of version, and confusing them is the most common source of migration bugs.

| Axis                       | Example value                 | Who owns it                   | Changes when                                     |
| -------------------------- | ----------------------------- | ----------------------------- | ------------------------------------------------ |
| On-chain contract version  | `3` (integer in the envelope) | Pina, allocated by `make`     | an account, instruction, or event schema changes |
| ABI document `abiVersion`  | `"0.20"`                      | `pina_abi`'s own release line | a breaking `pina_abi` release — and nothing else |
| `pina_abi` package version | `0.20.3`                      | the release planner           | any `pina_abi` release, including patches        |

Only the first one is written into account bytes. An ABI document upgrade never consumes an on-chain migration version, and an on-chain migration never changes `abiVersion`.

`pina_abi` is not part of the `core` release group. It releases on its own single-member group, so a CLI fix or a renderer tweak in core moves nothing here, while any `pina_abi` release cascades into a core release because `pina_cli` and `pina_macros` consume it. The contract moves deliberately and consumers follow; core releases never move the contract.

## The version value

`abiVersion` is a committed value, one line of text in `crates/pina_abi/ABI_VERSION`, embedded with `include_str!`:

```rust,ignore
pub const ABI_VERSION: &str = include_str!("../ABI_VERSION").trim_ascii_end();
```

The file is pinned to `pina_abi`'s `major.minor` and is allowed to **lead** the crate. A shape-changing pull request advances the file while `Cargo.toml` still reads the old version — the pre-release window — and the release planner's bump catches the crate up. This is the monochange model: a committed `SCHEMA_VERSION` that can sit ahead of its crate (verified at `0.7` against a crate at `0.6.3`), which a compile-time derivation of `CARGO_PKG_VERSION` cannot express. monochange tried derivation twice and reverted both times; [ADR 0009](../adrs/0009-abi-document-versioning.md) carries the history and the reasoning.

### Pre-1.0 bump semantics

While the major is `0`, the release planner shifts bump severity: a `breaking` changeset advances the minor, a `feat` or `fix` advances only the patch. Since `abiVersion` keeps `major.minor`, only a `breaking` changeset moves it:

| changeset on `pina_abi` | crate version       | `abiVersion` |
| ----------------------- | ------------------- | ------------ |
| `breaking`              | `0.20.0` → `0.21.0` | `"0.21"`     |
| `feat`                  | `0.20.0` → `0.20.1` | `"0.20"`     |
| `fix`                   | `0.20.0` → `0.20.1` | `"0.20"`     |

The version moves if and only if the contract moved. This property is why `pina_abi` stays pre-1.0: at `1.0.0` the shift switches off, a `feat` would advance `major.minor` with no wire change, and `1.0.0` is not reachable through the planner anyway. Every `0.N` version also keeps a permanent schema URL under the publishing policy, and every ABI version is a real contract worth one.

## Reading a document

Every reader goes through the decode path rather than deserializing straight into the typed model:

1. parse the JSON value;
2. read `abiVersion` and reject a document stamped above the reader's own version, naming the supported version in the error;
3. apply each adjacent converter in order, from the document's version up to the current one;
4. deserialize into the current typed model;
5. validate every content-addressed invariant, re-deriving everything the document no longer stores.

Step 2 is a capability marker, not a compatibility promise, and behaves like `Cargo.lock`'s `version` field: a repository's documents are read by the same Pina the repository builds with, so the rejection only reaches a human who downgraded, and the remedy is one line — upgrade.

Conversions run in memory only. No command rewrites a checked-in document as a side effect of reading it.

## The converter contract

The reader carries an ordered table of adjacent converters — one edge per ABI version step — keyed by the version itself. There is no separate epoch table: the version advances exactly at contract changes, so it is the only axis.

```rust,ignore
fn upgrade_document(version: &str, value: serde_json::Value) -> Result<serde_json::Value, String> {
	match version {
		"0.20" => migrate_0_20_to_0_21(value),
		"0.21" => migrate_0_21_to_0_22(value),
		_ => Err(format!("no Pina ABI migration is available from {version}")),
	}
}
```

Four rules make the chain trustworthy.

**Every step has an edge, including no-ops.** The version follows every breaking `pina_abi` release, but the document shape changes only some of those times. A breaking change with an unchanged document registers a validating no-op edge that decodes, validates, and returns the document unchanged. monochange's `v0.6 → v0.7` edge is the precedent — payloads unchanged, version advanced because a config contract changed — and the edge exists so the `0.6` contract stays frozen while the walk stays gapless.

**Forward only.** Writers always emit the current version, and a document that must reach an older tool is regenerated from source by that tool. A downgrade entry point does not exist, so no best-effort write can lose information.

**Never deleted.** Once an edge ships, it is part of published history. Fixing a defective converter means a new version, never editing the old one — the same immutability rule the on-chain migration system applies to its transitions.

**Gapless by construction and by test.** The table is an ordered walk over adjacent versions, and a test decodes every frozen fixture to the current model, so a missing or mis-wired edge is a CI failure rather than a runtime error on a user's machine.

## Stored versus derived

The 0.20 baseline stores facts and re-derives everything else at load. What the document used to carry, and where it comes from now:

| Removed field                      | Where it comes from now                                           |
| ---------------------------------- | ----------------------------------------------------------------- |
| `DataSchema.physical`              | derived from `layout` and `fields` by the frozen grammar          |
| `SchemaVersion.version`            | the version's position in the `versions` array                    |
| `SchemaVersion.schemaSha256`       | computed from the decoded schema                                  |
| `SchemaVersion.processSha256`      | computed from the decoded process                                 |
| `Transition.from` / `to`           | adjacency — the transition sits on version `index` from `index-1` |
| `Transition` schema/process hashes | the neighbouring versions' computed hashes                        |
| `Transition.process` proof         | re-derived by `classify_process_transition`                       |
| `ProcessAccount.constraints`       | not recorded — validation rules live in the IDL clients use       |
| receipt `cluster`                  | not recorded — `rpc_url` keeps the credential-free endpoint       |

What stays is what cannot be derived: `codec` (the semantic pin for layout derivation), `rust_name` (macro lookup), the `auto` policy, identity validation including the path-traversal proofs, `mode`, `renames`, `implementation_sha256` (the hash of an external transition file), and the receipt hash chain with its pending and `abandoned` records. Publication receipts pin schema hashes by computing them from the decoded manifest at pin time.

Because `deny_unknown_fields` applies, any field change is a breaking change for older readers, and every document change therefore advances the ABI version through a `breaking` changeset. The converse does not hold — a breaking crate change with an unchanged document is the no-op edge above.

## The frozen fixture matrix

```text
crates/pina_abi/fixtures/0.20/manifest.json
crates/pina_abi/fixtures/0.20/publications.json
crates/pina_abi/fixtures/0.20/manifest.schema.json
crates/pina_abi/fixtures/0.20/publications.schema.json
crates/pina_abi/fixtures/current/…
```

- A `<version>/` directory is frozen when that release ships and is never regenerated. They are the only artifacts that record what an older release actually wrote; a fixture regenerated from current code would encode today's shape under yesterday's version and prove nothing.
- `current/` is regenerated by the schema and fixture task whenever the model changes.
- Fixtures are generated deterministically from fixed seeds, the way the monochange schema assets are, so the frozen bytes are reproducible on every machine rather than hand-maintained.
- The fixtures prove the shape changed; the committed value only names it. The **fixture-drift guard** pairs the two: regenerating `current/` must reproduce the frozen bytes unless `ABI_VERSION` advanced. A serialization change without a version change fails CI; a version change with unchanged bytes is the deliberate no-op edge, which registers a new frozen directory with identical bytes.
- A test decodes every frozen fixture through the current reader and asserts it reaches the current model — the **gapless-walk guard**.
- The **not-lag guard** asserts the committed value never lags the crate's `major.minor`, and the **ahead-requires-changeset guard** asserts that a value ahead of the crate — the pre-release window — has an active `pina_abi` changeset behind it. Any other ahead state fails.

## JSON Schemas

The document types derive `schemars::JsonSchema`, so the schema is generated from the same types that serialize and deserialize the document — `deny_unknown_fields` becomes `additionalProperties: false`, and the schema can never drift from the code that enforces it.

- Canonical artifacts are checked in at `crates/pina_abi/schemas/manifest.schema.json` and `publications.schema.json`, regenerated and drift-checked in CI the way Codama IDL fixtures are.
- Each version's schema is frozen inside its fixture directory, so the schema for any historical shape stays printable.
- Every version's schema is hosted at a permanent URL under this book: `https://pina-rs.github.io/pina/abi/schemas/<version>/manifest.schema.json`, with `$id` set to that URL. Every `0.N` version keeps its URL permanently.
- `pina abi schema [--document manifest|publications] [--version <major.minor>]` prints the JSON with stable output, so editors, CI jobs, and non-Rust toolchains can validate a document without trusting the bytes:

```sh
pina abi schema --document manifest > manifest.schema.json
```

## Upgrading from pre-0.20 integer formats

The integer `formatVersion` era is retired, not migrated. After upgrading Pina:

1. run `pina migrations sync` (or `pina migrations make`) in the program directory — every checked-in draft is regenerated at the current `abiVersion`;
2. commit the rewritten `manifest.json` and `publications.json`.

A ledger that pinned a real deployment would have no upgrade path from the integer formats. None exists today — the only non-empty receipt in any example records the `fixture` cluster — and accepting that one-time stranding is recorded in [ADR 0009](../adrs/0009-abi-document-versioning.md).

## What this does not cover

This mechanism governs the **document**. It is not the on-chain migration system:

- it does not read or write account bytes;
- it does not allocate or consume a contract version;
- a broken converter cannot corrupt an account, because no account data passes through it.

The publication ledger records a manifest digest, and that digest sits inside each receipt's own hash, so it is covered by the chain. It is not recomputed and compared, though: a receipt's `manifestSha256` is written once at publication and thereafter only carried forward, and validation checks that it is a well-formed digest rather than that it still matches the checked-in manifest. Whether it should become a live assertion is an open question in [ADR 0009](../adrs/0009-abi-document-versioning.md).

The on-chain rules — envelopes, adjacent transitions, rent, the publication ledger, and the four scenarios — are in [How ABI migrations flow](./flow.md).

## Checklist for an ABI-affecting release

1. Change the document shape, or the `pina_abi` contract. `deny_unknown_fields` means every document change is breaking; there are no additive-only changes.
2. Advance `crates/pina_abi/ABI_VERSION` to the next minor. The value leads the crate until the release bump catches up.
3. Add the adjacent converter edge — a real one when the bytes changed, a validating no-op when they did not.
4. Add the `breaking` changeset on `pina_abi`. The ahead-requires-changeset guard fails without it.
5. Regenerate the canonical schemas and the `current/` fixtures, freeze the outgoing version's fixture directory, and copy the new version's schemas into the hosted location.
6. Run `pina migrations sync` so every checked-in example is rewritten at the current version, and confirm the `current/` fixtures changed — or, for a no-op edge, did not.
7. Run `pina migrations check` and the `pina_abi` suite. The not-lag, ahead-requires-changeset, fixture-drift, and gapless-walk tests are the four gates that must pass.
