# `pina_macros`

Procedural macros for building Pina programs with less boilerplate.

This crate powers the attributes/derives re-exported by `pina`.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

Most projects should depend on `pina` and use the re-exported macros.

If needed directly:

```bash
cargo add pina_macros
```

## Macros

- `#[discriminator]`: defines a typed discriminator enum (`u8`, `u16`, `u32`, `u64`).
- `#[account]`: defines discriminator-first account POD structs and generated builders.
- `#[instruction]`: defines discriminator-first instruction data POD structs.
- `#[event]`: defines discriminator-first event POD structs.
- `#[error]`: maps custom enums to `ProgramError::Custom(code)`.
- `#[derive(Accounts)]`: parses `&[AccountView]` into a named struct.

## Common Usage

```rust
use pina::*;

#[discriminator]
pub enum Instruction {
	Initialize = 0,
}

#[instruction(discriminator = Instruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountView,
	pub state: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleError {
	InvalidAuthority = 6000,
}
```

## Attribute Options

### `#[discriminator(...)]`

- `primitive = u8|u16|u32|u64`
- `crate = ::pina` (defaults to `::pina`)
- `final` (omits `#[non_exhaustive]`)

### `#[account(...)]`, `#[instruction(...)]`, `#[event(...)]`

- `discriminator = PathToEnum`
- `variant = EnumVariant` (optional; defaults to inferred struct name)
- `crate = ::pina` (optional)

### `#[error(...)]`

- `crate = ::pina` (optional)
- `final` (omits `#[non_exhaustive]`)

### `#[derive(Accounts)]`

- Supports one lifetime parameter.
- Supports `#[pina(remaining)]` on a single trailing field to capture remaining accounts.
- Supports `#[pina(crate = ::pina)]` on the struct to override the crate path.

## Notes

- Generated account/instruction/event structs are intended for fixed-size, bytemuck-safe layouts.
- The macros are designed for `no_std` Solana program crates.
- If you use `pina`, these macros are available directly without importing `pina_macros`.

[crate-image]: https://img.shields.io/crates/v/pina_macros.svg?style=flat-square
[crate-link]: https://crates.io/crates/pina_macros
[docs-image]: https://docs.rs/pina_macros/badge.svg
[docs-link]: https://docs.rs/pina_macros/
[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicense-blue.svg?style=flat-square
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina
