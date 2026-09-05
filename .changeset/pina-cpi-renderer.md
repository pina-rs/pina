---
pina_cpi_renderer: feat
---

# Add the pina_cpi_renderer crate

New Codama Rust renderer that generates standalone Pinocchio CPI clients from Anchor and Codama IDLs. Point it at a Codama root node — the output of `@codama/nodes-from-anchor` for Anchor IDLs, or `pina generate` for Pina programs — and it renders one builder per instruction, each owning its discriminator bytes, argument encoding, and account metadata, ready to be consumed from another program via `pinocchio::cpi::invoke_signed`.

The renderer refuses rather than guess: optional accounts, optional signers, optional arguments, big-endian numbers, and unsupported argument or discriminator types are rejected with errors naming the exact node instead of generating instruction data that would never dispatch. Accounts the IDL derives from PDA seeds stay ordinary builder fields, because the runtime resolves CPI accounts against the executing program's own account list.
