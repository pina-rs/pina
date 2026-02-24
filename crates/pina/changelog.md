## 0.3.0 (2026-02-20)

### Breaking Changes

- **BREAKING**: `assert_writable()` now returns `ProgramError::InvalidAccountData` instead of `ProgramError::MissingRequiredSignature`. The previous error type was misleading — a writability check is unrelated to signatures. Code that matches on `MissingRequiredSignature` from `assert_writable()` must update to match `InvalidAccountData`.
- **BREAKING**: `combine_seeds_with_bump` now returns `Result<[Seed; MAX_SEEDS], ProgramError>` instead of `[Seed; MAX_SEEDS]`. The function previously used `assert!` which would abort the transaction on-chain with no recovery. It now returns `Err(ProgramError::InvalidSeeds)` when `seeds.len() >= MAX_SEEDS`, giving callers a graceful error path. Update call sites to handle the `Result` with `?` or pattern matching.
- **BREAKING**: Removed the `Loggable` trait. This trait had no implementations anywhere in the codebase and was dead code. If you were depending on this trait, define your own logging trait or use `log!` / `sol_set_return_data` directly.

#### Migrate to pinocchio 0.10.x with breaking type changes across the entire API surface:

- **`AccountInfo` → `AccountView`** — all trait signatures, implementations, and generated code now use `AccountView` from pinocchio 0.10.x.
- **`Pubkey` → `Address`** — the 32-byte public key type is now `Address` from `solana-address` (with `bytemuck` feature for `Pod`/`Zeroable`/`Copy` derives). All function parameters, struct fields, and return types updated.
- **`pinocchio-pubkey` → `solana-address`** — replaced the `pinocchio-pubkey` dependency with `solana-address ^2.0` for `declare_id!`, `address!`, and address constants.
- **`pinocchio-log` → `solana-program-log`** — replaced `pinocchio-log` with `solana-program-log ^1.1` for on-chain logging. The `log!` macro now uses `$crate`-based paths for cross-crate compatibility.
- **Method renames** — `key()` → `address()`, `try_borrow_data()` → `try_borrow()`, `try_borrow_mut_data()` → `try_borrow_mut()`, `realloc()` → `resize()`, `data_is_empty()` → `is_data_empty()`, `minimum_balance()` → `try_minimum_balance()`.
- **Module renames** — `pinocchio::pubkey` → `pinocchio::address`, `pinocchio::account_info` → `pinocchio::account`, `pinocchio::program_error` → `pinocchio::error`.
- **`owner()` is now unsafe** — `AccountView::owner()` requires an `unsafe` block as it reads from raw account memory. All validation methods updated accordingly.
- **PDA functions cfg-gated** — `try_find_program_address`, `find_program_address`, and `create_program_address` are now behind `#[cfg(target_os = "solana")]` with native stubs returning `None`/`Err`.
- **Dependency bumps** — pinocchio-system ^0.5, pinocchio-token ^0.5, pinocchio-token-2022 ^0.2, pinocchio-associated-token-account ^0.3, pinocchio-memo ^0.3.

#### Remove the `pina_token_2022_extensions` crate from the workspace entirely.

The upstream `pinocchio-token-2022` crate is adding native extension parsing support, making this crate redundant. The crate was never widely adopted and removing it simplifies the workspace.

**What was removed:**

- `crates/pina_token_2022_extensions/` directory and all source files
- Workspace member entry in root `Cargo.toml`
- Package configuration in `knope.toml`
- All references in documentation and changeset files

Extensions support can be re-added once `pinocchio-token-2022` ships its built-in extension types.

### Features

#### Add three custom dylint lint rules to catch common Solana security mistakes at compile time:

- `require_owner_before_token_cast`: Warns when `as_token_mint()`, `as_token_account()`, `as_token_2022_mint()`, or `as_token_2022_account()` is called without a preceding `assert_owner()` or `assert_owners()` on the same receiver.
- `require_empty_before_init`: Warns when `create_program_account()` or `create_program_account_with_bump()` is called without a preceding `assert_empty()` on the target account.
- `require_program_check_before_cpi`: Warns when `.invoke()` or `.invoke_signed()` is called without a preceding program address verification via `assert_address()`, `assert_addresses()`, or `assert_program()`.

