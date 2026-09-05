---
pina_cpi_renderer: feat
pina_cli: feat
pina_codama_renderer_cpi: feat
---

# Add IDL-driven Pina CPI generation

Adds a Codama Rust renderer and `@pina-rs/codama-renderer-cpi` visitor that generate standalone, `no_std` Pina CPI crates. Point the Codama pipeline or `pina cpi` at a Pina or Anchor IDL, or run `pina generate --client cpi` in a Pina program. Every instruction becomes a typed builder with `.invoke()` and `.invoke_signed()` methods backed by Pina's validated `ProgramAccount`, `CpiHandle`, `ToCpiAccounts`, and `CpiContext` APIs.

The renderer refuses rather than guess: optional accounts, optional signers, optional arguments, big-endian numbers, and unsupported argument or discriminator types are rejected with errors naming the exact node instead of generating instruction data that would never dispatch. Accounts the IDL derives from PDA seeds stay ordinary builder fields, because the runtime resolves CPI accounts against the executing program's own account list.
