---
core: feat
pina_root: none
---

# Add IDL-driven Pina CPI generation

Adds a Codama Rust renderer and `@pina-rs/codama-renderer-cpi` visitor that generate standalone, `no_std` Pina CPI crates. Point the Codama pipeline or `pina cpi` at a Pina or Anchor IDL, select `cpi` in `pina.toml`, or run `pina generate --client cpi` in a Pina program. Every instruction becomes a direct call struct with typed account fields, a separately encoded instruction-data struct, and `.invoke()` and `.invoke_signed()` methods backed by Pina's validated `ProgramAccount`, `CpiHandle`, `ToCpiAccounts`, and `CpiContext` APIs.

Generated fields preserve IDL documentation and identify account privileges or instruction-argument roles. Program-ID fallback optional accounts and runtime signer choices retain their Codama semantics; omitted optional-account layouts, optional arguments, big-endian numbers, and unsupported argument or discriminator types are rejected with errors naming the exact node instead of generating instruction data that would never dispatch. Accounts the IDL derives from PDA seeds stay ordinary call fields, because the runtime resolves CPI accounts against the executing program's own account list.

New `pina init` projects select the standalone CPI client in `pina.toml` instead of creating a program-local `cpi` feature and handwritten `src/cpi.rs`. The `pina_bpf` CPI regression consumes the generated Prop AMM crate for both `.invoke()` and PDA-backed `.invoke_signed()`. A committed raw Anchor fixture is passed through the pinned converter and its generated crate is compile-checked end to end.
