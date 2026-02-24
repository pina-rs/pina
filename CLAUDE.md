# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Pina is a performant Solana smart contract framework built on top of `pinocchio` — a lightweight alternative to `solana-program` that massively reduces dependency bloat and compute units. The workspace produces `no_std`-compatible libraries for on-chain Solana programs.

## Build & Development

### Environment Setup

Uses `devenv` (Nix-based) for reproducible development environments. After cloning:

```sh
# Enter the dev shell (automatic with direnv, or manually):
devenv shell

# Install all tooling (cargo binaries + GitHub release binaries):
install:all
```

Cargo binaries are managed via `cargo-run-bin` (versions pinned in `[workspace.metadata.bin]` in root `Cargo.toml`). External binaries (Solana CLI/agave, surfpool) are managed via `eget` (config in `.eget/.eget.toml`).

### Key Commands

```sh
# Build
cargo build --all-features        # Build all crates
cargo build-escrow-program        # Build escrow example for SBF target

# Test
cargo test                        # Run all tests
cargo nextest run                 # Run tests with nextest (faster)
cargo test -p pina                # Test a single crate
cargo test -p pina -- test_name   # Run a single test

# Lint & Format
lint:all                          # Run all checks (clippy + format)
lint:clippy                       # cargo clippy --all-features
lint:format                       # dprint check
fix:all                           # Auto-fix all (clippy + format)
fix:clippy                        # cargo clippy --fix
fix:format                        # dprint fmt

# Reusable docs (mdt)
docs:sync                         # mdt update over the repo
docs:check                        # mdt check over the repo
verify:docs                       # docs:check + mdbook build

# Coverage
cargo llvm-cov                    # Code coverage via cargo-llvm-cov

# Semver checking
cargo semver-checks               # Check for semver violations
```

When using `devenv`, `pina ...` is available as a shortcut for `cargo run -p pina_cli -- ...`. Reusable docs providers live in `template.t.md`.

### Formatting

Formatting is handled by `dprint` (not `cargo fmt` directly). dprint delegates to `rustfmt` for `.rs` files, `nixfmt` for `.nix`, and `shfmt` for shell scripts. Always use `fix:format` or `dprint fmt` rather than running `rustfmt` directly.

Key style rules: hard tabs, max width 100, one import per line (`imports_granularity = "Item"`), imports grouped by `StdExternalCrate`.

## Architecture

### Workspace Crates

- **`crates/pina`** — Core framework. Provides traits (`AccountValidation`, `AccountInfoValidation`, `TryFromAccountInfos`, `ProcessAccountInfos`, `HasDiscriminator`), account loaders, Pod types, CPI helpers, and the `nostd_entrypoint!` macro. Features: `logs` (solana-program-log), `token` (SPL token support), `derive` (proc macros).
- **`crates/pina_macros`** — Proc-macro crate. Provides `#[derive(Accounts)]`, `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]` attribute macros. Uses `darling` for attribute parsing.
- **`crates/pina_sdk_ids`** — Solana program and sysvar IDs using `solana_address::declare_id!`.
- **`examples/escrow_program`** — Reference implementation of a token escrow using pina. Built as `cdylib` for SBF target with `bpf-entrypoint` feature.

### Core Patterns

**Entrypoint pattern:**

```rust
nostd_entrypoint!(process_instruction);
fn process_instruction(
	program_id: &Address,
	accounts: &[AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: MyInstruction = parse_instruction(program_id, &ID, data)?;
	match instruction {
		MyInstruction::Action => MyAccounts::try_from(accounts)?.process(data),
	}
}
```

**Discriminator system:** Every account/instruction/event type has a discriminator enum (u8/u16/u32/u64) as its first field. The `#[discriminator]` macro generates conversions and `Pod`/`Zeroable` impls. The `#[account]`/`#[instruction]`/`#[event]` macros auto-inject a discriminator field and generate `HasDiscriminator` + validation impls.

**Account validation:** Chain assertions on `AccountView` references: `account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?`. These methods return `Result<&AccountView>` for chaining.

**Pod types:** `PodBool`, `PodU16`, `PodU32`, `PodU64`, `PodU128`, `PodI16`, `PodI64` — alignment-safe wrappers for use in `#[repr(C)]` account structs with bytemuck.

