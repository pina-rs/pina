# ADR 0012: Judge instruction process compatibility on wire facts only

- Status: Accepted
- Date: 2026-10-04
- Deciders: Pina maintainers
- Related: [ADR 0007](./0007-first-class-versioned-abi.md), [ADR 0009](./0009-abi-document-versioning.md), [Manage ABI migrations](../cli/migrations.md#change-an-instruction-process)

## Context

`pina migrations create` snapshots each instruction's positional account list as a `ProcessContract`. Every `ProcessAccount` slot records its name, its `writable`, `signer`, and `optional` flags, and two more fields: `defaultValue`, the known address generated clients fill in, and `pda`, the Pina PDA the slot belongs to.

The document already says that only wire facts belong in the snapshot, but compatibility compared every field. `classify_process_transition`, the `create` path, and the drift check behind `pina migrations check`, `status`, and IDL generation all used whole-slot equality. So a change that only affects what generated clients fill in looked like a wire break.

The trigger was better PDA detection in the IDL extractor. Typed loads and helper functions now reveal PDAs it missed before, so existing slots gain a `pda`. For an unpublished draft, `create` just re-records it. A published instruction refuses any change to an existing slot, so upgrading `pina_cli` would leave such a program unable to pass `pina migrations check` or generate its IDL, although its program and every client request are unchanged.

`defaultValue` and `pda` do not change what the program accepts. A client that passes an account explicitly sends the same list whether or not the IDL could have derived it. The wire facts are `name`, `writable`, `signer`, and `optional`: they decide whether an old account list still parses and is authorised. (Renaming a slot is treated as breaking because the name is the slot's stable identity in the snapshot.)

## Decision

Process compatibility compares wire facts only.

- `ProcessAccount::same_wire` compares `name`, `writable`, `signer`, and `optional`. `ProcessContract::same_wire` requires the same slots in the same order. Both ignore `defaultValue` and `pda`.
- `classify_process_transition` returns `Unchanged` when the processes are equal on the wire, and `AppendOptional` when the destination's leading slots are equal on the wire and every appended slot is optional. `ProcessTransitionKind::Unchanged` now means "unchanged on the wire", not byte-for-byte.
- `pina migrations create`, `check`, `status`, and IDL generation compare the recorded snapshot with source the same way, through `CurrentContract::matches_wire`. A hint-only difference is not drift, consumes no version, and does not rewrite the manifest.
- A hint-only difference keeps the recorded snapshot's hints. Hints are carried forward unchanged until a wire change rewrites an unpublished draft or appends a version; that write records the source's current hints along with its wire facts.

The document format does not change: the same fields are written the same way, and no stored or pinned value moves.

## Consequences

- Upgrading `pina_cli` with better PDA or known-address detection never blocks a published instruction, and never forces a re-recording of one.
- A published snapshot's hints can lag the source. That is harmless, because nothing reads them for compatibility or client generation: clients are generated from the current IDL, not the manifest. The recorded hints document what the clients of that version filled in.
- `ProcessContract::sha256` still hashes the whole recorded document, hints included. It identifies a document, not a compatibility class. No receipt pins a process hash: a publication receipt pins `schemaSha256`, which covers only the payload schema, and `transitionSha256`, which covers only transition source. Neither can move when a hint changes, so a hint-only change cannot cause a hash mismatch. The checked-in fixtures under `crates/pina_abi/fixtures` and every existing document hash stay byte-identical.
- The `ProcessAccount` doc comments are emitted into the published JSON Schemas, which are frozen per `abiVersion`. The rule is documented on `same_wire` and here instead of in those comments.
- A name, signer, writable, or optional change, a reorder, a removal, or a new required slot still fails closed and still needs a new instruction discriminator.

## Alternatives considered

- **Re-record hints on a published version in place.** Process hashes are not pinned, so `create` could rewrite a published snapshot's hints without breaking a receipt. It was rejected because it rewrites a published version whenever detection improves, which churns the manifest and the generated `tests/abi_layout.rs` header for no wire reason, and because a snapshot that silently changes after publication is harder to audit.
- **Append a new version for a hint-only change.** That spends a version number, and for an enveloped instruction forces a no-op transition, to record a fact the program does not depend on.
- **Remove `defaultValue` and `pda` from the snapshot.** That is a document-format change: it needs a new `abiVersion`, a converter, and new schemas, and it drops a useful record of what each version's clients filled in. Ignoring the hints when comparing gets the compatibility fix without a format change.
- **Keep comparing every field and tell users to re-record.** A published version cannot be re-recorded, so this leaves the program stuck.
