---
pina_cli: feat
pina_cpi_renderer: feat
---

# Render real-world foreign IDLs into CPI crates

`pina cpi` could already turn a Codama or Anchor IDL into a standalone CPI crate, but it only survived IDLs that Pina itself generated. Pointed at a real third-party program it failed immediately: Anchor routes most non-primitive arguments through `definedTypes`, and those tables nest structs, enums, options, length-prefixed collections, and maps.

The renderer now resolves `definedTypeLinkNode` references, and declares the structs and enums it needs as Rust types with their own `encode_into`, so nesting stays recursive instead of unrolling into the instruction body. Options, length-prefixed strings and byte slices, maps, tuples, and enums whose variants carry differently sized payloads are all encoded. Anchor's `omitted` optional-account strategy is accepted when every optional account is trailing, which is the only layout a fixed-size CPI account array can express; a mid-list optional account still fails with the instruction named.

An instruction whose arguments are all fixed-width keeps returning `[u8; LEN]` and its generated output is byte-identical to before, so existing clients do not change. Only when an argument's length depends on caller-supplied data does the instruction gain `MAX_DATA_LEN` and an `encode_into`/`invoke_signed` pair that takes a caller-owned buffer. The crate stays `no_std` and allocator-free either way.

Two formats are deliberately rejected rather than guessed at, with the reason in the error message. `shortU16` is a variable-length 1-3 byte prefix that Anchor reserves for on-chain account lengths, so a fixed-width little-endian write would disagree with Anchor's reader. Floating-point arguments are rejected because a `no_std` crate has no float ABI conversion; the generated writer would have to reinterpret raw bits and silently disagree with a reader expecting a float.

`crates/pina_cpi_renderer/fixtures/` now holds four real IDLs — Switchboard On-Demand randomness, Metaplex Token Metadata, Meteora DLMM, and Squads v4 multisig — and `verify-codama-idls.sh` renders each one and compiles the result for `bpfel-unknown-none`. The Switchboard fixture is the capability benchmark from the hardening issue: its instruction discriminators, account counts, and encoded instruction lengths are asserted against the hand-written `pina-rs/lootbox` reference crate, and they match exactly.
