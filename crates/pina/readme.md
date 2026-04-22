# `pina`

<br>

Core runtime crate for building Solana programs on top of [`pinocchio`](https://github.com/anza-xyz/pinocchio).

It provides zero-copy account loaders, discriminator-aware account/instruction/event modeling, account validation traits, and `no_std` entrypoint helpers.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

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
- `account-resize` only unlocks realloc helpers such as `realloc_account()` and `realloc_account_zero()`. Close helpers still do not implicitly resize or zero account data.

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

## Instruction authoring tips

<br>

<!-- {=pinaInstructionAuthoringTips} -->

- Entry points should accept `&mut [AccountView]` and dispatch with `Accounts::try_from(accounts)?.process(data)`.
- Use `&AccountView` for read-only accounts and `&mut AccountView` only when you need mutable loaders, direct lamport mutation, `close_*` helpers, or writable IDL inference.
- Keep `assert_writable()` explicit even on `&mut AccountView`. Type-level mutability unlocks mutable APIs, but the runtime still decides whether the account is writable for the current instruction.
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

<!-- {=pinaBadgeLinks} -->

[crate-image]: https://img.shields.io/crates/v/pina.svg?style=flat-square
[crate-link]: https://crates.io/crates/pina
[docs-image]: https://docs.rs/pina/badge.svg
[docs-link]: https://docs.rs/pina/
[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicense-blue.svg?style=flat-square
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina

<!-- {/pinaBadgeLinks} -->
