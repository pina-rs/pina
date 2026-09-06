# `pina`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

<br>

Core runtime crate for building Solana programs on top of [`pinocchio`](https://github.com/anza-xyz/pinocchio).

It provides zero-copy account loaders, discriminator-aware account/instruction/event modeling, account validation traits, and `no_std` entrypoint helpers.

<!-- {=crateReadmeBadgeRow:"pina"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina-orange?logo=rust)](https://crates.io/crates/pina) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina-1f425f?logo=docs.rs)](https://docs.rs/pina/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

## Installation

<br>

```bash
cargo add pina
```

Enable optional token helpers:

```bash
cargo add pina --features token
```

## What This Crate Includes

<br>

- `nostd_entrypoint!` for `no_std` Solana entrypoint wiring.
- `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` integration via the default `derive` feature.
- Validation chains on `AccountView` (`assert_signer`, `assert_writable`, `assert_owner`, PDA checks, sysvar checks, and more).
- Zero-copy POD wrappers (`PodU*`, `PodI*`, `PodBool`) for stable on-chain layouts.
- CPI helpers for system/token operations.

## Compact accounts

<br>

<!-- {=compactAccountQuickstart} -->

Compact mode is opt-in. Enable `compact` for schemas and checked loaders; add `account-resize` when using the typed creation and rent-adjusting reallocation builders:

```toml
[dependencies]
pina = { version = "...", features = ["compact", "account-resize"] }
```

The `compact` feature also enables `derive`. Add `compact` to an account with a suffix of one or more bounded `Vec` fields. Fixed fields must come first, and every capacity must be a literal so the macro can audit and generate the maximum layout:

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub entries: Vec<u64, 8>,
	pub markers: PodVec<u8, 8, 8>,
}

fn account_size(entry_count: usize) -> Result<usize, ProgramError> {
	if entry_count > 8 {
		return Err(ProgramError::InvalidArgument);
	}

	Ok(Journal::HEADER_SIZE
		+ entry_count * (core::mem::size_of::<PodU64>() + core::mem::size_of::<u8>()))
}
```

The macro generates `JournalHeader`, `JournalRef`, and `JournalMut`, plus `HEADER_SIZE`, `MAX_SIZE`, checked load/initialize methods, and the compact-account traits used by Pina's typed CPI builders. Every tail length is stored in the fixed header. Active payloads are concatenated after that header in declaration order; declared capacity is a validation bound, not reserved space.

Pina uses Pinapod, its maintained and wire-compatible ZeroPod fork. Each immutable and mutable accessor reads its own length prefix, so compact tails may have independent active lengths.

<!-- {/compactAccountQuickstart} -->

<!-- {=compactAccountResizeOrdering} -->

Compact mutation has one important ordering rule:

- **Grow:** calculate and validate the target size, call `ReallocCompactAccount` first, then write and `commit` the longer tail.
- **Same size:** write and `commit`; skip the realloc CPI.
- **Shrink or clear:** write and `commit` the shorter tail first, then call `ReallocCompactAccount` with the returned encoded size.

```rust
if target_size > account.data_len() {
	ReallocCompactAccount {
		account,
		payer,
		new_size: target_size,
		program_id,
	}
	.invoke::<Journal>()?;
}

let encoded_size = {
	let mut data = account.try_borrow_mut()?;
	let mut journal = Journal::try_from_bytes_mut(&mut data)?;
	journal
		.set_entries(entries)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.set_markers(markers)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.commit()
		.map_err(|_| ProgramError::InvalidAccountData)?
};

if encoded_size < account.data_len() {
	ReallocCompactAccount {
		account,
		payer,
		new_size: encoded_size,
		program_id,
	}
	.invoke::<Journal>()?;
}
```

`ReallocCompactAccount` checks the current compact type, validates the destination size, and verifies that a shrink retains every active tail before moving rent. It preserves rent exemption on growth and refunds excess lamports to `payer` on shrink. Scope immutable runtime borrows with `with_compact_account`; use a direct `try_borrow_mut` guard when a tail setter must borrow instruction-local values through `commit`.

<!-- {/compactAccountResizeOrdering} -->

See `examples/compact_accounts` for a complete lifecycle with unit and Surfpool coverage.

## Feature Flags

<br>

<!-- {=pinaFeatureFlags} -->

| Feature          | Default | Description                                                  |
| ---------------- | ------- | ------------------------------------------------------------ |
| `derive`         | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.)   |
| `logs`           | Yes     | Enables on-chain logging via `solana-program-log`            |
| `compact`        | No      | Enables compact schemas, checked loaders, and typed APIs     |
| `token`          | No      | Enables SPL token / token-2022 helpers and ATA utilities     |
| `memo`           | No      | Enables memo program helpers via `pina::memo`                |
| `account-resize` | No      | Enables raw account reallocation and safe Pinocchio resizing |

<!-- {/pinaFeatureFlags} -->

## Feature selection tips

<br>

<!-- {=pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `compact` enables `#[account(compact)]`, `PinaCompactAccount`, compact account validation/loaders, and `pina::Vec`; it also enables `derive`.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` enables `ReallocAccount` and `ReallocAccountZeroed`. Enable it together with `compact` for `ReallocCompactAccount` and the compact creation builders. Close helpers still do not implicitly resize or zero account data.

