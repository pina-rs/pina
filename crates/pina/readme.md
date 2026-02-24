# `pina`

Core runtime crate for building Solana programs on top of [`pinocchio`](https://github.com/anza-xyz/pinocchio).

It provides zero-copy account loaders, discriminator-aware account/instruction/event modeling, account validation traits, and `no_std` entrypoint helpers.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

```bash
cargo add pina
```

Enable optional token helpers:

```bash
cargo add pina --features token
```

## What This Crate Includes

- `nostd_entrypoint!` for `no_std` Solana entrypoint wiring.
- `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` integration via the default `derive` feature.
- Validation chains on `AccountView` (`assert_signer`, `assert_writable`, `assert_owner`, PDA checks, sysvar checks, and more).
- Zero-copy POD wrappers (`PodU*`, `PodI*`, `PodBool`) for stable on-chain layouts.
- CPI helpers for system/token operations.

## Feature Flags

- `derive` (default): enables `pina_macros` re-exports.
- `logs` (default): enables Solana log macros via `solana-program-log`.
- `token`: enables SPL token/token-2022 and ATA helpers.

## Minimal Program Skeleton

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
	accounts: &[AccountView],
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

## Related Crates

- [`pina_macros`](https://docs.rs/pina_macros): proc-macro implementations for the attributes and derives used here.
- [`pina_cli`](https://docs.rs/pina_cli): CLI/library used to generate Codama IDLs from Pina programs.
- [`pina_sdk_ids`](https://docs.rs/pina_sdk_ids): shared Solana program/sysvar IDs.

## Codama IDLs

`pina` models are designed to be extracted into Codama IDLs through `pina_cli`.

```bash
pina idl --path ./my_program --output ./idls/my_program.json
```

From there you can generate JS clients with Codama renderers, or Pina-style Rust clients using this repository's `pina_codama_renderer` tool.

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
