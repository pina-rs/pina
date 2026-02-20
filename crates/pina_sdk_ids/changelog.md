## 0.3.0 (2026-02-20)

### Breaking Changes

#### Migrate from `pinocchio-pubkey` to `solana-address` for program ID declarations:

- **`pinocchio_pubkey::declare_id!` → `solana_address::declare_id!`** — all 27 program and sysvar ID declarations now use the `solana-address` crate's `declare_id!` macro.
- **Dependency change** — replaced `pinocchio-pubkey` with `solana-address ^2.0` (features: `decode`).

### Documentation

#### Add comprehensive documentation across all crates:

- Add crate-level `//!` doc comments to `pina`, `pina_sdk_ids`, and the escrow example.
- Document all public traits (`AccountDeserialize`, `AccountValidation`, `AccountInfoValidation`, `IntoDiscriminator`, `HasDiscriminator`, `AsAccount`, `AsTokenAccount`, `LamportTransfer`, `CloseAccountWithRecipient`, `Loggable`, `TryFromAccountInfos`, `ProcessAccountInfos`) and their methods.
- Add `// SAFETY:` comments on all `unsafe` blocks in `loaders.rs`.
- Add `// SECURITY:` comments on unchecked token casts and lamport addition in `close_with_recipient`.
- Add `// TODO:` comments for `assert_writable` error type, `combine_seeds_with_bump` panic vs Result, `parse_instruction` error suppression, and missing `taker_ata_b` validation in the escrow example.
- Fix typos: "larges" to "largest", "alignement" to "alignment", "vaue" to "value", "underling" to "underlying".
- Document `#[derive(Accounts)]`, darling argument structs, `nostd_entrypoint!` macro, `log!` macro, Pod module, and `impl_int_conversion!` macro.
- Rewrite `readme.md` with feature highlights, installation instructions, quick-start usage example, crate overview table, contributing section, and license.

## 0.2.0 (2025-12-13)

### Breaking Changes

- Increase rust MSRV to `1.86.0` and `edition` to `2024`.

## 0.1.1 (2025-11-08)

### Fixes

- Add description for publishing

## 0.1.0 (2025-11-08)

### Breaking Changes

#### Initial release

The initial release of the `pina` libraries.