### Building for SBF (on-chain)

Programs are compiled to the `bpfel-unknown-none` target using `sbpf-linker`. The `.cargo/config.toml` sets linker flags. Example:

```sh
cargo build-escrow-program
# Expands to: cargo build --release --target bpfel-unknown-none -p escrow_program -Z build-std -F bpf-entrypoint
```

The `bpf-entrypoint` feature gate separates the on-chain entrypoint from the library code used in tests.

### Testing Solana Programs

Uses `mollusk-svm` for Solana VM simulation in tests. Programs are tested as regular Rust libraries (without the `bpf-entrypoint` feature) against the mollusk SVM. Test utilities from `test_utils_solana` are also available.

## Toolchain

- **Rust:** Nightly (`nightly-2025-11-20`), edition 2024, MSRV 1.86.0
- **Formatter:** dprint (orchestrates rustfmt, nixfmt, shfmt)
- **Linting:** clippy with strict workspace lints — `unsafe_code` and `unstable_features` are **denied**
- **Test runner:** cargo-nextest (also standard `cargo test`)
- **Coverage:** cargo-llvm-cov
- **Snapshot testing:** cargo-insta
- **Custom lints:** dylint (cargo-dylint + dylint-link)
- **Semver:** cargo-semver-checks
- **Release management:** knope (bot workflow)
- **Publishing:** cargo-workspaces (`cargo workspaces publish --from-git`)

## Release & Changelog Workflow

Uses [knope bot workflow](https://knope.tech/tutorials/bot-workflow/). Each crate has its own changelog and version.

```sh
# Document a change (creates a changeset file in .changeset/):
knope document-change

# Prepare a release (bumps versions, updates changelogs):
knope release

# Publish to crates.io:
knope publish
```

Changesets should be highly detailed. Conventional commit scopes map to packages: `pina`, `pina_macros`, `pina_sdk_ids`. Extra changelog sections: `Notes` (type: `note`) and `Documentation` (type: `docs`).

### Changeset Requirement

**Every pull request that modifies code in any crate (`crates/` or `examples/`) MUST include at least one changeset file in `.changeset/`.** This ensures all changes are tracked in changelogs and version bumps are applied correctly.

To create a changeset interactively:

```sh
knope document-change
```

Or create one manually by adding a markdown file in `.changeset/` with TOML-style frontmatter:

```markdown
---
package_name: change_type
---

Detailed description of the change.
```

**Change types:**

- `major` — breaking changes
- `minor` — new features (backwards compatible)
- `patch` — bug fixes
- `docs` — documentation-only changes
- `note` — general notes

**Package names:** `pina`, `pina_macros`, `pina_sdk_ids`

A single changeset file can reference multiple packages. Always run `dprint fmt .changeset/* --allow-no-files` after creating changeset files.

## Git & PR Workflow

- Create a dedicated branch for each change before committing.
- Branch names must use `feat/<description>` for features or `fix/<description>` for bug fixes.
- Do not use the `codex/` branch prefix.
- Commit messages must follow [Conventional Commits](https://www.conventionalcommits.org/).
- Push the branch and open a pull request for review.
- After review, apply any requested fixes (including breaking-change fixes) on the same branch.
- Merge only after approvals are complete.

## Cargo Aliases

Defined in `.cargo/config.toml` — these proxy to `cargo-run-bin`:

- `cargo dylint` — run cargo-dylint
- `cargo insta` — run cargo-insta
- `cargo llvm-cov` — run cargo-llvm-cov
- `cargo nextest` — run cargo-nextest
- `cargo semver-checks` — run cargo-semver-checks
- `cargo workspaces` — run cargo-workspaces

## Security Constraints

- `unsafe_code` is **denied** workspace-wide
- `unstable_features` is **denied** workspace-wide
- `clippy::correctness` is **denied** (not just warned)
- `clippy::wildcard_dependencies` is **denied**
- `Result::expect` is a disallowed method (use `unwrap_or_else` with explicit panic message instead)
- All account operations should use the validation trait chain for safety
- Programs must be `no_std` compatible for on-chain deployment
