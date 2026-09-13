---
pina_skill: feat
---

# Add migration workflow guidance to the bundled skill

The skill now ships `references/migrations.md`, an end-to-end operational checklist for the opt-in versioned ABI: the `[discriminator][schema version][payload]` envelope, per-contract and `[migrations].auto` opt-in, the `pina migrations make`/`check`/`status` loop, the checked-in manifest as macro policy source, automatic versus manual transitions, runtime migration and event provenance, read-only account handling, budget failures, and the legacy-adoption limit.

`SKILL.md` routes migration work to the new reference and adds the invariants that keep it safe: run `make` after a schema change, never strip a recorded envelope, keep the manifest and generated clients in sync, and preserve published wire formats. The CLI reference documents the `cli-rust`, `cli-ts`, and `cli-dart` client ecosystems and the migration-aware generated code, and the project-setup reference documents the `[migrations]`, `[clients]`, and `build.rs` requirements.
