---
pina_macros: feat
pina_cli: feat
pina_skill: docs
---

# Derive the reserved `Migrate` ladder from the manifest

The reserved `Migrate` instruction's account ladder is now derived instead of declared. A program opts into the route with `migrations_max_lamports` alone:

```rust
#[discriminator(entrypoint, migrations_max_lamports = MAX_INLINE_MIGRATION_LAMPORTS)]
pub enum ProgramInstruction {
	// …
}
```

Slots come from `migrations/manifest.json` — one per enveloped account contract, in the manifest's identity-sorted order, which is exactly the order generated clients compose. The endpoint and its callers therefore cannot disagree about slot assignment, and adding an account no longer requires editing the entrypoint.

An explicit `migrations(A, B)` list stays supported as an override. It is the only way to expose several accounts of the _same_ contract in one sweep, because the manifest records contracts rather than account instances; `examples/migrations_program` keeps its list for that batching demo. Listing contracts without a budget is still a configuration error, because the budget is program policy and a default would silently misprice rent transfers.

`pina build`, `pina idl`, and `pina doctor` now fail closed when more than one discriminator enum declares `entrypoint`, naming every offending enum. The proc macro cannot detect this on its own — separate macro invocations share no state — so the earlier symptom was a duplicate-symbol link error during the SBF build.

The bundled skill and `docs/src/migrations/flow.md` document the derived default, the batching override, and the single-entrypoint rule.
