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

## Feature Flags

<br>

<!-- {=pinaFeatureFlags} -->

| Feature          | Default | Description                                                     |
| ---------------- | ------- | --------------------------------------------------------------- |
| `derive`         | Yes     | Enables proc macros (`#[account]`, `#[instruction]`, etc.)      |
| `logs`           | Yes     | Enables on-chain logging via `solana-program-log`               |
| `token`          | No      | Enables SPL token / token-2022 helpers and ATA utilities        |
| `memo`           | No      | Enables memo program helpers via `pina::memo`                   |
| `account-resize` | No      | Enables account realloc helpers that call Pinocchio resize APIs |

<!-- {/pinaFeatureFlags} -->

## Feature selection tips

<br>

<!-- {=pinaFeatureSelectionTips} -->

- `derive` is the normal choice for program crates; disable it only when you want the low-level runtime traits without the proc macros.
- `logs` is useful during **initial development and debugging**, testing, and audits. Disable it when you want the smallest possible binary or completely silent runtime failures.
- `token` enables `pina::token`, `pina::token_2022`, `pina::associated_token_account`, and the `TokenAccount` compatibility aliases over the upstream renamed account types.
- `memo` is separate from `token`, so memo CPI support can be enabled without pulling in the token helper surface.
- `account-resize` only enables the `ReallocAccount` and `ReallocAccountZeroed` builders. Close helpers still do not implicitly resize or zero account data.

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
#[pda(seeds = [COUNTER_SEED, authority: Address], bump = bump)]
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
