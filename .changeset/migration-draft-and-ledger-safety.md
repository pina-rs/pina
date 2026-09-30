---
pina_cli: breaking
---

# Stop migration drafts and published history drifting

- A finished manual transition body survived later changes to its draft's layout and kept compiling, silently misplacing bytes. When a draft's destination schema changes, `pina migrations create` now moves the finished body to `vN_to_vM.rs.stale`, writes a fresh stub for the new offsets, and says so; the stub's marker blocks the build until the body is ported.
- `--manual <field>` was ignored for a draft already recorded as automatic. It now converts the draft to a manual stub.
- A publication receipt whose `history` was empty skipped the published-schema immutability check, so blanking it let a published schema be rewritten. An unpinned receipt is now refused when the ledger is decoded: a 0.21 entry with no pins fails with "pins no versions", and a 0.20 ledger that names versions without pinning them fails with an error naming the new `pina migrations reconcile --pin-legacy`, which pins those entries from the manifest once the operator has verified it and writes the ledger in the current shape.
- An unpublished history now rebinds to a changed `declare_id!` on the next `create` (the normal path after `pina keys new`), and `pina keys new`/`sync` point at that step. A published history still refuses a different program ID.
- The generated `tests/abi_layout.rs` reports a compact contract's `MAX_SIZE` including the envelope header, like its `MIN_SIZE`.
- Library API: `CreateMigrationsOutput`, `KeySync`, and `KeyGeneration` gain public fields, and `KeysError::MissingKeypair`, `CodamaError::ClientManifest`, and `DeployError::CommandInterrupted` are new variants, so exhaustive struct literals and matches over them must be updated.
