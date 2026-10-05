# Changelog

All notable changes to the `pina_abi` ABI document contract are documented in this file.

This crate owns its own release line. The version recorded in `crates/pina_abi/ABI_VERSION` is the ABI document version, pinned to this package's `major.minor`: it advances with a `breaking` changeset here and with nothing else, so the ABI version moves if and only if the document contract moved.

## [0.21.0](https://github.com/pina-rs/pina/releases/tag/abi/v0.21.0) (2026-10-05)

Grouped release for `abi`.

### Breaking Changes

#### Store each ABI fact once in the 0.21 manifest

_Packages:_ 🔴 _pina_abi_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #568](https://github.com/pina-rs/pina/pull/568) · _Related issues:_ [#569](https://github.com/pina-rs/pina/issues/569), [#571](https://github.com/pina-rs/pina/issues/571)

`abiVersion` advances to `0.21`. Readers convert `0.20` documents in memory, and the next `pina migrations create` writes them in the new shape.

- **The wire format moves into `abiVersion`.** Every schema stored `"codec": "pinaPodV2"`, a fact no reader could vary. It is now implied by the document version (`pina_abi::SCHEMA_CODEC`) and kept only in the schema hash preimage, so every published `schemaSha256` pin keeps its value. `DataCodec` is removed. A `PinaPod` release that changes wire bytes becomes a breaking `pina_abi` release rather than a per-schema tag.
- **The contract key is the identity.** Each history stored an `identity` object whose kind, width, and hex repeated its `kind:width:hex` key. The object is gone; `ContractIdentity::from_key` parses and validates the key, and `ContractHistory::identity` is filled from it when a manifest is read.
- **Events are versioned, not migrated.** An event history keeps one schema per version and no transitions, because old log bytes are decoded with the schema that emitted them. The converter drops recorded event transitions, and the ledger converter drops their pinned hashes.
- **Instructions may omit the envelope.** `ContractHistory` gains `envelope` (written only as `"envelope": false`). An instruction recorded without an envelope has exactly one version and no transitions: its snapshot gates wire-breaking changes without adding a version byte.
- A version with no transition no longer writes `"transition": null`.
- **Process slots omit their defaults.** A `ProcessAccount` writes `writable`, `signer`, and `optional` only when true and `defaultValue` and `pda` only when set; readers treat an absent field as false or none. Process hashes are never pinned, so nothing published changes.
- **The publication ledger keeps only what nothing else records.** A receipt is now `{ rpcUrl, executableSha256, versions, abandoned? }` and the pending record `{ cluster, rpcUrl, executableSha256, versions }`. `sequence` repeated the receipt's position; `programId` repeated the manifest's, and the ledger belongs to the manifest beside it; `manifestSha256` hashed a manifest later versions rewrite, and nothing compared it; `previousReceiptSha256` chained the receipts, but anyone able to edit the file could recompute the chain, so it guarded nothing version control does not. `PublicationReceipt::sha256` is removed.
- **A published contract is its list of pins.** Each `versions` entry serializes as the bare list of pins, `"account:1:01": [{ "schemaSha256": … }, { "schemaSha256": …, "transitionSha256": … }]`: entry `n` pins version `n`, so the highest published version is the position of the last pin. `PublishedContract::version()` returns that position, `pins(n)` reports whether version `n` is pinned, and `PublishedContract::legacy` is removed. An empty list is invalid ("pins no versions"), because every receipt pins every version it made live.
- **The ledger converter proves what it drops.** It checks that `sequence` equals the receipt's position, that every record names the same `programId`, and that each contract's `version` is the position of its last pin, then drops them; it drops `manifestSha256` and the chain link unchecked. A 0.20 entry that pinned nothing cannot be represented, so the converter refuses it with an error naming `pina migrations reconcile --pin-legacy`. The new `pin_legacy_publications` fills such entries from a manifest (trust on first use) so the ledger converts.
- **Equivalent spellings are one schema.** The new `wire_type` maps type spellings that store the same bytes under the same reading to one form: `PodU16`/`PodI16`/`PodU32`/`PodI32`/`PodU64`/`PodI64`/`PodU128`/`PodI128`/`PodBool` to their native names, `Address` to `[u8; 32]`, `PodString<N>` and `PodString<N, 1>` to `String<N>`, and `PodVec<T, N>` and `PodVec<T, N, 2>` to `Vec<T, N>`, recursively through arrays, `Option`, and vectors. `DataSchema::same_wire` compares layouts, field names, and those forms, so tools can treat a respelling as no change. Recorded spellings and hashes are unchanged, and types that only share a width (`u64` and `i64`, `u32` and `f32`, `u8` and `bool`) stay different.
- `MigrationAuto` and `MigrationVersionType` implement `FromStr` for the spellings they display: `true`/`all`, `false`/`none`, or a comma-separated kind list, and `u8`, `u16`, or `u32`.
- `AbiStep` carries a converter per document, and `walk_document` takes an `AbiDocument`.

