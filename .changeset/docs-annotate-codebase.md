---
pina: docs
pina_macros: docs
pina_sdk_ids: docs
pina_token_2022_extensions: docs
---

Add comprehensive documentation across all crates:

- Add crate-level `//!` doc comments to `pina`, `pina_sdk_ids`, `pina_token_2022_extensions`, and the escrow example.
- Document all public traits (`AccountDeserialize`, `AccountValidation`, `AccountInfoValidation`, `IntoDiscriminator`, `HasDiscriminator`, `AsAccount`, `AsTokenAccount`, `LamportTransfer`, `CloseAccountWithRecipient`, `Loggable`, `TryFromAccountInfos`, `ProcessAccountInfos`) and their methods.
- Add `// SAFETY:` comments on all `unsafe` blocks in `loaders.rs`.
- Add `// SECURITY:` comments on unchecked token casts, lamport addition in `close_with_recipient`, and extension parsing in `pina_token_2022_extensions`.
- Add `// TODO:` comments for `assert_writable` error type, `combine_seeds_with_bump` panic vs Result, `parse_instruction` error suppression, and missing `taker_ata_b` validation in the escrow example.
- Fix typos: "larges" to "largest", "alignement" to "alignment", "vaue" to "value", "underling" to "underlying".
- Document `#[derive(Accounts)]`, darling argument structs, `nostd_entrypoint!` macro, `log!` macro, Pod module, and `impl_int_conversion!` macro.
- Rewrite `readme.md` with feature highlights, installation instructions, quick-start usage example, crate overview table, contributing section, and license.
