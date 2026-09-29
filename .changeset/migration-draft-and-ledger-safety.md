---
pina_cli: feat
---

# Stop migration drafts and published history drifting

- A finished manual transition body survived later changes to its draft's layout and kept compiling, silently misplacing bytes. When a draft's destination schema changes, `pina migrations create` now moves the finished body to `vN_to_vM.rs.stale`, writes a fresh stub for the new offsets, and says so; the stub's marker blocks the build until the body is ported.
- `--manual <field>` was ignored for a draft already recorded as automatic. It now converts the draft to a manual stub.
- A publication receipt whose `history` was empty skipped the published-schema immutability check, so blanking it let a published schema be rewritten. Every command now refuses unpinned receipts with `UnpinnedPublication`, and the new `pina migrations reconcile --pin-legacy` pins them once the operator has verified the manifest.
- An unpublished history now rebinds to a changed `declare_id!` on the next `create` (the normal path after `pina keys new`), and `pina keys new`/`sync` point at that step. A published history still refuses a different program ID.
- `pina idl` and `pina generate` refuse to run for a program whose `pina.toml` enables `auto` before `pina migrations create` records a baseline, instead of emitting an IDL without the version byte.
- The generated `tests/abi_layout.rs` reports a compact contract's `MAX_SIZE` including the envelope header, like its `MIN_SIZE`.
