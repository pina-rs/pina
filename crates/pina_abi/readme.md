# `pina_abi`

Canonical, host-side ABI history types shared by Pina's migration CLI and procedural macros. It owns the versioned manifest envelope and the adjacent upgraders that normalize older Pina ABI documents before validation. Its format version is independent from application account, instruction, and event versions. This crate is not linked into Solana programs.
