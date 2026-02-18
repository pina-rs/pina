# Implementation Plan: Pinocchio Ecosystem Upgrade & Crate Cleanup

**Branch**: `001-pinocchio-upgrade` | **Date**: 2026-02-15 | **Spec**: [spec.md](spec.md) **Input**: Feature specification from `/specs/001-pinocchio-upgrade/spec.md`

## Summary

Upgrade all pinocchio ecosystem dependencies from 0.9.x to 0.10.x, adapting to the fundamental type renames (AccountInfo→AccountView, Pubkey→Address) and module restructuring. Remove the deprecated `pina_token_2022_extensions` crate. Replace `pinocchio-pubkey` with `solana-address` and `pinocchio-log` with `solana-program-log`. Update all crate source, macros, tests, examples, documentation, and configuration. Create granular knope changesets for each breaking change.

## Technical Context

**Language/Version**: Rust nightly (`nightly-2025-11-20`), edition 2024, MSRV 1.86.0 **Primary Dependencies**: pinocchio 0.10.x, solana-address 2.x, solana-program-log 1.x, bytemuck 1.x **Storage**: N/A (on-chain Solana account data) **Testing**: cargo-nextest, mollusk-svm (SVM simulation), cargo-insta (snapshots) **Target Platform**: Solana BPF (`bpfel-unknown-none`) for on-chain, native for tests **Project Type**: Rust workspace (4 library crates + 1 example) **Performance Goals**: Zero CU overhead vs raw pinocchio; no_std compatible **Constraints**: No unsafe code, no heap allocations on-chain, alignment-safe Pod types **Scale/Scope**: ~30 source files across 5 crates, ~3000 LOC to modify

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

| Principle                           | Status | Notes                                                                                                                                                          |
| ----------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I. no_std & Performance First       | PASS   | All dependencies default to `default-features = false`. Pinocchio 0.10.x forward allocator saves ~7 CUs/alloc. No new std dependencies.                        |
| II. Safety Without Unsafe           | PASS   | No new unsafe code introduced. Removing `pina_token_2022_extensions` eliminates `#![allow(unsafe_code)]` from the workspace. Validation trait chain preserved. |
| III. Strict Code Quality            | PASS   | All code will pass `lint:all`, `dprint fmt`, `cargo semver-checks`. Breaking changes documented in `major` changesets.                                         |
| IV. Testing Discipline              | PASS   | All existing tests updated. New tests added for expanded API. mollusk-svm simulation preserved.                                                                |
| V. Semantic Versioning & Changesets | PASS   | Granular changesets per breaking change. `cargo semver-checks` validates. Major version bump for all affected crates.                                          |

**Security & Deployment**: Removing `pina_token_2022_extensions` (which had `#![allow(unsafe_code)]`) improves workspace security posture. SBF build and `bpf-entrypoint` feature gate preserved.

**Post-Design Re-check**: PASS — no new violations introduced by the design. The dependency graph is simpler (fewer crates, fewer transitive deps) after removal of `pinocchio-pubkey` and `pina_token_2022_extensions`.

## Project Structure

### Documentation (this feature)

```text
specs/001-pinocchio-upgrade/
├── plan.md              # This file
├── research.md          # Phase 0: upstream API research
├── data-model.md        # Phase 1: type/entity changes
├── quickstart.md        # Phase 1: verification steps
└── tasks.md             # Phase 2: implementation tasks
```

### Source Code (repository root)

```text
crates/
├── pina/
│   ├── Cargo.toml           # Dependency version bumps, new deps
│   ├── src/
│   │   ├── lib.rs           # Re-export updates, macro updates
│   │   ├── traits.rs        # AccountInfo→AccountView, Pubkey→Address
│   │   ├── loaders.rs       # Validation impl updates
│   │   ├── cpi.rs           # CPI helper updates
│   │   ├── error.rs         # Unchanged
│   │   ├── utils.rs         # Type updates
│   │   └── pod/             # Unchanged
│   └── tests/               # Update test types
├── pina_macros/
│   ├── Cargo.toml           # Unchanged (no pinocchio deps)
│   └── src/
│       ├── lib.rs           # Generated code references (AccountView)
│       └── args.rs          # Unchanged
├── pina_sdk_ids/
│   ├── Cargo.toml           # pinocchio-pubkey → solana-address
│   └── src/lib.rs           # declare_id! macro source change
└── [REMOVED] pina_token_2022_extensions/

examples/
└── escrow_program/
    ├── Cargo.toml           # Unchanged (depends on pina)
    └── src/lib.rs           # Type updates throughout

# Config files
Cargo.toml                   # Workspace deps, members
knope.toml                   # Remove pina_token_2022_extensions
CLAUDE.md                    # Update architecture docs
readme.md                    # Comprehensive rewrite
.specify/memory/constitution.md  # Remove package scope
```

**Structure Decision**: Existing Rust workspace structure. No new crates or directories. One crate removed (`pina_token_2022_extensions`). The workspace shrinks from 5 members to 4.

## Complexity Tracking

No constitution violations. The upgrade is a breaking change but is the minimum necessary change — every modification is required by the upstream pinocchio 0.10.x migration.

## File-by-File Change Summary

### PR1: Pinocchio Upgrade (single PR)

**crates/pina/Cargo.toml** — Bump all pinocchio deps, replace `pinocchio-pubkey` with `solana-address`, replace `pinocchio-log` with `solana-program-log`, add `cpi` feature to pinocchio.

**crates/pina/src/lib.rs** — Update all re-exports:

- `pinocchio::account_info::AccountInfo` → `pinocchio::AccountView`
- `pinocchio::pubkey::Pubkey` → `pinocchio::Address`
- `pinocchio::instruction::*` → behind cpi feature
- `pinocchio_pubkey::*` → `pinocchio::address::*`
- `pinocchio_log::*` → `solana_program_log::*`
- Update `nostd_entrypoint!` macro documentation
- Update `log!` macro to forward to `solana_program_log`

**crates/pina/src/traits.rs** — Replace all `AccountInfo` with `AccountView`, `Pubkey` with `Address`. Consider renaming traits (`AccountInfoValidation` → keep name or rename). Update method signatures.

**crates/pina/src/loaders.rs** — Update all `impl` blocks from `AccountInfo` to `AccountView`. Update method calls (`key()` → `address()`, `try_borrow_data()` → `try_borrow()`, `try_borrow_mut_data()` → `try_borrow_mut()`). Update PDA functions (`try_find_program_address` import path).

**crates/pina/src/cpi.rs** — Update CPI types (`Instruction` → `InstructionView`, `AccountMeta` → `InstructionAccount`, `Signer` path change). Update `invoke_signed` import path. Update `combine_seeds_with_bump` for new `Seed` type. Update `Rent::get()` usage.

**crates/pina/src/utils.rs** — Update `Pubkey` → `Address` in `try_get_associated_token_address`. Update pinocchio ATA ID import.

**crates/pina/src/error.rs** — Likely unchanged (uses pina's own error macro).

**crates/pina/src/pod/** — Unchanged (pure byte-level types).

**crates/pina/tests/*.rs** — Update all test types.

**crates/pina_macros/src/lib.rs** — Update generated code references: `::pina::AccountInfo` → `::pina::AccountView` in `#[derive(Accounts)]`. `::pina::TryFromAccountInfos` → update trait name if renamed.

**crates/pina_sdk_ids/Cargo.toml** — Replace `pinocchio-pubkey` with `solana-address` (features: `["decode"]`).

**crates/pina_sdk_ids/src/lib.rs** — Replace all `pinocchio_pubkey::declare_id!` with `solana_address::declare_id!`.

**examples/escrow_program/src/lib.rs** — Update all types: `Pubkey` → `Address`, `AccountInfo` → `AccountView`. Update CPI calls and validation chain calls.

**Root Cargo.toml** — Remove `pina_token_2022_extensions` member and dependency. Bump all pinocchio workspace dep versions. Add `solana-address` and `solana-program-log`. Remove `pinocchio-pubkey`.

**knope.toml** — Remove `pina_token_2022_extensions` package config.

**CLAUDE.md** — Update architecture section, remove `pina_token_2022_extensions` references, update type names in examples.

**readme.md** — Comprehensive rewrite with new API types.

**.specify/memory/constitution.md** — Remove `pina_token_2022_extensions` from package scopes in Principle V.

**Delete** — `crates/pina_token_2022_extensions/` (entire directory).

### PR2: Ported Examples (separate PR, after PR1 merges)

**examples/** — New example programs ported from solana-developers/program-examples, using pina with comprehensive documentation.
