# Changelog

All notable changes to this project will be documented in this file.

## 0.3.1 (2026-02-25)

### Features

#### Extract POD primitive wrappers into `pina_pod_primitives` crate

Extract POD primitive wrappers into a new publishable `pina_pod_primitives` crate and re-export them from `pina` to preserve API compatibility.

Move `pina_codama_renderer` into `crates/`, update generated Rust clients to depend on `pina_pod_primitives`, reuse instruction docs in rendered output, and remove embedded shared primitive modules.

Add `pina codama generate` for end-to-end Codama IDL/Rust/JS generation with example filtering and configurable JS renderer command.

Expand Codama verification to all examples, move the pnpm workspace to repository root, add CLI snapshot tests with `insta-cmd`, and enforce deterministic regeneration checks for IDLs and generated clients.

#### `PodBool::is_canonical()` validation

Added `PodBool::is_canonical()` method to detect non-canonical boolean values (2-255) that pass `bytemuck` deserialization but fail `PartialEq` comparison against canonical `PodBool(0)` or `PodBool(1)`. Programs should call `is_canonical()` at deserialization boundaries to validate account data integrity.

#### Codama SDK integration tests for `pina_cli`

Generate both Rust and JavaScript clients from pina-generated IDLs using the codama SDK renderers, and verify that the generated code compiles correctly. The test pipeline covers all four example programs (counter_program, escrow_program, hello_solana, transfer_sol) and validates:

- IDL parsing by the codama SDK
- Rust client code generation and compilation (`cargo check`)
- JavaScript client code generation and TypeScript type-checking (`tsc --noEmit`)

#### `pina init` scaffold command

Added a new `pina init` command to scaffold a starter Pina program project:

- Creates a new project directory (default `./<name>`) with `Cargo.toml`, `src/lib.rs`, `README.md`, and `.gitignore`.
- Provides a `--path` option to control destination.
- Provides a `--force` option to overwrite scaffold files when they already exist.

The generated project includes a minimal no-std Pina program skeleton with entrypoint wiring and an `Initialize` instruction.

### Fixes

#### Release and publishing pipeline hardening

- Added a `docs-pages` GitHub Actions workflow that builds mdBook docs and deploys them to GitHub Pages on each published release.
- Tightened CI defaults by reducing workflow permissions to read-only where write access is not required.
- Updated CI test coverage to run `cargo test --all-features --locked` for closer release parity.
- Updated the pinned `knope` tool version to `0.22.3` so `knope` commands validate and run reliably in this toolchain.

#### No-logs build hardening

- Gate `core::panic::Location` behind the `logs` feature and explicitly mark assertion messages as used in non-logs builds so `pina` compiles cleanly in no-logs paths (including Surfpool smoke builds).
- Move `ignore_conventional_commits` from `PrepareRelease` to the `[changes]` section in `knope.toml` to match current `knope` configuration expectations.

#### Documentation and release-quality updates

- Standardized crate README badges to explicitly show crates.io and docs.rs links with current versions.
- Added a dedicated `pina_sdk_ids` crate README with crates.io/docs.rs badges and switched the crate manifest to use it.
- Added workspace coverage tooling with `coverage:all` and a CI `coverage` workflow that produces an LCOV artifact and uploads to Codecov.

#### Security hardening with additional example/test coverage

- Added `cargo-deny` and `cargo-audit` tooling plus `security:deny`, `security:audit`, and `verify:security` commands.
- Added a CI security job and a dependency policy (`deny.toml`) for license/source/dependency-ban enforcement.
- Hardened release workflows by validating `pina_cli` release tags against `crates/pina_cli/Cargo.toml` and scoping binary builds to the `pina_cli` package.
- Expanded docs publishing triggers to include docs changes on `main` and added docs verification in the Pages workflow.
- Added a new `todo_program` example, generated Codama IDL output, and Rust snapshot tests to keep generated IDLs aligned with committed `codama/idls/*.json` artifacts.

### Notes

#### Surfpool-based IDL smoke coverage

- Add dedicated `readme.md` files for each `examples/anchor_*` crate documenting intent and key differences from Anchor.
- Update each Anchor example crate manifest to point its `readme` field at the local example README.
- Strengthen IDL verification checks to assert discriminator metadata is present for generated anchor instructions/accounts.
- Add a Surfpool smoke test script that patches a test program ID, generates IDL, deploys the compiled program to Surfpool, and invokes it using generated IDL discriminator metadata.
- Add a dedicated `surfpool` GitHub Actions workflow for these longer-running deployment/invocation checks.

