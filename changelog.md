# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Breaking Changes

- Upgrade the workspace to the Pinocchio 0.11 crate family (`pinocchio` 0.11, `pinocchio-system` 0.6, `pinocchio-token` 0.6, `pinocchio-token-2022` 0.3, `pinocchio-memo` 0.4, and `pinocchio-associated-token-account` 0.4).
- Raise the workspace `rust-version` baseline to `1.89.0` to match upstream requirements.
- Change the entrypoint and account parsing model from `&[AccountView]` to `&mut [AccountView]`.
- Change `ProcessAccountInfos::process` to consume `self` instead of borrowing `&self`.
- Change `AsAccount::as_account` / `as_account_mut` to return guard-backed `Ref<T>` / `RefMut<T>` values.
- Extend `#[derive(Accounts)]` to support `&'a mut AccountView` and `&'a mut [AccountView]`, and use mutable fields to infer writable accounts in generated IDLs.
- Split close and realloc behavior: `close_with_recipient()` no longer zeroes account data implicitly, and realloc helpers are gated behind the new `account-resize` feature.
- Replace Pina's local bytemuck primitive layer with zeropod's `ZcElem`, `ZcValidate`, `ZeroPodFixed`, and fixed-capacity collection model.
- Separate native zeropod schemas from their generated zero-copy storage views. Account loaders now return `TypeZc`, and storage fields use zeropod's accessor methods.
- Remove Pina's whole-object `to_bytes()`, `PinaSerialize`, generic `InstructionBuilder`, custom `PodEnum`, and generic pointer-cast helpers. Inactive string and vector capacity is no longer observable through Pina.
- Token loaders no longer project Token-2022 bytes into legacy SPL Token state. Multi-program callers receive a guard-backed enum that preserves the concrete upstream type and extension layout.

### Features

- Add a standalone `memo` feature that re-exports `pinocchio_memo` as `pina::memo`.
- Preserve `TokenAccount` compatibility aliases through `pina::token` and `pina::token_2022` wrapper modules.
- Infer writable Codama/IDL accounts from mutable `#[derive(Accounts)]` fields in `pina_cli`.

#### Re-export zeropod collections and validation

Pina re-exports zeropod's fixed-capacity `PodOption`, `PodString`, and `PodVec` types. `PinaAccount` and macro-generated instruction/event parsers recursively validate tags, prefixes, active elements, booleans, enums, and UTF-8 at the byte boundary before returning typed references.

New mdt providers:

- `podCollectionTypesTable` — collection types reference table
- `podCollectionDescription` — collection type semantics

Account, instruction, and event schemas derive `zeropod::ZeroPod`. Their `initialize` helpers zero caller-owned storage, write the discriminator, and return the validated generated view. Generated client instruction builders own their fully initialized buffers and do not expose schema object representations.

Generated JavaScript codecs reject over-capacity values rather than truncating them and validate discriminators, booleans, and UTF-8 using the same canonical rules as the on-chain zeropod views.

### Documentation

- Refresh tutorials, READMEs, API docs, and security guidance for the mutable-account parsing model.
- Document the explicit `zeroed()` then `close_with_recipient()` close flow.
- Regenerate Codama IDLs and committed Rust/JS clients for the updated writable-account inference.

## [0.10.0](https://github.com/pina-rs/pina/releases/tag/v0.10.0) (2026-08-22)

Grouped release for `core`.

### Breaking Changes

#### Enforce CPI signer and PDA target invariants

_Packages:_ _pina_

Make CPI signer metadata describe the called instruction instead of inheriting outer-instruction privileges. Generated-style builders can now request signer accounts explicitly, so both transaction signatures and `invoke_signed` PDA seeds satisfy the same typed CPI API without forwarding unrelated signatures.

Require `CpiContext` to receive a validated `Program<T>` and reject PDA account creation when the supplied account address does not match the requested seeds, bump, and owner. This prevents independently signing accounts from bypassing PDA derivation checks on both zero-balance and pre-funded allocation paths.

When migrating, replace the raw program address passed to `CpiContext::new` with `Program::<YourProgram>::new(program_account)?`, and add the target program marker as the context's program type parameter. This deliberately breaks the previous constructor so program-ID validation cannot be bypassed accidentally.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #219](https://github.com/pina-rs/pina/pull/219) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Add optional accounts with a fixed program-address filler

_Packages:_ _pina_, _pina_cli_, _pina_macros_

`#[derive(Accounts)]` now supports `Option<&'a AccountView>` and `Option<&'a mut AccountView>` fields. Account counts stay fixed: generated Codama clients fill an omitted optional slot with a readonly meta pointing at the executing program's address, and on-chain parsing maps any slot holding the program address back to `None`.

Breaking: `TryFromAccountInfos::try_from_account_infos` and the derived `TryFrom<(&Address, &mut [AccountView])>` now take the executing program id so optional slots can detect the filler sentinel. Entrypoint dispatch changes from `Accounts::try_from(accounts)?` to `Accounts::try_from((program_id, accounts))?`. The IDL pipeline emits `isOptional: true` plus the `programId` optional-account strategy for optional slots, and validation-chain analysis now attributes assertions written against `if let Some(alias)` bindings back to their originating fields.

Optional account parsing rejects non-`AccountView` references, keeps validation aliases within their Rust lexical scopes, and never assigns a default that would turn omission into presence. Generated JavaScript parsers compare filler slots against the instruction's effective program address, including custom deployment overrides. The example validates authority-bound PDAs and runs its Mollusk, LiteSVM, and Surfpool coverage against a required SBF artifact.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #226](https://github.com/pina-rs/pina/pull/226)

### Features

#### Improve PDA signer and CPI builder ergonomics

_Packages:_ _pina_, _pina_cli_, _pina_macros_

Adds a stack-backed `PdaSigner` helper, generated `#[pda]` seed conversions for Pinocchio CPI signing, and a validated `Program<T>` wrapper for generated CPI builders. The Prop AMM generated-style CPI prototype now exposes Pinocchio-style builders with `.invoke()` and `.invoke_signed()` methods behind an opt-in `cpi` feature.