<!-- {/pinaFeatureSelectionTips} -->

## Minimal Program Skeleton

<br>

```rust
#![no_std]

use pina::*;

declare_id!("YourProgramId11111111111111111111111111111111");

#[discriminator]
pub enum Instruction {
	Initialize = 0,
}

#[instruction(discriminator = Instruction, variant = Initialize)]
pub struct InitializeInstruction {}

nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let ix: Instruction = parse_instruction(program_id, &ID, data)?;
	match ix {
		Instruction::Initialize => {
			let _ = InitializeInstruction::try_from_bytes(data)?;
			let _ = accounts;
			Ok(())
		}
	}
}
```

## PDAs, accounts, and validation in practice

`examples/counter_program` wires the full loop: discriminator-first instructions, a PDA-seeded account, and a validation chain that ends in a checked zero-copy mutation.

```rust
#[account(discriminator = CounterAccountType)]
#[pda(seeds = [SEED_COUNTER, authority: Address], bump = bump)]
pub struct CounterState {
	pub bump: u8,
	pub count: u64,
}

#[derive(Accounts, Debug)]
pub struct IncrementAccounts<'a> {
	pub authority: &'a AccountView,
	pub counter: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for IncrementAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = IncrementInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.counter
			.assert_not_empty()?
			.assert_type::<CounterState>(&ID)?;

		// Verify the account is the PDA for the authority, using the stored
		// bump field (avoids re-deriving the canonical bump on-chain).
		CounterState::assert_seeds(self.counter, self.authority.address(), &ID)?;

		// Mutate state
		let mut counter = self.counter.as_account_mut::<CounterState>(&ID)?;
		let next = counter
			.count
			.get()
			.checked_add(1)
			.ok_or(ProgramError::ArithmeticOverflow)?;
		counter.count.set(next);

		Ok(())
	}
}
```

For the complete program — `CreateProgramAccountWithBump`, `log!`, and the isolated Surfpool tests — see [`examples/counter_program`](https://github.com/pina-rs/pina/tree/main/examples/counter_program).

## Instruction authoring tips

<br>

<!-- {=pinaInstructionAuthoringTips} -->

- Entry points should accept `&mut [AccountView]` and dispatch with `Accounts::try_from((program_id, accounts))?.process(data)`.
- Use `&AccountView` for read-only accounts and `&mut AccountView` only when you need mutable loaders, direct lamport mutation, `close_*` helpers, or writable IDL inference.
- Keep `assert_writable()` explicit even on `&mut AccountView`. Type-level mutability enables mutable APIs, but the runtime still decides whether the account is writable for the current instruction.
- `as_account()` / `as_account_mut()` return `Ref<T>` / `RefMut<T>` borrow guards. Copy out the fields you need and `drop(...)` the guard before CPIs or later mutable borrows.
- Keep validation chains direct inside `process(self, ...)` when possible. That makes audits easier and gives `pina idl` the clearest signal for signer, writable, PDA, and default-account inference.

<!-- {/pinaInstructionAuthoringTips} -->

## Related Crates

<br>

- [`pina_macros`](https://docs.rs/pina_macros): proc-macro implementations for the attributes and derives used here.
- [`pina_cli`](https://docs.rs/pina_cli): CLI/library used to generate Codama IDLs from Pina programs.
- [`pina_sdk_ids`](https://docs.rs/pina_sdk_ids): shared Solana program/sysvar IDs.

## Codama IDLs

<br>

`pina` models are designed to be extracted into Codama IDLs through `pina_cli`.

```bash
pina idl --path ./my_program --output ./idls/my_program.json
```

From there you can generate JS clients with Codama renderers, or Pina-style Rust clients using this repository's `pina_codama_renderer` tool.