#### Migrate `examples/pinocchio_bpf_starter` to `examples/pina_bpf`

- Replace the starter implementation with `declare_id!`, `#[discriminator]`, `#[instruction]`, `parse_instruction`, and `nostd_entrypoint!`.
- Add a dedicated README for the example with explicit nightly build instructions using `-Z build-std=core,alloc`.
- Update workspace wiring (`Cargo.toml`, cargo aliases, docs, and CI scripts) to use `pina_bpf`.
- Add additional host tests and ignored BPF artifact verification tests, and run those artifact checks in `test:anchor-parity`.

### Documentation

- Add dedicated `readme.md` files to all example program directories with focused coverage notes and local run commands.
- Fix markdown JS snippet import ordering so `dprint` formatting checks pass in CI.
- Refresh all crate READMEs with up-to-date runtime features, feature flags, installation guidance, and usage examples.
- Added 50+ new tests across `pina` and `pina_pod_primitives` covering parse_instruction, error codes, PDA functions, Pod types, PodBool canonical validation, AccountDeserialize, discriminator roundtrips, and lamport helper edge cases.
- Updated book chapters to use mdt shared blocks for codama workflow commands, release workflow commands, and feature flags table.
- Use `linePrefix:"/// ":true` in all mdt consumer blocks for blank-line doc comment continuity.
- Improved API and developer documentation coverage for `pina` and `pina_sdk_ids` crates with reusable `mdt` template snippets.

## 0.3.0 (2026-02-20)

### Breaking Changes

#### Migrate to pinocchio 0.10.x

- **`AccountInfo` -> `AccountView`** -- all trait signatures, implementations, and generated code now use `AccountView` from pinocchio 0.10.x.
- **`Pubkey` -> `Address`** -- the 32-byte public key type is now `Address` from `solana-address` (with `bytemuck` feature for `Pod`/`Zeroable`/`Copy` derives).
- **`pinocchio-pubkey` -> `solana-address`** -- replaced the `pinocchio-pubkey` dependency with `solana-address ^2.0` for `declare_id!`, `address!`, and address constants.
- **`pinocchio-log` -> `solana-program-log`** -- replaced `pinocchio-log` with `solana-program-log ^1.1` for on-chain logging.
- **Method renames** -- `key()` -> `address()`, `try_borrow_data()` -> `try_borrow()`, `try_borrow_mut_data()` -> `try_borrow_mut()`, `realloc()` -> `resize()`, `data_is_empty()` -> `is_data_empty()`, `minimum_balance()` -> `try_minimum_balance()`.
- **Module renames** -- `pinocchio::pubkey` -> `pinocchio::address`, `pinocchio::account_info` -> `pinocchio::account`, `pinocchio::program_error` -> `pinocchio::error`.
- **`owner()` is now unsafe** -- `AccountView::owner()` requires an `unsafe` block.
- **PDA functions cfg-gated** -- `try_find_program_address`, `find_program_address`, and `create_program_address` are now behind `#[cfg(target_os = "solana")]`.
- **`assert_writable()` error change** -- now returns `ProgramError::InvalidAccountData` instead of `ProgramError::MissingRequiredSignature`.
- **`combine_seeds_with_bump` returns Result** -- now returns `Result<[Seed; MAX_SEEDS], ProgramError>` instead of panicking.
- **Removed `Loggable` trait** -- no implementations existed; trait was dead code.

#### `pina_macros` updates for pinocchio 0.10.x

- `#[derive(Accounts)]` generates `&'a AccountView` references and `TryFromAccountInfos` using `AccountView`.
- Doc examples updated to reference `Address` instead of `Pubkey`.

#### `pina_sdk_ids` migrated to `solana-address`

- `pinocchio_pubkey::declare_id!` -> `solana_address::declare_id!` for all 27 program and sysvar ID declarations.

#### Removed `pina_token_2022_extensions` crate

The upstream `pinocchio-token-2022` crate is adding native extension parsing support, making this crate redundant.

### Features

#### Custom dylint lint rules

Add three custom dylint lint rules to catch common Solana security mistakes at compile time:

