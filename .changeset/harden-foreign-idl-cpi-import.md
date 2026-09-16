---
pina_cli: feat
pina_cpi_renderer: feat
---

# Render real-world foreign IDLs into CPI crates

`pina cpi` could already turn a Codama or Anchor IDL into a standalone CPI crate, but it only survived IDLs that Pina itself generated. Pointed at a real third-party program it failed immediately: Anchor routes most non-primitive arguments through `definedTypes`, and those tables nest structs, enums, options, length-prefixed collections, and maps.

The renderer now resolves `definedTypeLinkNode` references, and declares the structs and enums it needs as Rust types with their own `encode_into`, so nesting stays recursive instead of unrolling into the instruction body. Options, length-prefixed strings and byte slices, maps, tuples, and enums whose variants carry differently sized payloads are all encoded. Anchor's `omitted` optional-account strategy is accepted when every optional account is trailing, which is the only layout a fixed-size CPI account array can express; a mid-list optional account still fails with the instruction named.

An instruction whose arguments are all fixed-width keeps returning `[u8; LEN]` and its generated output is byte-identical to before, so existing clients do not change. Only when an argument's length depends on caller-supplied data does the instruction gain `MAX_DATA_LEN` and an `encode_into`/`invoke_signed` pair that takes a caller-owned buffer. The crate stays `no_std` and allocator-free either way.

Two formats are deliberately rejected rather than guessed at, with the reason in the error message. `shortU16` is a variable-length 1-3 byte prefix that Anchor reserves for on-chain account lengths, so a fixed-width little-endian write would disagree with Anchor's reader. Floating-point arguments are rejected because a `no_std` crate has no float ABI conversion; the generated writer would have to reinterpret raw bits and silently disagree with a reader expecting a float.

`pina import <name> --program-id <pubkey>` is the supported entry point. It reads the IDL from `--idl <file>`, `--url <url>`, or the cluster's canonical on-chain program metadata, renders the crate into `clients/cpi/<name>`, and stamps the generated README with the IDL's SHA-256, the source it came from, and the generator version. Re-importing an unchanged IDL reports "already up to date" instead of rewriting the crate, and `--idl` plus `--url` together is rejected rather than silently preferring one.

The provenance digest is the point: a CPI crate is a copy of another program's interface, so without a recorded digest a reviewed crate can be regenerated from a different IDL with nothing in the diff to show it.

Each account in the IDL also becomes a read-only parser: a struct with the account's fields, its discriminator as a public constant, an encoded-size constant, a `matches` guard, and — when every field has a fixed offset — a `parse` that reads the fields out and returns `None` on a short buffer or a foreign discriminator. The Switchboard randomness account parses to `LEN = 408`, matching the hand-written `pina-rs/lootbox` parser byte for byte, so that crate can be regenerated instead of maintained. An account whose layout has a variable-width field still gets its struct and discriminator, but no parser: a partial one would read the wrong offsets, so the generated crate records the reason in a `PARSER_UNSUPPORTED` constant instead.

`crates/pina_cpi_renderer/fixtures/` now holds four real IDLs — Switchboard On-Demand randomness, Metaplex Token Metadata, Meteora DLMM, and Squads v4 multisig — and `verify-codama-idls.sh` renders each one and compiles the result for `bpfel-unknown-none`. The Switchboard fixture is the capability benchmark from the hardening issue: its instruction discriminators, account counts, and encoded instruction lengths are asserted against the hand-written `pina-rs/lootbox` reference crate, and they match exactly.
