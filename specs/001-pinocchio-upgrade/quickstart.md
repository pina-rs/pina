# Quickstart: Verifying the Pinocchio Upgrade

**Date**: 2026-02-15 **Branch**: `001-pinocchio-upgrade`

## Prerequisites

- Rust nightly toolchain (`nightly-2025-11-20` or later)
- devenv shell active (`devenv shell`)
- All tools installed (`install:all`)

## Step 1: Verify Build

```sh
# Build all crates with all features
cargo build --all-features
```

Expected: Zero errors. All crates compile against pinocchio 0.10.x.

## Step 2: Run Tests

```sh
# Run all tests
cargo nextest run

# Or with standard test runner
cargo test
```

Expected: All existing and new tests pass.

## Step 3: Build Escrow for SBF

```sh
cargo build-escrow-program
```

Expected: Escrow example builds for `bpfel-unknown-none` target.

## Step 4: Verify Lint & Format

```sh
lint:all
```

Expected: Clippy and dprint checks pass.

## Step 5: Check Semver

```sh
cargo semver-checks
```

Expected: Reports breaking changes (expected for major upgrade). Verify all reported changes are captured in changesets.

## Step 6: Verify Crate Removal

```sh
# Should find zero results
grep -r "pina_token_2022_extensions" --include="*.toml" --include="*.rs" --include="*.md" .
```

Expected: Zero matches.

## Step 7: Verify Changesets

```sh
ls .changeset/
dprint fmt .changeset/* --allow-no-files
```

Expected: At least 3 changeset files, all pass formatting.

## Smoke Test: New API Types

```rust
use pina::AccountView; // was AccountInfo
use pina::Address; // was Pubkey
use pina::ProgramResult;

fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
	// Account validation chains work the same
	let account = &accounts[0];
	account.assert_signer()?.assert_writable()?;
	Ok(())
}
```
