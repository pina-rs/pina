# ADR 0009: Version the ABI document on `pina_abi`'s own release line, reset its shape, and publish generated schemas

- Status: Accepted
- Date: 2026-09-19
- Owners: Pina maintainers
- Related: [ADR 0007](./0007-first-class-versioned-abi.md), [ADR 0008](./0008-migration-ux-and-legacy-adoption.md), [ABI document versioning](../migrations/abi-versioning.md), [Migrate to the reset ABI document](../migrations/abi-document-reset.md)

## Context

ADR 0007 established that Pina's ABI document carries its own `formatVersion`, independent of every on-chain contract version, and that readers normalize historical formats through Pina-owned adjacent migrations before deserializing the current typed model. That machinery exists and works: `MANIFEST_FORMAT_VERSION` is `4`, `PUBLICATION_FORMAT_VERSION` is `3`, and `decode_manifest` walks a document from any older format up to the current one.

Five gaps remain between that implementation and the guarantee a reader actually needs.

1. **The format number is an arbitrary integer with no relationship to a release.** `"formatVersion": 4` does not tell a reader which release wrote the document or which release can still read it, and nothing prevents the counter from advancing for a non-breaking reason. The fix is not to derive a value from a version `pina_abi` does not control: today `pina_abi` carries `version = { workspace = true }` inside the ~25-crate `core` group, so its version moves on every core release — a CLI fix, a renderer tweak — and any value derived from it would advance for reasons unrelated to the document contract. `pina_abi` needs a release line of its own before any version pin is meaningful.
2. **No historical documents are checked in.** Every one of the 24 checked-in example manifests is format 4 and every publication ledger is format 3. The `1 -> 2 -> 3 -> 4` chain is exercised only by unit tests that construct documents in memory; it has never run against bytes an older release actually wrote. Formats 1 and 2 never shipped in any tagged release at all — `pina_abi` was introduced by v0.16.0 already at format 3 — so their readers and the entire downgrade ladder serve documents that provably never existed.
3. **The chain is not proved gapless, and half of it is uncalled.** No test asserts that every format in `1..=4` is covered, and the explicit downgrade entry points (`convert_manifest_format`, `convert_publication_ledger_format`) have no callers outside `pina_abi`'s own tests. Nothing in CI runs `pina migrations check`, `create`, or `sync` against the repository's 48 checked-in documents — the nightly walkthrough drives a scratch copy — so document drift has no gate.
4. **The document stores its own derivations as frozen caches.** `DataSchema.physical` is recomputed from `layout` plus `fields` and rejected when it disagrees — measurements across `migrations_program` show it is 45–75% of every schema snapshot's bytes. Every transition re-records the neighbouring schema and process hashes it is validated against, a `version` field that must equal its vector index, `from`/`to` numbers that must equal adjacency, and a frozen process proof that `validate` re-derives and compares. None of it adds information; all of it adds size and a second copy of every invariant.
5. **No JSON Schema exists for external consumers.** The document is JSON, but nothing can validate it outside Rust: no schema artifact, no published URL, no command that prints one.

No Pina program is live, and ADR 0008 already relies on that when it reserves an instruction discriminator. The claim this ADR needs is narrower than "no published crate": `pina_abi` is published, so a third party could in principle have unpacked a manifest to read it, but no checked-in document describes a deployed program — the single non-empty receipt in the repository records the `fixture` cluster — and every checked-in document is regenerated from source by `pina migrations create`. Nothing outside this repository depends on the integer values or the stored derivations, so retiring both is free today and expensive once a real program pins them.

## Decision

`pina_abi` leaves the `core` group and owns its release line; the ABI document version is a committed value pinned to that line; converters run between every adjacent pair of values, validating as no-ops where the document shape did not change; the baseline shape drops every stored derivation; and a JSON Schema for each version is generated, checked in, and published at a versioned URL.

### `pina_abi` leaves `group.core`

The structural precondition. monochange keeps its schema crates (`monochange_schema`, `monochange_classification`) deliberately outside its grouped release train; pina's topology is currently the opposite. The reset moves `pina_abi` to a single-member group with its own changelog, the `[group.snapshot]` pattern:

- `crates/pina_abi/Cargo.toml` becomes a literal `version = "0.20.0"` instead of `{ workspace = true }`;
- the root workspace dependency pin (`Cargo.toml`'s `pina_abi = { path = "crates/pina_abi", version = "…" }`) gains a `versioned_files` rule so the release planner bumps it in lockstep with the crate;
- `pina_cli` and `pina_macros` consume `pina_abi`, so a `pina_abi` release cascades into a core release. That direction is the point: the contract moves deliberately and consumers follow, while core releases never move the contract.

Starting at `0.20.0` continues the workspace's version neighborhood and gives the reset version a clean name. It must not jump to `1.0.0`; see the bump table below.

### A committed value, not a derived one

Both documents replace their independent integers with one shared string field:

```json
{
	"abiVersion": "0.20",
	"programId": "...",
	"versionType": "u8"
}
```

The value lives in a one-line committed file, `crates/pina_abi/ABI_VERSION`, embedded with `include_str!`:

```rust,ignore
pub const ABI_VERSION: &str = include_str!("../ABI_VERSION").trim_ascii_end();
```

The file is pinned to `pina_abi`'s `major.minor` — and, crucially, it is allowed to **lead** the crate. A shape-changing PR merges while `Cargo.toml` still reads the old version; the release PR bumps the crate afterwards. monochange works exactly this way and verified the state repeatedly (`SCHEMA_VERSION` at `0.7` while the crate sat at `0.6.3`), because a value that cannot lead its crate forces merged source to stamp new shapes with yesterday's version. Derivation was tried there twice — a `build.rs` reading `CARGO_PKG_VERSION`, then `extract_major_minor(env!("CARGO_PKG_VERSION"))` — and reverted both times: once so an already-merged release commit embedding an older schema version stayed readable, and once because the build script was excluded from `package.include` and `cargo publish` failed outright. The crate that still derives a snapshot version records the core objection in its own config: a major changeset would "advance the published command snapshot contract … without any wire change." Derivation advances the contract for reasons unrelated to the contract.

The stamp is written by the tool and read by the tool. An older Pina rejects a document stamped above its own version, failing closed with the supported version named in the error — the same contract as `Cargo.lock`'s `version` field. A repository's documents are read by the same Pina the repository builds with, so the rejection only reaches a human who downgraded, and the remedy is one line: upgrade.

### Pre-1.0 bump semantics: the version moves if and only if the contract moved

The release planner shifts bump severity only while the major is `0` (`breaking`/`major` → minor, `feat` → patch). Combined with a `pina_abi` on its own axis, this gives exactly the wanted property:

| changeset on `pina_abi` | at `0.20.0` | `abiVersion` | at `1.0.0` | `abiVersion` |
| ----------------------- | ----------- | ------------ | ---------- | ------------ |
| `breaking`              | `0.21.0`    | `"0.21"` ✓   | `2.0.0`    | `"2.0"` ✓    |
| `feat`                  | `0.20.1`    | `"0.20"` ✓   | `1.1.0`    | `"1.1"` ✗    |
| `fix`                   | `0.20.1`    | `"0.20"` ✓   | `1.0.1`    | `"1.0"` ✓    |

Pre-1.0, only a `breaking` changeset advances `major.minor`, so the ABI version moves if and only if the `pina_abi` contract moved. At `1.0.0` the shift switches off and a `feat` — say, making `physical_layout` public, no wire change — would advance the ABI version and restamp every document. Jumping to `1.0.0` is therefore rejected; it is not even reachable through the machinery, since pre-1.0 a `major` changeset yields a minor bump, so reaching `1.0.0` would require hand-editing `Cargo.toml` past the planner that enforces all of this. Staying `0.x` also keeps the versioned-URL policy publishing every ABI version permanently (`0.N` always, `N.0`-only after 1.0), and every ABI version is a real contract worth a permanent URL.

### The converter contract: adjacent edges, no-ops included

The reader carries an ordered table of adjacent converters, one per ABI version step, and reading is reject-then-walk: reject a stamp newer than the reader, apply each converter from the document's version to current, deserialize the typed model, validate the invariants. There is no separate epoch table — the version itself is the only axis, because it now advances exactly at contract changes.

Because the version follows every breaking `pina_abi` release but the document shape changes only sometimes, **validating no-op edges are required, not avoidable**. monochange's `v0.6 → v0.7` edge is the precedent: payloads unchanged, version advanced because a config contract changed, and the edge exists so the `0.6` contract stays frozen while the walk stays gapless. A no-op edge validates the document and returns it; that is all.

Edges run forward only and are never deleted. Writers always emit the current version, and a document that must reach an older tool is regenerated from source by that tool.

### The guards, ported from monochange

1. **Not-lag guard.** The committed value must not lag the crate's `major.minor`. After a release the two are equal.
2. **Ahead-requires-changeset guard.** A value ahead of the crate is the legitimate pre-release window — and must have an active `pina_abi` changeset behind it so the release actually catches the crate up. Any other ahead state fails.
3. **Fixture-drift guard.** Regenerating the canonical `current/` fixture must reproduce the frozen bytes unless the committed value advanced. The fixtures prove the shape changed; the committed value only names it — so a serialization change without a value change fails CI, while a value change with unchanged bytes is exactly the no-op-edge case, registered deliberately.
4. **Gapless-walk guard.** Every frozen fixture decodes through `decode_manifest` or `decode_publication_ledger` to the current model, so a missing or mis-wired edge is a test failure, never a user's runtime error.

### The lean baseline shape

The 0.20 baseline stores facts and derives everything else at load:

- `DataSchema` keeps `{layout, fields, codec}` and drops the stored `physical`; the existing derivation becomes the public source for offsets, sizes, and capacities, and `codec` remains the semantic pin so a future layout change is a loud version event, not a silent reinterpretation.
- `SchemaVersion` drops `version` (it is the vector index), `schema_sha256`, and `process_sha256` (they are computed at load; publication receipts compute their pins from the decoded content).
- `Transition` keeps `{mode, renames, implementation_sha256}` and drops `from`/`to` (adjacency), the four neighbouring schema and process hashes, and the frozen `ProcessTransition` proof (re-derived by `classify_process_transition`).
- `ProcessAccount` drops `constraints`. The migration ABI records wire facts — name, writable, signer, optional, default address, PDA identity — because those decide whether old bytes and old account lists still parse. Declarative validation rules are program semantics, live in the IDL clients generate from, and previously made a validation-only change demand a new instruction discriminator.
- Receipts drop the `cluster` label and keep the credential-free `rpc_url`.
- `rust_name`, the `auto` policy, identity validation (including the path-traversal proofs), the receipt hash chain, the pending record, and the `abandoned` flag all stay.

Because `deny_unknown_fields` applies, any document field change is breaking for older readers, and every document change therefore advances the ABI version through a `breaking` changeset. The converse does not hold — a breaking crate change with an unchanged document is the no-op edge.

### Generated, published JSON Schemas

The document types derive `schemars::JsonSchema`, so `deny_unknown_fields` becomes `additionalProperties: false` and the schema is generated, never hand-written:

- canonical artifacts checked in at `crates/pina_abi/schemas/{manifest,publications}.schema.json`, regenerated and drift-checked the way Codama IDL fixtures are;
- one frozen copy per version inside the fixture matrix, so a historical shape's schema stays printable forever;
- `pina abi schema [--document manifest|publications] [--version <major.minor>]` prints the JSON, stable for redirection;
- every version's schema hosted at a permanent URL under the book, `https://pina-rs.github.io/pina/abi/schemas/<version>/manifest.schema.json`, with `$id` set to that URL. External tools — editors, CI, other languages — validate documents against the URL instead of trusting the bytes.

### The one-time rebase

Because no program is live and no consumer depends on the current shapes, the integer formats are retired rather than migrated:

- `pina_abi` moves out of `[group.core]` in `monochange.toml` onto its own single-member group, with a literal `version = "0.20.0"` and a `versioned_files` rule covering the root workspace pin;
- `MANIFEST_FORMAT_VERSION`, `PUBLICATION_FORMAT_VERSION`, the format 1–3 readers, and both downgrade ladders are deleted, along with the private `DataSchemaV2`, `PublicationLedgerV1/V2`, and `PublicationReceiptV1/V2` shims — roughly 700 lines and their tests;
- all 24 checked-in example manifests and ledgers are regenerated at `abiVersion` `0.20` by `pina migrations create`;
- a hypothetical pre-0.20 ledger that pinned a real deployment would have no upgrade path. None exists — the only non-empty receipt records the `fixture` cluster — and accepting that stranding is the price of the reset, paid once, now.

## Consequences

Benefits:

- a stamp names a real `pina_abi` release, and the version advances if and only if the contract moved;
- the committed value can lead the crate, so merged source never stamps a new shape with yesterday's version;
- core releases stop touching the ABI version entirely;
- every contract change is forced through the gate: converter, fixtures, schema, and a `breaking` changeset, with CI proving the chain against frozen bytes;
- the manifest roughly halves, because derivations are no longer stored twice;
- any consumer, in any language, can validate a document against a published, versioned JSON Schema;
- downgrade code that nothing called is gone.

Costs:

- two version numbers exist — the crate's semver and the ABI `major.minor` — and the not-lag and ahead-requires-changeset guards are what keep them honest;
- a `pina_abi` release cascades into a core release, so the contract cannot move without releasing its consumers;
- a breaking crate change with an unchanged document must still register a validating no-op edge; the fixture-drift guard catches the omission, but the ceremony is real;
- `schemars` becomes a direct dependency of `pina_abi`, pinned so generated schemas stay byte-stable.

## Alternatives considered

### Keep the unbounded integer counter

This is the current state. It works for a single maintainer tracking two integers, and it fails the question a third-party tool needs answered: which release wrote this, and is my reader new enough. Nothing stops the counter advancing for a non-breaking reason, which is how a format number loses its meaning.

### Derive the version from `CARGO_PKG_VERSION` at build time

An earlier draft of this ADR chose this. It eliminates the second version source, but it cannot represent the design's working state — a value ahead of its crate during the pre-release window — and monochange tried it twice and reverted both times (a `build.rs` on `CARGO_PKG_VERSION`, then `env!("CARGO_PKG_VERSION")`), once to keep an already-merged release commit readable and once after `cargo publish` failed because the build script was excluded from the package. In pina it is strictly worse: `pina_abi` sits in `group.core` with a workspace version, so a CLI fix or renderer tweak would restamp all 48 checked-in documents with no CI gate on the migration commands to catch the drift. Derivation advances the contract for reasons unrelated to the contract.

### Keep `pina_abi` in `group.core`

Any pin between the ABI version and `pina_abi`'s version is incoherent while that version moves with ~25 unrelated crates. Moving `pina_abi` onto its own single-member group is the structural change that makes "ABI version equals `pina_abi`'s `major.minor`" true, mirroring how monochange keeps its schema crates outside its release groups.

### Jump to `1.0.0`

Rejected by the bump table above: post-1.0 the pre-stable shift switches off and a `feat` on `pina_abi` would advance `major.minor` with no wire change, restamping every document. `1.0.0` is not reachable through the planner anyway — pre-1.0 a `major` changeset yields a minor bump — so taking it means hand-editing `Cargo.toml` past the machinery doing the work.

### Independent manifest and ledger axes

Separate values double the committed files, converter tables, fixture matrices, and schema pairs to preserve a distinction no reader uses. The two documents change shape rarely and always in the same crate; one shared axis answers both.

### Keep the stored derivations

The frozen `physical` descriptor and the duplicated hashes are tamper-evidence: a document that lies about its layout is rejected. But `validate` re-derives every one of them at load, publication receipts pin hashes computed from decoded content, and `codec` pins semantics — the stored copies are a cache of checks that run regardless, at roughly half the document's size.

### Keep the downgrade converters

Downgrades fail closed when they would lose information and nothing calls them. Writers always emit the current shape; a document that must reach an older tool is regenerated from source by that tool. Forward-only halves the surface that must be kept correct forever.

## Open questions

- Should a receipt's recorded `manifestSha256` become a live assertion that the checked-in manifest still matches what was published? Today it is written once and carried forward, so a rewritten manifest does not invalidate receipts. Making it live would strengthen the record and would also mean any document change invalidates every existing receipt — acceptable only while nothing is published, and the reason this ADR does not propose it now.
- Should `deny_unknown_fields` be relaxed so genuinely additive fields do not force a version advance? The strict reading is chosen because the documents are tool-written and tool-read; there is no third-party writer to be lenient with.
- Do the retired integer formats need fixtures preserved for forensic reading of an old commit, given that no artifact outside this repository ever carried one?
