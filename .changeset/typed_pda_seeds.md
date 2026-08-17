---
pina: feat
pina_macros: feat
pina_cli: feat
pina_codama_renderer: feat
---

feat: add typed `#[pda]` attribute for PDA seed declarations

Adds a `#[pda(seeds = [...], bump = <field>)]` attribute macro for `#[account]` structs, inspired by Quasar's typed seed declarations. The macro generates owned seed structs (`XxxSeeds` / `XxxSeedsWithBump`), `seeds()` / `as_slices()` / `with_bump()` helpers, `try_find_pda()` / `find_pda()` derivation helpers, and `assert_seeds()` stored-bump verification when a bump field is declared.

Supported seed types: `Address`, `u8`, `u16`, `u32`, `u64`, `[u8; N]`, and `const &[u8]` references. The CLI parses the attribute for IDL generation with accurate seed types (fixing the escrow `u64` seed being mis-typed as an address in generated clients), and the renderer emits `find_pda` / `create_pda` helpers for linked accounts. All examples now use the attribute instead of manual seed macros.
