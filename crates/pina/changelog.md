## 0.3.1 (2026-02-25)

### Features

#### Extract POD primitive wrappers into a new publishable `pina_pod_primitives` crate and re-export them from `pina` to preserve API compatibility.

Move `pina_codama_renderer` into `crates/`, update generated Rust clients to depend on `pina_pod_primitives`, reuse instruction docs in rendered output, and remove embedded shared primitive modules.

Add `pina codama generate` for end-to-end Codama IDL/Rust/JS generation with example filtering and configurable JS renderer command.

Expand Codama verification to all examples, move the pnpm workspace to repository root, add CLI snapshot tests with `insta-cmd`, and enforce deterministic regeneration checks for IDLs and generated clients.

### Fixes

#### Release and publishing pipeline hardening updates:

- Added a `docs-pages` GitHub Actions workflow that builds mdBook docs and deploys them to GitHub Pages on each published release.
- Tightened CI defaults by reducing workflow permissions to read-only where write access is not required.
- Updated CI test coverage to run `cargo test --all-features --locked` for closer release parity.
- Updated the pinned `knope` tool version to `0.22.3` so `knope` commands validate and run reliably in this toolchain.

#### Harden no-logs builds and release workflow compatibility.

- Gate `core::panic::Location` behind the `logs` feature and explicitly mark assertion messages as used in non-logs builds so `pina` compiles cleanly in no-logs paths (including Surfpool smoke builds).
- Move `ignore_conventional_commits` from `PrepareRelease` to the `[changes]` section in `knope.toml` to match current `knope` configuration expectations.

#### Documentation and release-quality updates across crates:

- Standardized crate README badges to explicitly show crates.io and docs.rs links with current versions.
- Added a dedicated `pina_sdk_ids` crate README with crates.io/docs.rs badges and switched the crate manifest to use it.
- Added workspace coverage tooling with `coverage:all` and a CI `coverage` workflow that produces an LCOV artifact and uploads to Codecov.

#### Improved release and security hardening with additional example/test coverage:

- Added `cargo-deny` and `cargo-audit` tooling plus `security:deny`, `security:audit`, and `verify:security` commands.
- Added a CI security job and a dependency policy (`deny.toml`) for license/source/dependency-ban enforcement.
- Hardened release workflows by validating `pina_cli` release tags against `crates/pina_cli/Cargo.toml` and scoping binary builds to the `pina_cli` package.
- Expanded docs publishing triggers to include docs changes on `main` and added docs verification in the Pages workflow.
- Added a new `todo_program` example, generated Codama IDL output, and Rust snapshot tests to keep generated IDLs aligned with committed `codama/idls/*.json` artifacts.

### Notes

#### Expand Anchor parity documentation and add Surfpool-based IDL smoke coverage.

- Add dedicated `readme.md` files for each `examples/anchor_*` crate documenting intent and key differences from Anchor.
- Update each Anchor example crate manifest to point its `readme` field at the local example README.
- Strengthen IDL verification checks to assert discriminator metadata is present for generated anchor instructions/accounts.
- Add a Surfpool smoke test script that patches a test program ID, generates IDL, deploys the compiled program to Surfpool, and invokes it using generated IDL discriminator metadata.
- Add a dedicated `surfpool` GitHub Actions workflow for these longer-running deployment/invocation checks.
- Update pinned Surfpool binary from `v0.12.0` to `v1.0.1` in `.eget/.eget.toml`.
- Update pinned Agave release from `v3.0.12` to `v3.1.8` so `cargo-build-sbf` can build workspace edition-2024 programs for Surfpool smoke tests.

#### Migrate `examples/pinocchio_bpf_starter` to `examples/pina_bpf` and convert the program to the `pina` API surface.

- Replace the starter implementation with `declare_id!`, `#[discriminator]`, `#[instruction]`, `parse_instruction`, and `nostd_entrypoint!`.
- Add a dedicated README for the example with explicit nightly build instructions using `-Z build-std=core,alloc`.
- Update workspace wiring (`Cargo.toml`, cargo aliases, docs, and CI scripts) to use `pina_bpf`.
- Add additional host tests and ignored BPF artifact verification tests, and run those artifact checks in `test:anchor-parity`.

#### Added new example coverage and upstream BPF tooling updates:

- Added `examples/pinocchio_bpf_starter` based on the upstream `pinocchio-bpf-starter` template pattern.
- Added sequential Anchor parity examples:
  - `examples/anchor_declare_id`
  - `examples/anchor_declare_program`
  - `examples/anchor_duplicate_mutable_accounts`
  - `examples/anchor_errors`
  - `examples/anchor_events`
  - `examples/anchor_floats`
  - `examples/anchor_system_accounts`
  - `examples/anchor_sysvars`
  - `examples/anchor_realloc`
- Extended `examples/escrow_program` with parity-focused tests aligned with Anchor's escrow coverage.
- Updated `sbpf-linker` in `[workspace.metadata.bin]` to `0.1.8`.
- Added a `build-bpf` cargo alias for the starter example and documented Anchor porting progress in the mdBook docs, including explicit notes for suites that are Anchor-CLI-specific.
- Added Codama IDL fixtures for all `anchor_*` example programs under `codama/idls/` and new Rust/JS IDL verification tests (`test:idl`) that run in CI.

### Documentation

- Add dedicated `readme.md` files to all example program directories with focused coverage notes and local run commands (`cargo test`, `pina idl`, and optional SBF build commands).
- Fix markdown JS snippet import ordering so `dprint` formatting checks pass in CI.
- Refresh the `pina` crate README with up-to-date runtime features, feature flags, installation guidance, a minimal program skeleton, and Codama workflow pointers.

#### Added `PodBool::is_canonical()` method to detect non-canonical boolean values (2–255) that pass `bytemuck` deserialization but fail `PartialEq` comparison against canonical `PodBool(0)` or `PodBool(1)`. Programs should call `is_canonical()` at deserialization boundaries to validate account data integrity.

Added badges (crates.io, docs.rs, CI, license, codecov) to `pina_pod_primitives` readme and root workspace readme. Created readme for `pina_codama_renderer` crate.

Added 50+ new tests across pina and pina_pod_primitives covering:

- `parse_instruction` (valid/invalid discriminators, wrong program ID, empty data, error remapping)
- `PinaProgramError` error codes (correct discriminants, reserved range, uniqueness)
- `assert` function (true/false conditions, custom error types)
- PDA functions (determinism, seed variations, roundtrip, wrong bump)
- Pod types (boundary values, endianness, bytemuck deserialization, defaults)
- PodBool canonical validation (non-canonical equality mismatch detection)
- AccountDeserialize trait (field preservation, mutable modification, wrong offset)
- Discriminator write/read roundtrips for all primitive sizes
- Lamport helper edge cases (exact balance, zero transfer, max values)

Updated book chapters to use mdt shared blocks for codama workflow commands, release workflow commands, and feature flags table. Added three new mdt providers (`codamaWorkflowCommands`, `releaseWorkflowCommands`, `pinaFeatureFlags`) to `template.t.md`.

#### Use `linePrefix:"/// ":true` in all mdt consumer blocks so the `///` prefix is applied to every line, including blank lines. Previously, blank lines within mdt-generated doc comments were left empty, which broke the `///` doc-comment continuity.

Upgraded `mdt_cli` from `0.0.1` (git) to `0.4.0` (crates.io) to gain the `:true` second argument for the `linePrefix` transformer.

Set `wrap_comments = false` in `rustfmt.toml` to prevent rustfmt from reflowing mdt-generated comment lines and splitting HTML closing tags across multiple lines.

Removed unused provider blocks (`pinaPodEndianContract`, `pinaSdkIdModuleContract`) and stale "Generated by mdt." header lines from `api-docs.t.md`.

#### Improve API and developer documentation coverage for the `pina` and `pina_sdk_ids` crates.

- Added reusable `mdt` template snippets for public API contracts and command examples.
- Expanded inline rustdoc across CPI, PDA, validation traits, utilities, and pod primitives.
- Added module-level and per-module ID documentation for `pina_sdk_ids`.
- Updated docs references to document the reusable template workflow (`template.t.md`, `api-docs.t.md`).

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