- `require_owner_before_token_cast`: Warns when token cast methods are called without a preceding `assert_owner()`.
- `require_empty_before_init`: Warns when `create_program_account()` is called without a preceding `assert_empty()`.
- `require_program_check_before_cpi`: Warns when `.invoke()` or `.invoke_signed()` is called without program address verification.

#### Security and robustness improvements

**Critical fixes:**

- `discriminator_from_bytes` returns `Err` instead of panicking when input is shorter than discriminator size.
- `matches_discriminator` returns `false` instead of panicking on short input.
- `as_account` and `as_account_mut` check `data_len()` before creating raw-parts slices.
- `parse_instruction` validates data length before calling `discriminator_from_bytes`.

**Security improvements:**

- `close_account` now zeroes account data via `resize(0)` before closing.
- Added checked token cast methods: `as_checked_token_mint()`, `as_checked_token_account()`, `as_checked_token_2022_mint()`, `as_checked_token_2022_account()`.
- Deprecated `find_program_address` in favor of `try_find_program_address` which returns `Option` instead of panicking.

**New error variants:** `DataTooShort`, `InvalidAccountSize`, `InvalidTokenOwner`, `SeedsTooMany`.

**New Pod types:** `PodI32`, `PodI128`.

### Fixes

- `close_with_recipient()` uses `checked_add` for lamport arithmetic instead of unchecked addition.
- Fixed `write_discriminator` to correctly slice the destination buffer to `Self::BYTES` before copying.
- Replaced duplicate `AccountValidation` trait implementations for SPL token types with a single `impl_account_validation!` macro.
- Fixed inverted logic in `assert_mut()` method generated by the `#[account]` macro.
- Fixed `assert_seeds()` inverted logic -- previously returned `Ok` when the account key did _not_ match the derived PDA.
- Fixed `send()` never writing back lamports -- results were computed but discarded.
- Fixed typo "recipent" to "recipient" in `send()` error log.
- Enabled `solana-address` `curve25519` feature for PDA helpers in host builds.
- Added missing `AccountValidation` implementations for token mint/account types.
- Hardened `IntoDiscriminator` primitive implementations for short input slices.
- Lamport send/close helpers now reject same-account recipients and enforce writable preconditions.

### Notes

#### Example programs

Add three example programs ported from `solana-developers/program-examples`:

- **`hello_solana`** -- Minimal program showing basic pina structure.
- **`counter_program`** -- PDA-based account state management.
- **`transfer_sol`** -- Two SOL transfer methods: CPI and direct lamport manipulation.

#### Anchor parity examples

Added sequential Anchor parity examples: `anchor_declare_id`, `anchor_declare_program`, `anchor_duplicate_mutable_accounts`, `anchor_errors`, `anchor_events`, `anchor_floats`, `anchor_system_accounts`, `anchor_sysvars`, `anchor_realloc`. Extended `examples/escrow_program` with parity-focused tests.

Added Codama IDL fixtures for all `anchor_*` example programs and IDL verification tests.

### Documentation

- Add comprehensive security guide with 11 sealevel-attacks categories.
- Added comprehensive documentation to core traits and modules.
- Added crate-level doc comments to `pina`, `pina_sdk_ids`, and escrow example.
- Documented all public traits and their methods.
- Added `SAFETY`, `SECURITY`, and `TODO` comments throughout codebase.
- Rewritten `readme.md` with comprehensive documentation for pinocchio 0.10.x API.

#### `pina_cli` initial release

Add the `pina_cli` crate for automatic Codama IDL generation from Pina program source code. Provides both a library API (`generate_idl()`) and a CLI binary (`pina`) with subcommands for IDL generation. Add automated release workflow for pina CLI binary distribution across 9 target platforms.

## 0.2.0 (2025-12-13)

### Breaking Changes

- Increase Rust MSRV to `1.86.0` and `edition` to `2024`.

### Fixes

- Ensure `pinocchio_log::logger::Logger` export is behind `logs` feature flag.

## 0.1.1 (2025-11-08)

### Fixes

- Tidy unused code and uncaptured errors.
- Add crate descriptions for publishing.

## 0.1.0 (2025-11-08)

### Breaking Changes

#### Initial release

The initial release of the `pina` libraries: `pina`, `pina_macros`, `pina_sdk_ids`.
