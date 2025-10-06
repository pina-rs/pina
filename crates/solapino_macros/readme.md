# `solapino_macros`

> Derive, Attribute and Funtion macros which are used to make development with solapino easier.

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Attribute Macros

### `#[error]`

`#[error]` is a lightweight modification to the provided enum acting as syntactic sugar to make it easier to manage your custom program errors.

```rust
use solapino::*;

#[error(crate = ::solapino, final = false)]
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
#[repr(u32)]
#[non_exhaustive] // This is present if you haven't set the attribute `final` or it is set to false.
#[derive(
	::core::fmt::macros::Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
	::solapino::IntoPrimitive, /* `IntoPrimitive` is added to the derive macros */
)]
pub enum MyError {
	/// Doc comments are significant as they will be read by a future parse to
	/// generte the IDL.
	Invalid = 0,
	/// A duplicate issue has occurred.
	Duplicate = 1,
}

impl ::core::convert::From<MyError> for ::solapino::ProgramError {
	fn from(e: MyError) -> Self {
		::solapino::ProgramError::Custom(e as u32)
	}
}
```

#### Properties

- `crate` - this defaults to `::solapino` as the developer is expected to have access to the `solapino` crate in the dependencies.

- `final` - By default all error enums are marked as `non_exhaustive`. The `final` attribute will remove this. This attribute is optional.

[crate-image]: https://img.shields.io/crates/v/solapino_macros.svg
[crate-link]: https://crates.io/crates/solapino_macros
[docs-image]: https://docs.rs/solapino_macros/badge.svg
[docs-link]: https://docs.rs/solapino_macros/
[ci-status-image]: https://github.com/solapino/solapino_macros/workflows/ci/badge.svg
[ci-status-link]: https://github.com/solapino/solapino_macros/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicence-blue.svg
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/solapino/solapino_macros/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/solapino/solapino_macros