#### Security and robustness improvements from codebase audit:

**Critical fixes:**

- `discriminator_from_bytes` now returns `Err(ProgramError::InvalidAccountData)` instead of panicking when the input slice is shorter than the discriminator size. This prevents on-chain aborts from malformed instruction data.
- `matches_discriminator` now returns `false` instead of panicking on short input slices.
- `as_account` and `as_account_mut` now check `data_len() < size_of::<T>()` before creating a raw-parts slice, returning `ProgramError::AccountDataTooSmall` instead of reading out-of-bounds memory.
- `parse_instruction` now validates data length before calling `discriminator_from_bytes` for defense-in-depth.

**Security improvements:**

- `close_account` now zeroes account data via `resize(0)` before closing, matching the behavior of `close_with_recipient` and preventing stale data from being read by subsequent transactions.
- Added checked token cast methods: `as_checked_token_mint()`, `as_checked_token_account()`, `as_checked_token_2022_mint()`, `as_checked_token_2022_account()` that verify token program ownership before casting.
- Deprecated `find_program_address` in favor of `try_find_program_address` which returns `Option` instead of panicking on-chain.

**New error variants** (non-breaking, `#[non_exhaustive]` enum):

- `PinaProgramError::DataTooShort` — data shorter than expected minimum.
- `PinaProgramError::InvalidAccountSize` — account size mismatch.
- `PinaProgramError::InvalidTokenOwner` — account not owned by expected token program.
- `PinaProgramError::SeedsTooMany` — too many PDA seeds provided.

**New Pod types:**

- `PodI32` — alignment-safe `i32` wrapper for `#[repr(C)]` account structs.
- `PodI128` — alignment-safe `i128` wrapper for `#[repr(C)]` account structs.

### Fixes

- `close_with_recipient()` now uses `checked_add` for lamport arithmetic instead of unchecked addition, returning `ProgramError::ArithmeticOverflow` on overflow. While overflow was practically impossible due to total lamport supply constraints, this follows the defensive pattern used in `send()` and prevents undefined behavior in edge cases.
- Fixed `write_discriminator` to correctly slice the destination buffer to `Self::BYTES` before copying. Previously, if the destination buffer was larger than the discriminator size, `copy_from_slice` would panic due to length mismatch.
- Replaced four duplicate `AccountValidation` trait implementations for SPL token types with a single `impl_account_validation!` macro, reducing code duplication while preserving identical behavior. Also fixed the inverted condition in the `Mint` impl's `assert` method which incorrectly returned `Ok` when the condition was `false`.

#### Fix three critical bugs in the `pina` core crate:

- **`assert_seeds()` inverted logic** — previously returned `Ok` when the account key did _not_ match the derived PDA. Now correctly returns `Ok` when the key matches, consistent with `assert_seeds_with_bump` and `assert_canonical_bump`.
- **`send()` never wrote back lamports** — the `checked_sub` / `checked_add` results were computed but discarded, so lamport balances never actually changed. Now assigns the results back via `*self_lamports = ...` and `*recipient_lamports = ...`.
- **Typo fix** — corrected "recipent" to "recipient" in the `send()` error log message.

#### Addressed outstanding review follow-ups and documentation quality:

- Enabled the `solana-address` `curve25519` feature in workspace dependencies so PDA helpers are available in host builds.
- Added missing `AccountValidation` implementations for token mint/account types used by token helpers.
- Replaced unchecked counter increment arithmetic in the example with checked overflow handling.
- Added an mdBook documentation site under `docs/` and wired docs verification into CI.

#### Hardened account and discriminator handling to avoid panic paths and unsafe deserialization assumptions:

- `IntoDiscriminator` primitive implementations now handle short input slices without panicking.
- `AsAccount`/`AsAccountMut` now require exact account data length before reinterpretation.
- PDA helper wrappers now work consistently on native targets and include roundtrip tests.
- Lamport send/close helpers now reject same-account recipients and enforce writable preconditions before balance mutation.

Also improved security examples by replacing saturating transfer/withdraw arithmetic with checked math that returns explicit `ProgramError` values on insufficient funds or overflow.

### Notes