### Fixes

#### Judge instruction process compatibility on wire facts only

_Packages:_ 🟢 _pina_abi_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #597](https://github.com/pina-rs/pina/pull/597) · _Related issues:_ [#592](https://github.com/pina-rs/pina/issues/592)

Each recorded instruction account slot holds four wire facts, `name`, `writable`, `signer`, and `optional`, and two client hints, `defaultValue` (the known address generated clients fill in) and `pda`. Compatibility compared all six, so a change that only affected what generated clients fill in looked like a broken account list. A published instruction refuses any change to an existing slot, so improving the IDL extractor's PDA or known-address detection could leave a program unable to pass `pina migrations check` or generate its IDL, with neither the program nor any request changed.

Compatibility now compares wire facts only:

- `pina_abi` adds `ProcessAccount::same_wire` and `ProcessContract::same_wire`. `classify_process_transition` uses them, so a hint-only difference is `Unchanged` and appending optional slots still counts as `AppendOptional` when the existing slots differ only in hints. Every name, signer, writable, or optional change, reorder, removal, or new required slot still fails.
- `pina migrations create`, `check`, `status`, and IDL generation compare a recorded snapshot with source the same way. A hint-only change is not drift, consumes no version, and does not rewrite the manifest: the recorded hints are carried forward until a wire change rewrites the draft or appends a version.

Nothing in the document format changes, and no stored hash moves. Publication receipts pin only `schemaSha256` and `transitionSha256`, and neither covers a process hint. See ADR 0012 for the reasoning and the alternatives considered.

The agent skill's migrations reference (`pina_skill`) states the same rule: `defaultValue` and `pda` are client hints that never block a published instruction.

## [0.20.0](https://github.com/pina-rs/pina/releases/tag/abi/v0.20.0) (2026-09-23)

Grouped release for `abi`.

### Breaking Changes

#### Version the ABI document on `pina_abi`'s own release line

_Packages:_ _pina_abi_

`pina_abi` leaves the `core` release group and owns its release line, so a CLI fix or renderer tweak can no longer move the ABI document version. The document records one `abiVersion` string — the `major.minor` committed in `crates/pina_abi/ABI_VERSION` — in place of the independent manifest and publication integer counters, and a release that changes the document shape advances it through a `breaking` changeset on this crate alone.

`pina_cli` and `pina_macros` follow the new model: schema versions are addressed by their position in a contract history, a transition's adjacency is implied by the version it sits on, the physical descriptor is derived on load, and publication pins compute their schema hashes from the decoded content. `pina abi schema` prints a document's JSON Schema from the same types that read and write it.

The document also stops storing its own derivations. The physical layout descriptor, the per-version `version`/`schemaSha256`/`processSha256` fields, the transition's adjacency numbers and neighbour hashes, the frozen process proof, the instruction process constraints, and the receipt cluster label are all recomputed on load from the facts a reader cannot derive. `examples/*/migrations/manifest.json` and `publications.json` are regenerated by `pina migrations create`.

Readers normalize any supported document through an ordered table of adjacent converters; the integer format 1–3 readers, their downgrade ladders, and the private compatibility shims are deleted. A document stamped above the reader, or below the oldest supported version, fails closed and names the supported version. Document types derive `JsonSchema`, so each document's schema is generated rather than hand-written and is published at a permanent versioned URL.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #468](https://github.com/pina-rs/pina/pull/468) · _Related issues:_ [#491](https://github.com/pina-rs/pina/issues/491), [#492](https://github.com/pina-rs/pina/issues/492), [#494](https://github.com/pina-rs/pina/issues/494)

