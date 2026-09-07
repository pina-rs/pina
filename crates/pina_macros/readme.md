# `pina_macros`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

<br>

Procedural macros for building Pina programs with less boilerplate.

This crate powers the attributes/derives re-exported by `pina`.

<!-- {=crateReadmeBadgeRow:"pina_macros"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina__macros-orange?logo=rust)](https://crates.io/crates/pina_macros) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina__macros-1f425f?logo=docs.rs)](https://docs.rs/pina_macros/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

## Installation

<br>

Most projects should depend on `pina` and use the re-exported macros.

Compact accounts are opt-in:

```bash
cargo add pina --features compact
```

For fixed account macros needed directly:

```bash
cargo add pina_macros
```

Add `--features compact` when invoking `#[account(compact)]` through a direct `pina_macros` dependency.

## Macros

<br>

- `#[discriminator]`: defines a typed discriminator enum (`u8`, `u16`, `u32`, `u64`).
- `#[account]`: defines discriminator-first fixed or compact account POD structs and generated builders.
- `#[instruction]`: defines discriminator-first instruction data POD structs.
- `#[event]`: defines discriminator-first event POD structs.
- `#[pda]`: defines typed PDA seed, derivation, and validation helpers.
- `#[error]`: maps custom enums to `ProgramError::Custom(code)`.
- `#[derive(Accounts)]`: parses `&mut [AccountView]` into a named struct of shared and/or mutable account references.

## Common Usage

<br>

```rust
use pina::*;

#[discriminator]
pub enum Instruction {
	Initialize = 0,
}

#[instruction(discriminator = Instruction::Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountView,
	pub state: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleError {
	InvalidAuthority = 6000,
}
```

## Attribute Options

<br>

### `#[discriminator(...)]`

<br>

- `primitive = u8|u16|u32|u64`
- `crate = ::pina` (defaults to `::pina`)
- `final` (omits `#[non_exhaustive]`)

### `#[account(...)]`, `#[instruction(...)]`, `#[event(...)]`

<br>

- `discriminator = PathToEnum`
- `variant = EnumVariant` (optional; defaults to inferred struct name; cannot be combined with a `discriminator` path that includes a variant)
- `crate = ::pina` (optional)
- `compact` (requires the crate's `compact` feature; generates checked read and patch APIs for one or more trailing compact fields)

### `#[error(...)]`

<br>

- `crate = ::pina` (optional)
- `final` (omits `#[non_exhaustive]`)

### `#[derive(Accounts)]`

<br>

- Supports one lifetime parameter.
- Supports `&'a AccountView`, `&'a mut AccountView`, `&'a [AccountView]`, and `&'a mut [AccountView]` fields.
- Supports `#[pina(remaining)]` on a single trailing field to capture remaining accounts. Mutable trailing addresses are distinct by default; `#[pina(remaining, distinct = false)]` permits duplicates only for intentionally documented instruction contracts.
- Supports `#[pina(crate = ::pina)]` on the struct to override the crate path.

## Notes

<br>

- Generated instructions, events, and ordinary accounts accept audited scalars, addresses, byte arrays, bounded `String` and `Vec` fields, and recursively fixed `Option` fields. PinaPod supplies alignment-one storage and load-bearing validation.
- `#[account(compact)]` accepts a final suffix made from `String<N>`, `Vec<T, N>` for fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`. Fixed `Option<T>` fields stay in the header. Unsupported nesting produces a compile-time error that lists these forms.
- Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` for an explicit `1`, `2`, `4`, or `8` byte prefix. Prefix widths are const arguments, not macro attributes.
- The macros are designed for `no_std` Solana program crates.
- If you use `pina`, these macros are available directly without importing `pina_macros`.