Generated `pina init` programs now declare `default = []`, `bpf-entrypoint = []`, and `cpi = []`, and include a starter `src/cpi.rs` module that is only compiled when consumers explicitly enable `features = ["cpi"]`.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #216](https://github.com/pina-rs/pina/pull/216) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Add strict mutable trailing accounts

_Packages:_ _pina_, _pina_macros_

Add opt-in `#[pina(remaining, distinct)]` parsing for programs whose mutable trailing accounts must have unique addresses. Existing `#[pina(remaining)]` pass-through behavior remains unchanged for instructions that intentionally accept aliases.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #220](https://github.com/pina-rs/pina/pull/220) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Make Pina Tooling Self-Describing

_Packages:_ _pina_cli_, _pina_codama_nodes_, _pina_cli_npm_, _pina_cli_darwin_arm64_, _pina_cli_darwin_x64_, _pina_cli_freebsd_x64_, _pina_cli_linux_arm64_gnu_, _pina_cli_linux_arm64_musl_, _pina_cli_linux_x64_gnu_, _pina_cli_linux_x64_musl_, _pina_cli_win32_arm64_msvc_, _pina_cli_win32_x64_msvc_, _pina_skill_

Add comprehensive CLI help, stable machine-readable IDL output, bundled documentation discovery, and a complete mdBook CLI reference. Publish prebuilt CLI packages for every release target, rename the Codama helpers to `@pina-rs/codama-nodes`, and add the installable `@pina-rs/skill` agent guide. Split macro expansion code into focused modules without changing generated output.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #215](https://github.com/pina-rs/pina/pull/215) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

### Fixes

#### Remove Clippy warnings without changing public ergonomics

_Packages:_ _pina_, _pina_cli_, _pina_macros_

Clean up macro and code-generation helpers, remove dead test scaffolding, and pass Pina's pointer-sized `AccountView` handle by value inside private validators. Public account-validation and cursor APIs remain unchanged, and no account data is copied.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #213](https://github.com/pina-rs/pina/pull/213) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Make IDL extraction fail closed

_Packages:_ _pina_, _pina_cli_, _pina_macros_

Reject incomplete or ambiguous program source graphs instead of silently emitting partial Codama metadata. IDL generation now resolves explicit module paths, distinguishes missing conditional modules from missing required files, validates package names and entrypoint ownership, and requires PDA attributes and inferred PDA account links to resolve exactly.

Keep account and PDA naming on one canonical path, including structs named exactly `State`. Constant-only `#[pda]` declarations now generate valid typed seed helpers, allowing data-free signer PDAs to use the same `seeds().with_bump(...)` API and generated client defaults as state-bearing PDAs.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #221](https://github.com/pina-rs/pina/pull/221) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Enforce CPI signer and PDA target invariants

_Packages:_ _pina_cli_

Make CPI signer metadata describe the called instruction instead of inheriting outer-instruction privileges. Generated-style builders can now request signer accounts explicitly, so both transaction signatures and `invoke_signed` PDA seeds satisfy the same typed CPI API without forwarding unrelated signatures.

Require `CpiContext` to receive a validated `Program<T>` and reject PDA account creation when the supplied account address does not match the requested seeds, bump, and owner. This prevents independently signing accounts from bypassing PDA derivation checks on both zero-balance and pre-funded allocation paths.

When migrating, replace the raw program address passed to `CpiContext::new` with `Program::<YourProgram>::new(program_account)?`, and add the target program marker as the context's program type parameter. This deliberately breaks the previous constructor so program-ID validation cannot be bypassed accidentally.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #219](https://github.com/pina-rs/pina/pull/219) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Make Pina Tooling Self-Describing

_Packages:_ _pina_macros_

Add comprehensive CLI help, stable machine-readable IDL output, bundled documentation discovery, and a complete mdBook CLI reference. Publish prebuilt CLI packages for every release target, rename the Codama helpers to `@pina-rs/codama-nodes`, and add the installable `@pina-rs/skill` agent guide. Split macro expansion code into focused modules without changing generated output.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #215](https://github.com/pina-rs/pina/pull/215) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

### Documentation

#### Add the Pina logo

_Packages:_ _pina_, _pina_cli_, _pina_macros_, _pina_codama_renderer_, _pina_profile_, _pina_sdk_ids_, _pina_codama_nodes_, _pina_cli_npm_, _pina_skill_

Introduce the low-poly origami pineapple mark across the project: README and crate/npm readme headers, the mdBook intro page, and the book favicon. Canonical asset lives at `.github/assets/logo.png`.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #223](https://github.com/pina-rs/pina/pull/223) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Unslop and cross-check documentation

_Packages:_ _pina_, _pina_cli_, _pina_macros_, _pina_codama_renderer_, _pina_sdk_ids_

Polish and verify the documentation corpus:

- Correct the license badge to Apache-2.0 (the workspace license) across the root, crate, and CLI readmes, and fix the broken `license` link in the root readme.
- Re-sync the compiled-in `pina docs` templates with the mdt providers so the CLI serves current content (Dart clients, npm packages, feature-selection and instruction-authoring tips).
- Fix stale references: the nonexistent `test_utils_solana` module, a duplicate `crates/pina` entry and incomplete Pod type list in the workspace architecture guide, and a duplicate `crates/pina` heading in the crates-and-features book page.
- Tighten example readmes: cut filler ("built with Pina", "reference example"), correct inaccurate claims (escrow Token-2022 validation, staking ATA idempotency, prop-amm CPI builder naming, float bit-pattern storage, `#[event(discriminator)]`), add missing e2e-test notes, and align heading and code-fence style.
- Note the authoritative `security:dylint` task in the lints readme and drop duplicated renderer description text.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #218](https://github.com/pina-rs/pina/pull/218) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

