# `pina_macros`

> Derive, Attribute and Funtion macros which are used to make development with pina easier.

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Attribute Macros

### `#[discriminator]`

This attribut macro should be used for annotating the globally shared instruction and account discriminators.

#### Attributes

- `primitive` - Defaults to `u8` which takes up 1 byte of space for the discriminator. This would allow up to 256 variations of the type being discriminated. The the type can be the following:
  - `u8` - 256 variations
  - `u16` - 65,536 variations
  - `u32` - 4,294,967,296 variations
  - `u64` - 18,446,744,073,709,551,616 variations (overkill!)
- `crate` - this defaults to `::pina` as the developer is expected to have access to the `pina` crate in the dependencies.
- `final` - By default all error enums are marked as `non_exhaustive`. The `final` flag will remove this annotation.

The following:

```rust
use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyAccount {
	ConfigState = 0,
	GameState = 1,
	SectionState = 2,
}
```

Is transformed to:

```rust
use pina::*;

#[repr(u8)]
#[derive(
	::core::fmt::macros::Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
	::pina::IntoPrimitive,
	::pina::TryFromPrimitive,
)]
pub enum MyAccount {
	ConfigState = 0,
	GameState = 1,
	SectionState = 2,
}

::pina::into_discriminator!(MyAccount, u8);
```

### `#[error]`

`#[error]` is a lightweight modification to the provided enum acting as syntactic sugar to make it easier to manage your custom program errors.

```rust
use pina::*;

#[error(crate = ::pina)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyError {
	/// Doc comments are significant as they will be read by a future parse to
	/// generte the IDL.
	Invalid = 0,
	/// A duplicate issue has occurred.
	Duplicate = 1,
}
```

The above is transformed into:

```rust
#[non_exhaustive] // This is present if you haven't set the flag`final`.
#[derive(
	::core::fmt::macros::Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
	::pina::IntoPrimitive, /* `IntoPrimitive` is added to the derive macros */
)]
#[repr(u32)]
pub enum MyError {
	/// Doc comments are significant as they will be read by a future parse to
	/// generte the IDL.
	Invalid = 0,
	/// A duplicate issue has occurred.
	Duplicate = 1,
}

impl ::core::convert::From<MyError> for ::pina::ProgramError {
	fn from(e: MyError) -> Self {
		::pina::pinocchio::program_error::ProgramError::Custom(e as u32)
	}
}

unsafe impl Zeroable for MyError {}
unsafe impl Pod for MyError {}
```

#### Properties

- `crate` - this defaults to `::pina` as the developer is expected to have access to the `pina` crate in the dependencies.
- `final` - By default all error enums are marked as `non_exhaustive`. The `final` flag will remove this.

[crate-image]: https://img.shields.io/crates/v/pina_macros.svg
[crate-link]: https://crates.io/crates/pina_macros
[docs-image]: https://docs.rs/pina_macros/badge.svg
[docs-link]: https://docs.rs/pina_macros/
[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicence-blue.svg
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina
