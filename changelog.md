# Changelog

All notable changes to this project will be documented in this file.

## 0.8.0 (2026-03-30)

### Breaking Changes

#### Codebase quality improvements:

- Fix cu_benchmarks test crash by checking for SBF binary before loading mollusk
- Mark `typed_builder` re-export as `#[doc(hidden)]` non-stable API
- Add 11 tests for `pina_cli` error type Display impls
- Add `cargo doc` API docs check to `verify:docs` CI
- Rename `loaders.rs` → `impls.rs` for clarity
- Improve SAFETY documentation for all unsafe blocks in impls.rs

### Features

#### Add multi-file module resolution to the IDL parser.

`parse_program()` now follows `mod` declarations from `src/lib.rs` to discover and parse additional source files. This enables IDL generation for programs that split code across multiple modules (e.g. `src/state.rs`, `src/instructions/mod.rs`).

New module: `crates/pina_cli/src/parse/module_resolver.rs` with 5 unit tests covering single-file crates, child modules, `mod.rs` style, missing modules, and inline modules.

The existing `assemble_program_ir()` function is preserved for backward compatibility and now delegates to the new `assemble_program_ir_multi()`.

#### Implement opcode-aware CU cost model for the static profiler.

The profiler now decodes each 8-byte SBF instruction's opcode and assigns costs based on the instruction class:

- Regular instructions (ALU, memory, branch): 1 CU each
- Syscall instructions (`call imm` with `src_reg=0`): 100 CU each

Per-function profiles now include `syscall_count` and the text output shows a Syscall column. The JSON output includes `total_syscalls` and per-function `syscall_count`.

This replaces the previous flat 1-CU-per-instruction model which could underestimate programs with heavy syscall usage by 10-100x.

### Fixes

#### Comprehensive documentation update across workspace.

New mdt providers (template.t.md):

- `pinaCliCommands` — CLI command reference table
- `pinaIntrospectionDescription` — introspection module overview
- `pinaProfileDescription` — static CU profiler overview

Updated documentation:

- `docs/src/crates-and-features.md` — added `pina_profile`, CLI commands table, multi-file parser note, pod arithmetic, codama renderer module structure
- `docs/src/core-concepts.md` — added Pod types table, arithmetic description, introspection section; fixed stale `loaders.rs` → `impls.rs` reference
- `readme.md` — added Pod arithmetic examples, static CU profiler section, replaced outdated 3-crate table with full workspace packages table
- `crates/pina_cli/readme.md` — added `pina profile` command, multi-file note
- Fixed missing `CU_PER_INSTRUCTION` import in profiler tests

mdt provider/consumer counts: 23/46 → 26/56.

#### Add 12 integration tests for `pina::introspection` module (previously 0% coverage).

Tests construct fake Instructions sysvar account data following the exact binary layout that pinocchio's `Instructions` parser expects, then exercise each introspection function end-to-end:

- `get_instruction_count`: single and multiple instructions
- `get_current_instruction_index`: correct index returned
- `assert_no_cpi`: passes for top-level, fails for CPI, checks correct index
- `has_instruction_before`: finds earlier programs, returns false when first
- `has_instruction_after`: finds later programs, returns false when last
- Instructions with account metas and data
- Wrong sysvar address rejection

#### Refactor `pina_codama_renderer`: split monolithic `lib.rs` into focused render modules.

- `render/helpers.rs` — string utilities, docs rendering, numeric casts
- `render/discriminator.rs` — discriminator type/value resolution
- `render/types.rs` — POD type rendering and defined-type pages
- `render/accounts.rs` — account struct, PDA helpers, accounts mod
- `render/instructions.rs` — instruction struct, account metas, data struct
- `render/seeds.rs` — variable and constant PDA seed expression rendering
- `render/errors.rs` — error enum pages and errors mod
- `render/scaffold.rs` — crate scaffold creation and file writing
- `render/mods.rs` — root mod and programs mod rendering

`lib.rs` retains only the public API (`RenderConfig`, `read_root_node`, `render_idl_file`, `render_root_node`, `render_program`) and the orchestrator `render_program_to_files`.

All 13 existing tests continue to pass.

## 0.7.0 (2026-03-29)

### Breaking Changes

#### Pod Arithmetic (pina_pod_primitives)

Add full Quasar-style arithmetic, bitwise, ordering, and display traits to all Pod integer types (`PodU16`, `PodU32`, `PodU64`, `PodU128`, `PodI16`, `PodI32`, `PodI64`, `PodI128`).

**Arithmetic operators** (`Add`, `Sub`, `Mul`, `Div`, `Rem`) work between Pod types and between Pod + native types. Assign variants (`AddAssign`, `SubAssign`, etc.) allow ergonomic in-place mutation like `my_account.count += 1u64;`.

**Arithmetic semantics**: debug builds panic on overflow (checked), release builds use wrapping for CU efficiency on Solana.

**Bitwise operators**: `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Not` with assign variants.

**Signed types** get `Neg` for unary negation.

**Checked arithmetic**: `checked_add`, `checked_sub`, `checked_mul`, `checked_div` return `Option` for explicit overflow detection.

**Saturating arithmetic**: `saturating_add`, `saturating_sub`, `saturating_mul` clamp at bounds.

**Constants**: `ZERO`, `MIN`, `MAX` for all types.

**Helpers**: `get()` method, `is_zero()`, improved `Debug` (e.g. `PodU64(42)`), `Display`, `Ord`, `PartialOrd`, `PartialEq<native>`, `PartialOrd<native>`.

**PodBool**: `Not` operator and `Display` added.

**Backward compatible**: all existing APIs preserved, no breaking changes.

#### IDL Parser Hardening (pina_cli)

Add static validation to the IDL parser that runs after IR assembly:

- **Discriminator collision detection**: checks within accounts and within instructions for duplicate discriminator values. Three-way collisions produce all pairwise diagnostics.
- **Duplicate input field detection**: checks within each instruction for name collisions between account names and argument names.
- **Human-readable error formatting** for both collision types.

Validation is automatically run during `assemble_program_ir()`.

#### Static CU Profiler (pina profile)

Add a new `pina profile` CLI command for static compute unit profiling of compiled SBF programs.

- `pina profile <path-to-so>` — text summary with per-function CU estimates
- `pina profile <path-to-so> --json` — JSON output for CI integration
- `pina profile <path-to-so> --output report.json` — write to file

The profiler parses ELF binaries to extract `.text` sections and symbol tables, counts SBF instructions per function, and estimates CU costs without requiring a running validator. Works best with unstripped binaries.

v1 scope: text/JSON output, per-function breakdown, best-effort symbol resolution. Flamegraph/browser UI planned for v2.

### Features

- Add `realloc_account` and `realloc_account_zero` CPI helpers for safe account reallocation with automatic rent recalculation.
- Add instruction introspection helpers for reading the sysvar instructions account: `assert_no_cpi`, `get_instruction_count`, `get_current_instruction_index`, `has_instruction_before`, and `has_instruction_after`.
- Add `pina init <name>` scaffolding command that generates a new Pina program project with a minimal program structure, tests, and build configuration.
- Add `InstructionBuilder` and account metadata helper functions (`writable_signer`, `writable`, `readonly_signer`, `readonly`) for typed client-side instruction construction.

### Fixes

- Expand snapshot test coverage for proc macros including edge cases for `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]`.
- Add property-based fuzz tests using `proptest` for Pod type deserialization, round-trip correctness, and discriminator parsing safety.
- Enforce the workspace's custom security dylints in CI, align `devenv` with the Node.js version selected by `pnpm-workspace.yaml` without a separate install step, and replace the `paste` dependency in `pina` with `pastey`.

#### Reduce warning noise and restore local Codama client verification ergonomics.

- Annotate the generated Rust root `mod.rs` re-export of `programs::*` with `#[allow(unused_imports)]`.
- Add regression coverage for the generated root module allowance.
- Update the repository Codama JS test harness to type-check generated clients against the current `@solana/kit` dependency layout using a local compatibility shim.

This keeps crate-internal program ID constants available at `crate::<PROGRAM>_ID` for generated instruction modules, while avoiding warnings for IDLs that only generate a `programs` module and keeping `pnpm run check:js` green.

#### Fix broken doc comments produced by `mdt` template expansion. The line-prefix mode was emitting `-->//` instead of `-->` followed by `///`, and blank lines inside reusable doc blocks were missing the `///` prefix. This caused rustdoc warnings and broken documentation rendering.

Also simplifies a raw string literal in `pina_cli` init templates and shortens a fully-qualified `std::result::Result::ok` path to `Result::ok`.

#### Add comprehensive end-to-end integration tests for the pina crate. The test suite covers:

- Full account lifecycle (create, write state, read/validate, update, close with rent return)
- Multi-instruction flows (Initialize then Update, verify state after each step)
- Error handling (invalid signer, wrong owner, discriminator mismatch, data length mismatch, invalid instruction discriminator, empty instruction data, wrong program ID, insufficient accounts, non-writable account, empty account rejection)
- Lamport transfer operations (send, insufficient funds, same-account rejection, close with recipient)
- PDA seed verification (derive and verify roundtrip, canonical bump assertion, assert_seeds_with_bump on AccountView)
- AccountView validation chains (chained assertions, short-circuit behavior)
- Discriminator dispatch across all instruction variants
- TryFromAccountInfos derive mapping and rejection of excess accounts
- Address assertion (single address and multi-address matching)

Tests use raw SVM input buffer construction to create AccountView instances without requiring compiled BPF programs, following the same memory layout as the pinocchio entrypoint deserializer.

#### Expand mdt documentation reuse across the workspace.

Added 10 new mdt provider blocks in `template.t.md`:

- `pinaProjectDescription` — single-source project tagline
- `pinaInstallation` — cargo add instructions
- `podTypesTable` — Pod types reference table
- `podArithmeticDescription` — Pod arithmetic semantics
- `pinaWorkspacePackages` — workspace crate table
- `pinaFeatureHighlights` — feature bullet list
- `sbfBuildInstructions` — SBF build commands
- `pinaTestingInstructions` — testing commands
- `pinaBadgeLinks` — shared badge link references
- `pinaSecurityBestPractices` — security checklist

Wired 15 new consumers across:

- `readme.md` (root) — 10 consumer blocks
- `crates/pina/readme.md` — feature flags table + badge links
- `crates/pina_pod_primitives/readme.md` — pod types table + arithmetic description
- `docs/src/security-model.md` — security best practices

Provider/consumer counts: 13/31 → 23/46.

#### Improve diagnostics and validation ergonomics in `pina`.

- `parse_instruction` still remaps `ProgramError::Custom(_)` discriminator errors to `InvalidInstructionData` for compatibility, but now logs the original custom error code when the `logs` feature is enabled.
- The escrow example now adds stronger account checks in both `Make` and `Take` flows, including explicit system program ID validation, vault owner validation, and associated-token-address validation for `taker_ata_b` and `maker_ata_b` before CPIs.
- Regenerated escrow Codama IDL and generated clients to reflect account metadata changes (default `systemProgram` and writable ATA fields where required).
- Clean up internal test assertions in `traits.rs` to avoid unnecessary qualification warnings.
- Make `PinaProgramError` independent from proc-macro expansion so `pina` now compiles with `--no-default-features` (without requiring the `derive` feature), and add regression coverage to keep the enum wire-size aligned to `u32`.
- Add a dedicated CI/devenv build gate (`build:pina:no-default`) to continuously verify `pina` no-default feature compatibility across key feature subsets.

#### Update the committed `escrow_program` CLI example snapshot to match the regenerated Codama IDL.

This keeps the `pina_cli` example snapshot tests aligned with the generated escrow metadata after account validation changes introduced writable ATA requirements and default `systemProgram` addresses.

### Notes

- Add compute unit benchmark tests measuring CU consumption for key Pina operations including account validation, PDA derivation, and CPI helpers.
- Document the workspace tooling refresh that updates the Codama JavaScript dependencies, adds `useNodeVersion` to `pnpm-workspace.yaml`, and makes `devenv` honor that pnpm workspace Node version via shell-local `node`/`npm`/`npx`/`corepack` shims while keeping the standalone `pnpm` binary active.

#### Harden CI setup reliability by adding retries to the shared `./.github/actions/devenv` action for transient Nix/devenv failures.

Also increase workflow timeouts for `release-preview`, `semver`, and `binary-size` so slow cold-cache environment provisioning does not cancel jobs before they execute their main steps.

#### Re-enable the anchor parity BPF artifact checks in CI by building `sbpf-linker` with the Blueshift `upstream-gallery-21` LLVM toolchain.

This adds a cached `install:sbpf-gallery` devenv script and restores `cargo build-bpf` plus the ignored `pina_bpf` `bpf_build_` tests in `test:anchor-parity`.

## 0.6.0 (2026-02-26)

### Breaking Changes

#### Unify all crate and package versions under a single `[workspace.package] version` field. All publishable crates (`pina`, `pina_macros`, `pina_pod_primitives`, `pina_sdk_ids`, `pina_cli`, `pina_codama_renderer`) and the `codama-nodes-from-pina` JS package now share the same version, managed by a single `[package]` entry in `knope.toml`. This replaces the previous per-crate `[packages.*]` configuration and ensures all crates are released together with a single version bump.

Simplify the assets workflow to match the new unified release tag format and remove the per-crate version validation step. Update tooling versions for `cargo-llvm-cov`, `cargo-nextest`, `cargo-semver-checks`, and `mdt_cli`. Switch publishing from `cargo-workspaces` to `cargo publish --workspace`.

### Notes

- Add a `binary-size` CI workflow that builds SBF programs and reports their binary sizes in the GitHub Actions job summary for pull requests.
- Remove `mdt` from `cargo-run-bin` management (`[workspace.metadata.bin]`) and the devenv script wrapper. `mdt` is now provided directly as a nix package from `ifiokjr-nixpkgs`.
- Use `pnpm-standalone` from `ifiokjr/nixpkgs` on all platforms after the upstream Linux fix (ifiokjr/nixpkgs#4), removing the macOS-only conditional.
- Add a `release-preview` CI workflow that runs `knope release --dry-run` on pull requests and outputs a summary of pending version bumps and changelog entries.
- Remove `knope` from `cargo-run-bin` management (`[workspace.metadata.bin]`) and the devenv script wrapper. `knope` is now provided directly as a nix package from `ifiokjr-nixpkgs`.
- Harden the rustup nix override to fix intermittent CI failures caused by rustup 1.28+ requiring a `version` field in `settings.toml` during shell completion generation in the install phase.
- Add a CI workflow that runs `cargo semver-checks` on pull requests to detect accidental semver violations before merge.

### Documentation

- Add comprehensive doc comments with examples to public API items in the `pina` crate.
- Add a parity tracking document for `pina_codama_renderer` listing supported and unsupported Codama node types.
- Add tutorial chapters to the mdBook: "Your First Program", "Token Escrow Tutorial", and "Migrating from Anchor".
- Add `<br>` tags after h1-h3 headings in all sub-crate, example, security, and lint readme files for improved visual spacing.

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