#### Add three example programs ported from `solana-developers/program-examples`, demonstrating core pina features with comprehensive documentation and unit tests:

- **`hello_solana`** — Minimal program showing basic pina structure: `declare_id!`, `#[discriminator]`, `#[instruction]`, `#[derive(Accounts)]`, `ProcessAccountInfos`, and `nostd_entrypoint!`.
- **`counter_program`** — PDA-based account state management with `#[account]`, `create_program_account`, `as_account_mut`, validation chains, and seed macros.
- **`transfer_sol`** — Two SOL transfer methods: CPI via `system::instructions::Transfer` and direct lamport manipulation via `LamportTransfer::send()`, plus custom error types with `#[error]`.

Each example includes a learning progression from basic → intermediate → advanced, with detailed module-level docs explaining every pina feature used.

### Documentation

- Add comprehensive security guide with 11 sealevel-attacks categories. Each category includes a readme explaining the vulnerability, an insecure example demonstrating the vulnerable pattern, and a secure example showing the correct pina validation approach. Covers signer authorization, account data matching, owner checks, type cosplay, initialization guards, arbitrary CPI prevention, duplicate mutable accounts, bump seed canonicalization, PDA sharing, account closing, and sysvar address checking. Updated README.md with security best practices section and expanded examples table.
- Added comprehensive documentation to core traits: `AccountDeserialize` (explaining the discriminator check asymmetry between account and instruction types), `AsAccount`, `AsTokenAccount`, `LamportTransfer`, and `CloseAccountWithRecipient`. Also documented collision avoidance guidance for `PinaProgramError` discriminant values.

#### Add comprehensive documentation across all crates:

- Add crate-level `//!` doc comments to `pina`, `pina_sdk_ids`, and the escrow example.
- Document all public traits (`AccountDeserialize`, `AccountValidation`, `AccountInfoValidation`, `IntoDiscriminator`, `HasDiscriminator`, `AsAccount`, `AsTokenAccount`, `LamportTransfer`, `CloseAccountWithRecipient`, `Loggable`, `TryFromAccountInfos`, `ProcessAccountInfos`) and their methods.
- Add `// SAFETY:` comments on all `unsafe` blocks in `loaders.rs`.
- Add `// SECURITY:` comments on unchecked token casts and lamport addition in `close_with_recipient`.
- Add `// TODO:` comments for `assert_writable` error type, `combine_seeds_with_bump` panic vs Result, `parse_instruction` error suppression, and missing `taker_ata_b` validation in the escrow example.
- Fix typos: "larges" to "largest", "alignement" to "alignment", "vaue" to "value", "underling" to "underlying".
- Document `#[derive(Accounts)]`, darling argument structs, `nostd_entrypoint!` macro, `log!` macro, Pod module, and `impl_int_conversion!` macro.
- Rewrite `readme.md` with feature highlights, installation instructions, quick-start usage example, crate overview table, contributing section, and license.

#### Rewrite `readme.md` with comprehensive documentation for the pinocchio 0.10.x API:

- Updated quick start example with `AccountView` and `Address` types
- Added crate features table (`derive`, `logs`, `token`)
- Added core concepts sections: entrypoint, discriminators, accounts, instructions, events, errors, validation chains, Pod types, CPI helpers, logging
- Added full account validation assertions reference
- Updated crate table (removed `pina_token_2022_extensions`)
- Updated building for SBF and testing sections

Updated `CLAUDE.md` to reflect pinocchio 0.10.x architecture:

- Updated workspace crates list (removed `pina_token_2022_extensions`, updated dependency names)
- Updated entrypoint pattern with `Address`/`AccountView`
- Updated account validation description with `Result<&AccountView>`
- Updated package names list for changesets

## 0.2.0 (2025-12-13)

### Breaking Changes

- Increase rust MSRV to `1.86.0` and `edition` to `2024`.

### Fixes

- Ensure `pinocchio_log::logger::Logger` export is behind `logs` feature flag.

## 0.1.2 (2025-11-08)

### Documentation

- Update published readme contents

## 0.1.1 (2025-11-08)

### Fixes

- Tidy unused code and uncaptured errors

## 0.1.0 (2025-11-08)

### Breaking Changes

#### Initial release

The initial release of the `pina` libraries.