### Features

#### Accept const capacities in schemas

_Packages:_ _pina_abi_

Compact account and instruction schemas now accept a `const` item wherever they previously required an integer literal:

```rust
const MAX_MEMBERS: usize = 24;

#[account(discriminator = MultisigAccountType, compact)]
pub struct Multisig {
	pub bump: u8,
	pub member_keys: Vec<Address, MAX_MEMBERS>,
}
```

`Vec<T, N>`, `PodVec<T, N, PFX>`, `String<N>`, `PodString<N, PFX>`, an `Option<...>` around them, and fixed `[T; N]` arrays and instruction arguments all resolve a capacity through a constant. Constants may be arithmetic over other constants and may be declared in any module of the crate, so one bound is declared once and reused in the account, the instruction that writes it, and the helper that sizes it.

Pina resolves each capacity during expansion and records the number it evaluates to. The ABI layer still receives concrete values, so replacing a literal with a constant of the same value leaves `migrations/manifest.json`, the generated `tests/abi_layout.rs` assertions, `MAX_SIZE`/`MIN_SIZE`/`HEADER_SIZE`, and `projected_bytes(...)` byte-identical, and `pina migrations check` and the Codama IDL unchanged.

A capacity that cannot be evaluated at expansion time now fails the build with a diagnostic naming the expression and pointing at the workaround, instead of the previous `unsupported compact field` or `arrays require an integer literal length` message. An associated constant such as `Bounds::MAX_MEMBERS` is not resolved; declare the bound as a `const` item.

`pina_abi` gains `SchemaConsts`, the shared evaluator both the macros and the CLI use so the two layers agree on what a constant means. `pina_cli` resolves capacities while it parses a program, so migration and IDL tooling reads the same numbers the compiler does.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #463](https://github.com/pina-rs/pina/pull/463) · _Closed issues:_ [#460](https://github.com/pina-rs/pina/issues/460) · _Related issues:_ [#491](https://github.com/pina-rs/pina/issues/491), [#492](https://github.com/pina-rs/pina/issues/492), [#494](https://github.com/pina-rs/pina/issues/494)

### Fixes

#### Enumerate the auto policy's kind names in the schema

_Packages:_ _pina_abi_

`MigrationAuto` has hand-written serde impls, so its JSON Schema is hand-written too, and it described its items as any string. A consumer validating a manifest against the published schema therefore admitted `["states"]` while the reader rejected it with `unknown migration kind` — the schema promised more than the code accepted, which defers the failure to runtime.

The items now carry the valid names, read from `ContractKind::config_name`, the same source `from_config_name` resolves through, so the schema and the reader cannot drift apart. The checked-in artifacts under `crates/pina_abi/schemas/`, the frozen fixtures, and the published copies under the book are regenerated to match, and a test asserts agreement in both directions: every name the schema advertises decodes, and every spelling the reader rejects stays unadvertised.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #473](https://github.com/pina-rs/pina/pull/473) · _Related issues:_ [#468](https://github.com/pina-rs/pina/issues/468), [#491](https://github.com/pina-rs/pina/issues/491), [#492](https://github.com/pina-rs/pina/issues/492), [#494](https://github.com/pina-rs/pina/issues/494)

#### Close the confirmed 2026-09-19 sweep findings

_Packages:_ _pina_abi_

`#[derive(Accounts)]` now rejects optional account fields followed by positional fields at compile time: an absent optional consumes its program-address filler slot, so dropping the filler shifted every later binding by one and each validation ran against the wrong account. Move optional fields to the end; a trailing `remaining` slice remains allowed.

The generated PDA seed slices now emit in declaration order instead of constants-first, so derived addresses match the order the IDL publishes and clients derive the same address the program verifies.

Enveloped instructions now pin the migration version byte at dispatch through an `IntoDiscriminator` gate derived from the checked-in manifest: a missing or unknown version byte fails closed before any handler runs, closing the zero-field fail-open. Programs whose manifests declare no instruction contracts expand to identical code.

