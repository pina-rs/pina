# pina

A performant Solana smart contract framework built on top of [pinocchio](https://github.com/anza-xyz/pinocchio) — a zero-dependency alternative to `solana-program` that massively reduces compute units and dependency bloat.

## Features

- **Zero-copy deserialization** — account data is reinterpreted in place via `bytemuck`, with no heap allocation.
- **`no_std` compatible** — all crates compile to the `bpfel-unknown-none` SBF target for on-chain deployment.
- **Low compute units** — built on `pinocchio` instead of `solana-program`, saving thousands of CU per instruction.
- **Discriminator system** — every account, instruction, and event type carries a typed discriminator as its first field.
- **Validation chaining** — chain assertions on `AccountInfo` references:
  ```rust
  account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?;
  ```
- **Proc-macro sugar** — `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` eliminate boilerplate.
- **CPI helpers** — PDA account creation, lamport transfers, and token operations.

## Installation

```sh
cargo add pina
```

To enable SPL token support:

```sh
cargo add pina --features token
```

## Quick start

```rust
use pina::*;

// Define a discriminator enum for your instructions.
#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
	Update = 1,
}

// Define instruction data.
#[instruction(discriminator = MyInstruction)]
pub struct Initialize {
	pub value: u8,
}

// Define your accounts struct.
#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountInfo,
	pub state: &'a AccountInfo,
	pub system_program: &'a AccountInfo,
}

// Wire up the entrypoint.
nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Pubkey,
	accounts: &[AccountInfo],
	data: &[u8],
) -> ProgramResult {
	let instruction: MyInstruction = parse_instruction(program_id, &ID, data)?;
	match instruction {
		MyInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
		MyInstruction::Update => {
			// ...
			Ok(())
		}
	}
}
```

## Crates

| Crate                                                             | Description                                                                                                           |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| [`pina`](crates/pina)                                             | Core framework — traits, account loaders, CPI helpers, Pod types, macros.                                             |
| [`pina_macros`](crates/pina_macros)                               | Proc-macro crate — `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, `#[derive(Accounts)]`. |
| [`pina_sdk_ids`](crates/pina_sdk_ids)                             | Well-known Solana program and sysvar IDs.                                                                             |
| [`pina_token_2022_extensions`](crates/pina_token_2022_extensions) | Token 2022 extension parsing _(slated for deprecation)_.                                                              |

## Ideology

- Macros are minimal syntactic sugar to reduce repetition of code.
- IDL generation is automated based on code you write, rather than annotations. So `payer.assert_signer()?` will generate an IDL that specifies that the account is a signer.
- Everything in Rust from the on-chain program to the client code used on the browser — this project strives to make it possible to build everything in your favourite language.

## Examples

See the [escrow program](examples/escrow_program) for a complete reference implementation of a token escrow using pina.

## Contributing

Contributions are welcome! Please open an issue or pull request on [GitHub](https://github.com/ifiokjr/pina).

## License

Licensed under the [Apache License, Version 2.0](license).