#### Document example production boundaries

_Packages:_ _pina_

Mark the staking and vesting programs as account and bookkeeping scaffolds, state their intentionally missing economic behavior, and add a production-readiness gate for teams adapting Pina examples to asset-bearing programs.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #222](https://github.com/pina-rs/pina/pull/222) · _Related issues:_ [#214](https://github.com/pina-rs/pina/issues/214)

## [0.9.0](https://github.com/pina-rs/pina/releases/tag/v0.9.0) (2026-08-19)

Grouped release for `core`.

### Breaking Changes

#### Enforce writability at account parse time: `AccountsCursor::next_mut` now validates that the account is marked writable in the instruction before returning a `&mut AccountView`, and `remaining_mut` validates every trailing account. A `&mut AccountView` (or `&mut [AccountView]`) field is now the single source of truth for writable accounts — the separate `assert_writable()` call is no longer required for mutable fields.

_Packages:_ _pina_

##### Migration guide

- **Remove redundant `assert_writable()` calls.** Any `assert_writable()` invoked on a field declared as `&'a mut AccountView` (or inside a `&'a mut [AccountView]` remaining slice) is now redundant and can be deleted. The check happens once, during `try_from`/`try_from_account_infos`, before any instruction processing.
- **Keep `assert_writable()` on immutable fields.** Accounts declared as `&'a AccountView` that must be writable (for example, CPI targets the program never mutates directly) still require an explicit `assert_writable()` call.
- **`remaining_mut` now returns `Result`.** `AccountsCursor::remaining_mut` changed from `&'a mut [AccountView]` to `Result<&'a mut [AccountView], ProgramError>` and rejects the call when any remaining account is not writable. The `#[derive(Accounts)]` expansion was updated accordingly; manual cursor users must add `?`.
- **Behavior change.** Programs that previously declared `&mut` fields without asserting writability will now fail with `ProgramError::InvalidAccountData` when a non-writable account is passed. This is the intended fix: a mutable view of a non-writable account is never legitimate.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #172](https://github.com/pina-rs/pina/pull/172)

#### Bind Anchor Realloc Samples to Their Authority

_Packages:_ _core_

`examples/anchor_realloc` is now a secure, intentionally non-ABI-compatible adaptation of Anchor's test fixture. It adds `Initialize` (discriminator `2`) and creates a per-authority sample PDA at `[b"sample", authority]`. `Realloc` still uses discriminator `0`, but its authority must now be writable and a signer, its sample must be initialized at the canonical PDA, and its `len` includes the 34-byte authenticated `Sample` header.

`Realloc2` (discriminator `1`) no longer resizes two arbitrary accounts. It validates both authenticated targets and returns `AccountDuplicateReallocs` before mutation, matching Anchor's duplicate-reallocation regression intent.

This removes the previous capability for an unrelated signer to resize an arbitrary writable account owned by the program. Regenerate Codama clients and initialize the sample before calling `Realloc`.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #203](https://github.com/pina-rs/pina/pull/203) · _Related issues:_ [#205](https://github.com/pina-rs/pina/issues/205)

#### Refresh workspace dependencies

_Packages:_ _pina_, _pina_cli_

Refresh workspace dependencies to their latest compatible versions.

- Bump `pinocchio-token` to `0.7` and `pinocchio-token-2022` to `0.4` (drops the `token_program` field from `TransferChecked`/`CloseAccount`; examples now use the `new()` constructors).
- Bump `codama-nodes` to `0.11` (spec `1.8.0`), `mollusk-svm` to `0.15`, `solana-account` to `4`, `solana-system-interface` to `3`, `insta-cmd` to `0.7`, and `object` to `0.40`.
- Bump the JS Codama toolchain (`codama` to `1.10`, `@codama/renderers-js` to `2.3`) and regenerate all IDLs and Rust/JS clients.
- Upgrade the workspace to `syn` 3, including the public `pina_cli` parsing API, and release the temporary derive-crate pins used during the migration.
- Resolves `RUSTSEC-2026-0097` (unsound `rand` 0.7.3) and `RUSTSEC-2026-0173` (unmaintained `proc-macro-error2`), dropping them from the dependency tree, and updates `jiff`, `defmt`, `env_logger`, `solana-logger`, and `crossbeam-epoch` to patched versions.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #167](https://github.com/pina-rs/pina/pull/167) · _Related issues:_ [#165](https://github.com/pina-rs/pina/issues/165), [#166](https://github.com/pina-rs/pina/issues/166), [#167](https://github.com/pina-rs/pina/issues/167), [#168](https://github.com/pina-rs/pina/issues/168), [#169](https://github.com/pina-rs/pina/issues/169), [#170](https://github.com/pina-rs/pina/issues/170), [#171](https://github.com/pina-rs/pina/issues/171), [#172](https://github.com/pina-rs/pina/issues/172), [#173](https://github.com/pina-rs/pina/issues/173), [#174](https://github.com/pina-rs/pina/issues/174), [#175](https://github.com/pina-rs/pina/issues/175), [#176](https://github.com/pina-rs/pina/issues/176), [#177](https://github.com/pina-rs/pina/issues/177), [#179](https://github.com/pina-rs/pina/issues/179), [#180](https://github.com/pina-rs/pina/issues/180), [#181](https://github.com/pina-rs/pina/issues/181), [#185](https://github.com/pina-rs/pina/issues/185), [#186](https://github.com/pina-rs/pina/issues/186), [#187](https://github.com/pina-rs/pina/issues/187), [#189](https://github.com/pina-rs/pina/issues/189), [#195](https://github.com/pina-rs/pina/issues/195)

#### Upgrade workspace to Pinocchio 0.11

_Packages:_ _core_

Upgrade the workspace to Pinocchio 0.11 and migrate Pina's core account APIs to the new mutable `AccountView` model.

Breaking changes include:

