---
pina_abi: breaking
---

# Store each ABI fact once in the 0.21 manifest

`abiVersion` advances to `0.21`. Readers convert `0.20` documents in memory, and the next `pina migrations create` writes them in the new shape.

- **The wire format moves into `abiVersion`.** Every schema stored `"codec": "pinaPodV2"`, a fact no reader could vary. It is now implied by the document version (`pina_abi::SCHEMA_CODEC`) and kept only in the schema hash preimage, so every published `schemaSha256` pin keeps its value. `DataCodec` is removed. A `PinaPod` release that changes wire bytes becomes a breaking `pina_abi` release rather than a per-schema tag.
- **The contract key is the identity.** Each history stored an `identity` object whose kind, width, and hex repeated its `kind:width:hex` key. The object is gone; `ContractIdentity::from_key` parses and validates the key, and `ContractHistory::identity` is filled from it when a manifest is read.
- **Events are versioned, not migrated.** An event history keeps one schema per version and no transitions, because old log bytes are decoded with the schema that emitted them. The converter drops recorded event transitions, and the ledger converter drops their pinned hashes and re-seals the receipt chain after proving it intact.
- **Instructions may omit the envelope.** `ContractHistory` gains `envelope` (written only as `"envelope": false`). An instruction recorded without an envelope has exactly one version and no transitions: its snapshot gates wire-breaking changes without adding a version byte.
- A version with no transition no longer writes `"transition": null`.
- `AbiStep` carries a converter per document, and `walk_document` takes an `AbiDocument`.