The gate covers exactly the zero-field instructions where the fail-open existed — 38 across the workspace, at a cost of 3-9 CU each, recorded as reviewed `runtimeApprovedTotals` in `scripts/compute-unit-policy.json` (payload instructions already validate their version in the generated parse, so their dispatch is bit-identical to before).

A migration manifest whose `rustName` is not a plain Rust identifier now fails validation with a typed error instead of panicking inside the macro expansion, and duplicate field names within one schema version are rejected per version.

The account migration executor zero-fills the grown region before each transition applies, matching the instruction and event workspaces, so a hand-written transition that skips an added field commits zeros rather than the account's own realloc residue.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #472](https://github.com/pina-rs/pina/pull/472) · _Related issues:_ [#491](https://github.com/pina-rs/pina/issues/491), [#492](https://github.com/pina-rs/pina/issues/492), [#494](https://github.com/pina-rs/pina/issues/494)

### Notes

#### Rename `pina migrations make` to `pina migrations create`

_Packages:_ _pina_abi_

`pina migrations make` is now `pina migrations create`. The old spelling is not kept as an alias: the subcommand parses as an unknown command, so a script still calling `make` fails loudly instead of appearing to work.

The rename reaches every surface that named the command. Compiler diagnostics from `pina_macros` and the `pina_abi` document readers, the `pina migrations` help text, `pina init`'s scaffolded next steps, and the growth warnings `pina migrations check` and `status` print all say `create` now. Docs, the bundled `pina_skill` reference, and the migration walkthrough script follow.

The internal API moved with it: `pina_cli::migrations::make_migrations` is `create_migrations`, `make_migrations_with_answers` is `create_migrations_with_answers`, and the `MakeMigrationsOutput` result struct is `CreateMigrationsOutput`. `MigrationCommands::Make` is `MigrationCommands::Create`.

Two things deliberately did not change. Checked-in transition files keep their recorded `@generated by pina migrations make` header, because their bytes are pinned by SHA-256 in `migrations/manifest.json` and rewriting a published transition is exactly the history rewrite the pin exists to prevent; the generator only writes that header for new transitions. Released changelog and publication records keep the historical command name as written.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #475](https://github.com/pina-rs/pina/pull/475) · _Related issues:_ [#491](https://github.com/pina-rs/pina/issues/491), [#492](https://github.com/pina-rs/pina/issues/492), [#494](https://github.com/pina-rs/pina/issues/494)

#### Close the shadow-PDA and discriminator-collision gaps

_Packages:_ _pina_abi_

Fixed-account `#[pda]` schemas now generate `load_checked_pda` and `load_checked_pda_mut` alongside the stored-bump `load_pda` pair. The checked loaders search the seeds for the canonical bump and reject both an account at any other address and a stored bump that is not canonical — the fixed-account counterpart of the compact family's `with_checked_pda`, and previously the one PDA family with no canonical option at all. The audit proved the gap executable: a shadow account created at a noncanonical bump whose stored bump field matches passes `load_pda` byte for byte, so a program whose seeds do not bind a required signer accepts the shadow and the canonical account as the same logical entity (`crates/pina/tests/audit_adversarial.rs`). The stored-bump loaders keep their single-derivation cost and their documentation of when they are safe.

A new deny lint `deny_colliding_account_discriminators` closes the other proven framework finding. Discriminators are author-chosen integers and rustc only rejects duplicates within one enum, so two account enums that agree on a value and a serialized width pass every typed loader check — owner, discriminator, exact size — and either account deserializes as the other (the sealevel-attacks type-cosplay class; proven executable in the same test file). The lint collects every generated `impl HasDiscriminator` for types that also implement pina's account traits, evaluates the `VALUE` const and the repr width, and denies any value claimed by two different account types. Events share the trait shape but live in the log namespace, so they are excluded by construction. A UI fixture pair pins the collision and the clean case, and the lint ships in the catalog with the CLI's `lints.json`, `--explain` reference, and the docs page updated to match.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #483](https://github.com/pina-rs/pina/pull/483) · _Related issues:_ [#472](https://github.com/pina-rs/pina/issues/472), [#491](https://github.com/pina-rs/pina/issues/491), [#492](https://github.com/pina-rs/pina/issues/492), [#494](https://github.com/pina-rs/pina/issues/494)