- entrypoints, `TryFromAccountInfos`, and downstream account parsing now use `&mut [AccountView]`
- `ProcessAccountInfos::process` now consumes `self`
- `AsAccount::as_account` and `as_account_mut` now return guard-backed `Ref` / `RefMut` values instead of bare references
- `#[derive(Accounts)]` now supports mutable account refs and slices, and writable IDL inference now follows mutable fields
- close helpers no longer implicitly zero or resize account data; callers can keep the explicit `zeroed()` flow or use the new `close_account_zeroed()` helper when stale bytes must be cleared before close

This release also upgrades the Pinocchio companion crates, adds the standalone `memo` and `account-resize` features, preserves token account compatibility aliases, refreshes docs/examples/security guidance for the new borrow model, and regenerates the affected Codama IDLs and generated clients.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #149](https://github.com/pina-rs/pina/pull/149)

#### Port to zeropod

_Packages:_ _pina_, _pina_cli_, _pina_macros_, _pina_codama_renderer_

Replace pina's own pod primitives (`pina_pod_primitives`, based on `bytemuck`) with [zeropod](https://crates.io/crates/zeropod) as the primitives library. This is a major change to the account model:

- **`pina_pod_primitives` is deleted.** Its types (`PodU64`, `PodBool`, `PodString`, `PodVec`, `PodOption`) are re-exported from `zeropod` with the same names.
- **`bytemuck::Pod` and `Zeroable` are removed from Pina's public account model.** `pina::Pod` / `pina::Zeroable` re-exports are gone; transitive Solana dependencies may still use bytemuck internally.
- **`AccountDeserialize` is replaced by `PinaAccount`.** Account schemas derive `zeropod::ZeroPod`; loaders return the generated `AccountZc` companion rather than reinterpreting the native schema as account memory.
- **Content validation at the deserialization boundary.** Non-canonical `PodBool` bytes, invalid UTF-8 in `PodString`, and overlength `PodVec` prefixes are now rejected by `try_from_bytes` / `as_account` instead of silently accepted.
- **`PodString::as_str()` is now safe** (validated at the boundary); `as_str_unchecked` / `try_as_str` are gone.
- **`PodU64::from_primitive` is replaced by `From<u64>`** (zeropod's API). `from_primitive` callers should use `PodU64::from(n)` or `n.into()`.
- **`PodBool::from_bool` is replaced by `From<bool>`**.
- **`PodVec::as_mut_slice` is renamed to `as_slice_mut`** (zeropod's API).
- The `#[account]`, `#[instruction]`, and `#[event]` macros derive `zeropod::ZeroPod` and expose checked `try_from_bytes` / `initialize` helpers. Pina no longer generates `to_bytes()` or any whole-object serialization API.
- Macro-generated schemas use a closed field grammar: native integers and booleans, Pina's Pod scalar wrappers, the exact Pina `Address`, literal-length byte arrays, and native scalar options. Generics, custom/nested mappings, enums, `NonZero*`, `char`, raw `PodOption`, and string/vector collections are rejected at macro expansion.
- Runtime loaders return the generated zero-copy representation, whose audited scalar fields are read and changed with zeropod's `get` / `set` accessors. Bounded text and list examples use fully initialized byte arrays with checked semantic helpers.
- Direct zeropod derives and manual `PinaAccount` / `ZeroPodFixed` implementations remain advanced escape hatches outside Pina's audited macro-generated contract; their authors own all zeropod safety invariants.
- `PinaSerialize`, Pina's generic `InstructionBuilder`, the custom `PodEnum` derive, and Pina's generic raw-cast helpers are removed. Zeropod owns the byte-to-view boundary.
- The `#[discriminator]` macro no longer emits `unsafe impl Pod` / `unsafe impl Zeroable` for enums.
- Codama-generated clients validate zeropod fields before zero-copy access. Instruction construction owns a fully initialized buffer and never exposes a schema or storage view as raw bytes.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #195](https://github.com/pina-rs/pina/pull/195) · _Closed issues:_ [#193](https://github.com/pina-rs/pina/issues/193) · _Related issues:_ [#194](https://github.com/pina-rs/pina/issues/194), [#205](https://github.com/pina-rs/pina/issues/205)

#### Use zeropod's native enum schema support

_Packages:_ _pina_macros_

Remove Pina's custom `PodEnum` derive in favor of zeropod's standalone native enum schema support. Pina's audited `#[account]`, `#[instruction]`, and `#[event]` schemas reject enum and custom `ZcField` fields because the macro cannot establish their full mapping and validation-order invariants. Standalone enums can still derive `zeropod::ZeroPod` for advanced direct zeropod integrations outside that closed contract.

```rust
use pina::ZeroPod;

#[derive(ZeroPod)]
#[repr(u8)]
enum Color {
	Red = 0,
	Green = 1,
	Blue = 2,
}

let color = Color::from_bytes(&[1]);
```

For macro-generated Pina schemas, store an audited scalar discriminant and convert it to a domain enum only after explicit semantic validation.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #195](https://github.com/pina-rs/pina/pull/195) · _Closed issues:_ [#193](https://github.com/pina-rs/pina/issues/193) · _Related issues:_ [#194](https://github.com/pina-rs/pina/issues/194), [#205](https://github.com/pina-rs/pina/issues/205)

### Features

#### Add fuzz harness infrastructure for pina-rs targeting `PinaAccount::try_from_bytes` and `parse_instruction`.

_Packages:_ _pina_

- New `crates/pina_fuzz/` crate with `libfuzzer-sys` integration
- Fuzz targets for account deserialization of `CounterState`, `RegistryConfig`, and `RoleEntry`
- Fuzz targets for instruction parsing of `CounterInstruction` and `RegistryInstruction`
- Uses real workspace example programs (counter_program, role_registry_program) for authentic account/instruction types
- Compiles both standalone fuzz binaries during the normal CI test task so broken dependencies and entry points cannot silently merge

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #172](https://github.com/pina-rs/pina/pull/172) · _Related issues:_ [#165](https://github.com/pina-rs/pina/issues/165), [#166](https://github.com/pina-rs/pina/issues/166), [#167](https://github.com/pina-rs/pina/issues/167), [#168](https://github.com/pina-rs/pina/issues/168), [#169](https://github.com/pina-rs/pina/issues/169), [#170](https://github.com/pina-rs/pina/issues/170), [#171](https://github.com/pina-rs/pina/issues/171), [#172](https://github.com/pina-rs/pina/issues/172), [#173](https://github.com/pina-rs/pina/issues/173), [#174](https://github.com/pina-rs/pina/issues/174), [#175](https://github.com/pina-rs/pina/issues/175), [#176](https://github.com/pina-rs/pina/issues/176), [#177](https://github.com/pina-rs/pina/issues/177), [#179](https://github.com/pina-rs/pina/issues/179), [#180](https://github.com/pina-rs/pina/issues/180), [#181](https://github.com/pina-rs/pina/issues/181), [#185](https://github.com/pina-rs/pina/issues/185), [#186](https://github.com/pina-rs/pina/issues/186), [#187](https://github.com/pina-rs/pina/issues/187), [#189](https://github.com/pina-rs/pina/issues/189), [#195](https://github.com/pina-rs/pina/issues/195)

#### Re-export zeropod fixed-capacity collection types

_Packages:_ _pina_

Replace Pina's local collection implementation with re-exports of zeropod's allocation-free `PodOption`, `PodString`, and `PodVec` storage types for advanced direct zeropod integrations. Pina's macro-generated account, instruction, and event schemas deliberately reject string/vector collection fields because not every upstream construction path initializes inactive capacity. Macro schemas support semantic `Option<scalar>` fields through an exact audited `PodOption` mapping; use fully initialized fixed byte arrays plus checked helpers for bounded text and lists.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #147](https://github.com/pina-rs/pina/pull/147) · _Related issues:_ [#205](https://github.com/pina-rs/pina/issues/205)

#### feat: add typed `#[pda]` attribute for PDA seed declarations

_Packages:_ _pina_, _pina_cli_, _pina_macros_, _pina_codama_renderer_

Adds a `#[pda(seeds = [...], bump = <field>)]` attribute macro for `#[account]` structs, inspired by Quasar's typed seed declarations. The macro generates owned seed structs (`XxxSeeds` / `XxxSeedsWithBump`), `seeds()` / `as_slices()` / `with_bump()` helpers, `try_find_pda()` / `find_pda()` derivation helpers, and `assert_seeds()` stored-bump verification when a bump field is declared.

Supported seed types: `Address`, `u8`, `u16`, `u32`, `u64`, `[u8; N]`, and `const &[u8]` references. The CLI parses the attribute for IDL generation with accurate seed types (fixing the escrow `u64` seed being mis-typed as an address in generated clients), and the renderer emits `find_pda` / `create_pda` helpers for linked accounts. All examples now use the attribute instead of manual seed macros.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #194](https://github.com/pina-rs/pina/pull/194)

#### Resolve PDA-derived account defaults in clients

_Packages:_ _pina_cli_, _pina_codama_renderer_

Resolve explicit PDA-derived account defaults in generated clients. Codama lowering now preserves deterministic PDA default metadata from account seeds, and the Rust renderer emits builders that derive those defaults while keeping signer and writable expectations explicit.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #158](https://github.com/pina-rs/pina/pull/158) · _Closed issues:_ [#144](https://github.com/pina-rs/pina/issues/144)

- **pina_cli**: Generate deterministic Dart clients for every example IDL alongside the Rust and JavaScript clients. Add package-root exports, strict semantic IDL validation, pinned dependency resolution, and byte-level tests covering Pina's zeropod account and instruction wire contracts.

#### Add UX improvements and parallel file I/O to the pina CLI

_Packages:_ _pina_cli_

Add UX improvements and parallel file I/O to the `pina` CLI.

- **Parallel file reading**: `resolve_crate` reads sibling module files in parallel via `rayon` while preserving deterministic parsing and error reporting.
- **Colored output**: Error messages and success indicators now use `owo-colors` for semantic terminal styling.
- **Summary table**: `pina idl` prints a `comfy-table` summary showing instruction, account, PDA, and error counts after generation.
- **`docs` subcommand**: `pina docs <topic>` renders bundled `.t.md` documentation in-terminal using `termimad`, with `PINA_TEMPLATES_DIR` support for custom topics.
- **New dependencies**: `rayon`, `owo-colors`, `comfy-table` (8.0), and `termimad` (0.35.1).

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #156](https://github.com/pina-rs/pina/pull/156)

#### Add semantic zeropod `String<N, PFX>`, `Vec<T, N, PFX>`, and fixed `Option<T>` parsing to the IDL toolchain for legacy IDLs and advanced direct zeropod integrations. Pina's audited account, instruction, and event macros accept only scalar `Option<T>` and reject string/vector, explicit `PodOption`, enum, custom, and nested layouts. Generated clients for supported Pina schemas therefore use scalar options and fully initialized fixed byte arrays; layouts Codama cannot represent faithfully fail generation instead of falling back to public keys.

_Packages:_ _pina_cli_

Encode account and instruction discriminators in generated clients, map signed Pod numeric elements at their real sizes, preserve generic capacity parameters during IDL extraction, reject noncanonical `PodString` length prefixes, initialize discriminators in typed account-creation helpers, and run the profile program's real SBF lifecycle in CI.

Generated JavaScript codecs validate discriminators, canonical booleans, and scalar-option tags. Advanced collection codecs reject values that exceed fixed capacity, decode UTF-8 strictly, and preserve embedded NUL characters, but those collection layouts are outside Pina's macro-generated schema contract.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #171](https://github.com/pina-rs/pina/pull/171) · _Related issues:_ [#205](https://github.com/pina-rs/pina/issues/205)

#### Support `discriminator = Enum::Variant` in the `#[instruction]`, `#[account]`, and `#[event]` attribute macros, replacing the separate `variant = Variant` argument.

_Packages:_ _pina_macros_

```rust
// Before
#[instruction(discriminator = VestingInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	// ...
}

// After
#[instruction(discriminator = VestingInstruction::Initialize)]
pub struct InitializeInstruction {
	// ...
}
```

The shorthand form is unchanged: when the struct name matches the variant, `discriminator = Enum` alone still works.

```rust
#[instruction(discriminator = VestingInstruction)]
pub struct Initialize {
	// ...
}
```

##### Migration guide

- Replace `#[instruction(discriminator = Enum, variant = Variant)]` with `#[instruction(discriminator = Enum::Variant)]`. The same applies to `#[account(...)]` and `#[event(...)]`.
- The old `variant = Variant` argument remains supported for backwards compatibility. When it is present, the complete `discriminator` value is treated as the enum path, which preserves qualified forms such as `crate::types::Enum, variant = Variant`.
- `pina_cli` IDL extraction understands all three forms: `Enum::Variant`, `Enum` + `variant = Variant`, and bare `Enum` (variant defaults to the struct name). The `pina init` template now emits the new syntax.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #172](https://github.com/pina-rs/pina/pull/172) · _Related issues:_ [#165](https://github.com/pina-rs/pina/issues/165), [#166](https://github.com/pina-rs/pina/issues/166), [#167](https://github.com/pina-rs/pina/issues/167), [#168](https://github.com/pina-rs/pina/issues/168), [#169](https://github.com/pina-rs/pina/issues/169), [#170](https://github.com/pina-rs/pina/issues/170), [#171](https://github.com/pina-rs/pina/issues/171), [#172](https://github.com/pina-rs/pina/issues/172), [#173](https://github.com/pina-rs/pina/issues/173), [#174](https://github.com/pina-rs/pina/issues/174), [#175](https://github.com/pina-rs/pina/issues/175), [#176](https://github.com/pina-rs/pina/issues/176), [#177](https://github.com/pina-rs/pina/issues/177), [#179](https://github.com/pina-rs/pina/issues/179), [#180](https://github.com/pina-rs/pina/issues/180), [#181](https://github.com/pina-rs/pina/issues/181), [#185](https://github.com/pina-rs/pina/issues/185), [#186](https://github.com/pina-rs/pina/issues/186), [#187](https://github.com/pina-rs/pina/issues/187), [#189](https://github.com/pina-rs/pina/issues/189), [#195](https://github.com/pina-rs/pina/issues/195)

### Fixes

#### Improve Codama IDL extraction coverage

_Packages:_ _core_

Improve Codama IDL extraction coverage so grouped match arms, accountless instructions, and instruction-only programs generate complete IDL and client surfaces. Document the supported extractor shapes and validation guarantees across the README, CLI docs, and mdBook.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #132](https://github.com/pina-rs/pina/pull/132)

#### Correct the CPI introspection guarantee

_Packages:_ _pina_

Expose `assert_current_instruction_program_id` for the guarantee the Instructions sysvar can actually provide. Deprecate the misleading `assert_no_cpi` name because transaction-level instruction metadata cannot detect self-CPI.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #204](https://github.com/pina-rs/pina/pull/204)

#### Add semantic zeropod `String<N, PFX>`, `Vec<T, N, PFX>`, and fixed `Option<T>` parsing to the IDL toolchain for legacy IDLs and advanced direct zeropod integrations. Pina's audited account, instruction, and event macros accept only scalar `Option<T>` and reject string/vector, explicit `PodOption`, enum, custom, and nested layouts. Generated clients for supported Pina schemas therefore use scalar options and fully initialized fixed byte arrays; layouts Codama cannot represent faithfully fail generation instead of falling back to public keys.

_Packages:_ _pina_, _pina_codama_renderer_

Encode account and instruction discriminators in generated clients, map signed Pod numeric elements at their real sizes, preserve generic capacity parameters during IDL extraction, reject noncanonical `PodString` length prefixes, initialize discriminators in typed account-creation helpers, and run the profile program's real SBF lifecycle in CI.

Generated JavaScript codecs validate discriminators, canonical booleans, and scalar-option tags. Advanced collection codecs reject values that exceed fixed capacity, decode UTF-8 strictly, and preserve embedded NUL characters, but those collection layouts are outside Pina's macro-generated schema contract.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #171](https://github.com/pina-rs/pina/pull/171) · _Related issues:_ [#205](https://github.com/pina-rs/pina/issues/205)

#### Preflight fallible account mutations

_Packages:_ _pina_

Check active borrows and the per-instruction growth limit before moving rent or closing account balances, so expected validation failures do not leave helper callers with partially mutated in-memory state.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #201](https://github.com/pina-rs/pina/pull/201)

- **pina**: Harden the token account loaders without local casts. SPL Token accounts retain Pinocchio's legacy state type, while Token-2022 accounts retain its complete `StateWithExtensions` type and validation. The new `TokenMintRef` and `TokenAccountRef` enums provide common field access for code that supports either program, while still exposing the concrete Token-2022 state when extensions are needed. Associated-token loaders validate against the explicitly selected program, so valid extensions are accepted without accepting overlong legacy accounts, malformed extensions, or mixed-program account sets.

#### Support `discriminator = Enum::Variant` in the `#[instruction]`, `#[account]`, and `#[event]` attribute macros, replacing the separate `variant = Variant` argument.

_Packages:_ _pina_cli_

```rust
// Before
#[instruction(discriminator = VestingInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	// ...
}

// After
#[instruction(discriminator = VestingInstruction::Initialize)]
pub struct InitializeInstruction {
	// ...
}
```

The shorthand form is unchanged: when the struct name matches the variant, `discriminator = Enum` alone still works.

```rust
#[instruction(discriminator = VestingInstruction)]
pub struct Initialize {
	// ...
}
```

##### Migration guide

- Replace `#[instruction(discriminator = Enum, variant = Variant)]` with `#[instruction(discriminator = Enum::Variant)]`. The same applies to `#[account(...)]` and `#[event(...)]`.
- The old `variant = Variant` argument remains supported for backwards compatibility. When it is present, the complete `discriminator` value is treated as the enum path, which preserves qualified forms such as `crate::types::Enum, variant = Variant`.
- `pina_cli` IDL extraction understands all three forms: `Enum::Variant`, `Enum` + `variant = Variant`, and bare `Enum` (variant defaults to the struct name). The `pina init` template now emits the new syntax.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #172](https://github.com/pina-rs/pina/pull/172) · _Related issues:_ [#165](https://github.com/pina-rs/pina/issues/165), [#166](https://github.com/pina-rs/pina/issues/166), [#167](https://github.com/pina-rs/pina/issues/167), [#168](https://github.com/pina-rs/pina/issues/168), [#169](https://github.com/pina-rs/pina/issues/169), [#170](https://github.com/pina-rs/pina/issues/170), [#171](https://github.com/pina-rs/pina/issues/171), [#172](https://github.com/pina-rs/pina/issues/172), [#173](https://github.com/pina-rs/pina/issues/173), [#174](https://github.com/pina-rs/pina/issues/174), [#175](https://github.com/pina-rs/pina/issues/175), [#176](https://github.com/pina-rs/pina/issues/176), [#177](https://github.com/pina-rs/pina/issues/177), [#179](https://github.com/pina-rs/pina/issues/179), [#180](https://github.com/pina-rs/pina/issues/180), [#181](https://github.com/pina-rs/pina/issues/181), [#185](https://github.com/pina-rs/pina/issues/185), [#186](https://github.com/pina-rs/pina/issues/186), [#187](https://github.com/pina-rs/pina/issues/187), [#189](https://github.com/pina-rs/pina/issues/189), [#195](https://github.com/pina-rs/pina/issues/195)

- **pina_cli**: Use the published `codama-renderers-dart@0.5.1` renderer and Solana Kit Dart `^0.8.0` runtime packages for generated clients, removing the temporary renderer patch and Git dependency overrides.

#### Preserve wide discriminator encodings

_Packages:_ _pina_cli_

Parse the complete discriminator attribute grammar when generating Codama IDLs and preserve `u16`, `u32`, and `u64` discriminator widths instead of silently lowering them to `u8`.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #200](https://github.com/pina-rs/pina/pull/200)

#### Refresh workspace dependencies

_Packages:_ _pina_macros_, _pina_codama_renderer_, _pina_profile_, _pina_sdk_ids_

Refresh workspace dependencies to their latest compatible versions.

- Bump `pinocchio-token` to `0.7` and `pinocchio-token-2022` to `0.4` (drops the `token_program` field from `TransferChecked`/`CloseAccount`; examples now use the `new()` constructors).
- Bump `codama-nodes` to `0.11` (spec `1.8.0`), `mollusk-svm` to `0.15`, `solana-account` to `4`, `solana-system-interface` to `3`, `insta-cmd` to `0.7`, and `object` to `0.40`.
- Bump the JS Codama toolchain (`codama` to `1.10`, `@codama/renderers-js` to `2.3`) and regenerate all IDLs and Rust/JS clients.
- Upgrade the workspace to `syn` 3, including the public `pina_cli` parsing API, and release the temporary derive-crate pins used during the migration.
- Resolves `RUSTSEC-2026-0097` (unsound `rand` 0.7.3) and `RUSTSEC-2026-0173` (unmaintained `proc-macro-error2`), dropping them from the dependency tree, and updates `jiff`, `defmt`, `env_logger`, `solana-logger`, and `crossbeam-epoch` to patched versions.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #167](https://github.com/pina-rs/pina/pull/167) · _Related issues:_ [#165](https://github.com/pina-rs/pina/issues/165), [#166](https://github.com/pina-rs/pina/issues/166), [#167](https://github.com/pina-rs/pina/issues/167), [#168](https://github.com/pina-rs/pina/issues/168), [#169](https://github.com/pina-rs/pina/issues/169), [#170](https://github.com/pina-rs/pina/issues/170), [#171](https://github.com/pina-rs/pina/issues/171), [#172](https://github.com/pina-rs/pina/issues/172), [#173](https://github.com/pina-rs/pina/issues/173), [#174](https://github.com/pina-rs/pina/issues/174), [#175](https://github.com/pina-rs/pina/issues/175), [#176](https://github.com/pina-rs/pina/issues/176), [#177](https://github.com/pina-rs/pina/issues/177), [#179](https://github.com/pina-rs/pina/issues/179), [#180](https://github.com/pina-rs/pina/issues/180), [#181](https://github.com/pina-rs/pina/issues/181), [#185](https://github.com/pina-rs/pina/issues/185), [#186](https://github.com/pina-rs/pina/issues/186), [#187](https://github.com/pina-rs/pina/issues/187), [#189](https://github.com/pina-rs/pina/issues/189), [#195](https://github.com/pina-rs/pina/issues/195)

- **pina_macros**: Improve the `#[discriminator]` size-assertion error message: it now reports the primitive's byte width (e.g. `u128` (16 bytes)) and lists the supported primitives (`u8`, `u16`, `u32`, `u64`), instead of only naming the symbolic `MAX_DISCRIMINATOR_SPACE` constant.
- **pina_macros**: Replace latent panic paths in the proc macros with spanned compile errors and explicit internal-error panics. `#[derive(Accounts)]` now reports a clear error instead of panicking if the input shape is ever accepted by `darling` without named fields, and the remaining `unwrap()` calls on provably-safe values carry explanatory messages. Add trybuild negative tests for `#[derive(Accounts)]` on enums and tuple structs.

#### Harden generated output boundaries

_Packages:_ _pina_codama_renderer_

Reject unsafe names and literals before rendering, validate generated Rust before replacing existing output, and constrain cleanup to managed files within the requested destination.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #198](https://github.com/pina-rs/pina/pull/198)

- **core**: Repair regressions discovered while auditing the recently merged pull requests. Ensure the npm package is built and verified before publication, publish its real CommonJS, ESM, and declaration entry points, build CPI-heavy examples with Solana's supported SBF toolchain, include the associated-token program in escrow instructions, and harden the release, development-shell, fuzz, mutation, and end-to-end workflows that guard the workspace. Pull-request mutation tests remain advisory, but surviving mutants now appear as a failed step and explicit summary instead of a false-green result.
- **codama-nodes-from-pina**: Update the Codama dependency to 1.10.1 and refresh the JavaScript development and test toolchain.

### Documentation

#### Improve documentation for guard-backed account access, cursor safety, inline_always allowance, and the log macro.

_Packages:_ _pina_

- **A1**: Add a "Guard lifetime" section to `AsAccount` explaining that `Ref`/`RefMut` guards block incompatible borrows while alive and should be dropped before later mutable access or CPIs.
- **A2**: Document that `AccountsCursor::next_mut` rejects aliases for individually parsed mutable fields, while `peek` performs no validation and `remaining_mut` deliberately preserves trailing-account aliases.
- **H2**: Annotate `#![allow(clippy::inline_always)]` with a rationale comment explaining CU optimization for on-chain programs.
- **H3**: Move the `log!` format-arg limitation into a dedicated `# Limitations` doc section.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #177](https://github.com/pina-rs/pina/pull/177) · _Related issues:_ [#165](https://github.com/pina-rs/pina/issues/165), [#166](https://github.com/pina-rs/pina/issues/166), [#167](https://github.com/pina-rs/pina/issues/167), [#168](https://github.com/pina-rs/pina/issues/168), [#169](https://github.com/pina-rs/pina/issues/169), [#170](https://github.com/pina-rs/pina/issues/170), [#171](https://github.com/pina-rs/pina/issues/171), [#172](https://github.com/pina-rs/pina/issues/172), [#173](https://github.com/pina-rs/pina/issues/173), [#174](https://github.com/pina-rs/pina/issues/174), [#175](https://github.com/pina-rs/pina/issues/175), [#176](https://github.com/pina-rs/pina/issues/176), [#177](https://github.com/pina-rs/pina/issues/177), [#179](https://github.com/pina-rs/pina/issues/179), [#180](https://github.com/pina-rs/pina/issues/180), [#181](https://github.com/pina-rs/pina/issues/181), [#185](https://github.com/pina-rs/pina/issues/185), [#186](https://github.com/pina-rs/pina/issues/186), [#187](https://github.com/pina-rs/pina/issues/187), [#189](https://github.com/pina-rs/pina/issues/189), [#195](https://github.com/pina-rs/pina/issues/195)

#### Refresh shared documentation after Pinocchio 0.11 migration

_Packages:_ _core_

Refresh the shared documentation after the Pinocchio 0.11 migration. This expands feature-selection guidance, adds explicit instruction-authoring tips for the new mutable `AccountView` and guard-backed loader model, clarifies close-account safety with `close_account_zeroed()`, and updates the security and design notes to match the current APIs.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #151](https://github.com/pina-rs/pina/pull/151)

#### Correct the CPI introspection guarantee

_Packages:_ _pina_cli_

Expose `assert_current_instruction_program_id` for the guarantee the Instructions sysvar can actually provide. Deprecate the misleading `assert_no_cpi` name because transaction-level instruction metadata cannot detect self-CPI.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #204](https://github.com/pina-rs/pina/pull/204)

### Notes

#### Keep prebuilt sbpf-linker available in devenv

_Packages:_ _core_

Keep the prebuilt `custom.sbpf-linker` package available in `devenv` so BPF and binary-size jobs can find `sbpf-linker` on `PATH`, while disabling the package's Nix `installCheckPhase`. The upstream binary now requires linker inputs and `--output`, so invoking it with no arguments during the install check fails on Linux; on Darwin the same check also exposed a stale Homebrew LLVM load path. Skipping the install check unblocks `devenv shell` without removing the linker used by CI.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/pina-rs/pina/pull/161)

#### Refresh devenv.lock

_Packages:_ _core_

Refresh `devenv.lock` to pick up the latest `ifiokjr/nixpkgs` fixes for `pnpm-standalone` activation in CI.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #107](https://github.com/pina-rs/pina/pull/107)

- **core**: Repair regressions discovered while auditing the recently merged pull requests. Ensure the npm package is built and verified before publication, publish its real CommonJS, ESM, and declaration entry points, build CPI-heavy examples with Solana's supported SBF toolchain, include the associated-token program in escrow instructions, and harden the release, development-shell, fuzz, mutation, and end-to-end workflows that guard the workspace. Pull-request mutation tests remain advisory, but surviving mutants now appear as a failed step and explicit summary instead of a false-green result.
- **pina_profile**: Replace `.expect()`/`.unwrap()` calls in pina_profile tests with `unwrap_or_else` and explicit, descriptive panic messages per repo conventions.
- **codama-nodes-from-pina**: Generate deterministic Dart clients for every example IDL alongside the Rust and JavaScript clients. Add package-root exports, strict semantic IDL validation, pinned dependency resolution, and byte-level tests covering Pina's zeropod account and instruction wire contracts.

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

#### Unify all crate and package versions under a single `[workspace.package] version` field. All publishable crates (`pina`, `pina_macros`, `pina_pod_primitives`, `pina_sdk_ids`, `pina_cli`, `pina_codama_renderer`) and the `codama-nodes-from-pina` JS package now share the same version, managed by a single `[package]` entry in `monochange.toml`. This replaces the previous per-crate `[packages.*]` configuration and ensures all crates are released together with a single version bump.

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
- Move `ignore_conventional_commits` from `PrepareRelease` to the `[changes]` section in `monochange.toml` to match current `monochange` configuration expectations.

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
