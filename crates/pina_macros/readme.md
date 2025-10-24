# `pina_macros`

> Derive, Attribute and Funtion macros which are used to make development with pina easier.

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Attribute Macros

### `#[discriminator]`

This attribute macro should be used for annotating the globally shared instruction and account discriminators.

#### Properties

- `primitive` - Defaults to `u8` which takes up 1 byte of space for the discriminator. This would allow up to 256 variations of the type being discriminated. The type can be the following:
  - `u8` - 256 variations
  - `u16` - 65,536 variations
  - `u32` - 4,294,967,296 variations
  - `u64` - 18,446,744,073,709,551,616 variations (overkill!)
- `crate` - this defaults to `::pina` as the developer is expected to have access to the `pina` crate in the dependencies.
- `final` - By default all discriminator enums are marked as `non_exhaustive`. The `final` flag will remove this annotation.

#### Codegen

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
	::core::fmt::Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
)]
pub enum MyAccount {
	ConfigState = 0,
	GameState = 1,
	SectionState = 2,
}

impl ::core::convert::From<MyAccount> for u8 {
	#[inline]
	fn from(enum_value: TryIt) -> Self {
		enum_value as Self
	}
}

impl ::core::convert::TryFrom<u8> for MyAccount {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __CONFIG_STATE: u8 = 0;
		const __GAME_STATE: u8 = 1;
		const __SECTION_STATE: u8 = 2;
		#[deny(unreachable_patterns)]
		match number {
			__CONFIG_STATE => ::core::result::Result::Ok(Self::ConfigState),
			__GAME_STATE => ::core::result::Result::Ok(Self::GameState),
			__SECTION_STATE => ::core::result::Result::Ok(Self::SectionState),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}

unsafe impl Zeroable for MyAccount {}
unsafe impl Pod for MyAccount {}
::pina::into_discriminator!(MyAccount, u8);
```

### `#[account]`

The account macro is used to annotate account data that will exist within a solana account.

#### Properties

- `crate` - this defaults to `::pina` as the developer is expected to have access to the `pina` crate in the dependencies.
- `discriminator` - the discriminator enum to use for this account. The variant should match the name of the account struct.

#### Codegen

It will transform the following:

```rust
use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyAccount {
	ConfigState = 0,
	GameState = 1,
	SectionState = 2,
}

#[account(crate = ::pina, discriminator = MyAccount)]
#[derive(Debug)]
pub struct ConfigState {
	/// The version of the state.
	pub version: u8,
	/// The authority which can update this config.
	pub authority: Pubkey,
	/// Store the bump to save compute units.
	pub bump: u8,
	/// The treasury account bump where fees are sent and where the minted
	/// tokens are transferred.
	pub treasury_bump: u8,
	/// The mint account bump.
	pub mint_bit_bump: u8,
	/// The mint account bump for KIBIBIT.
	pub mint_kibibit_bump: u8,
	/// The mint account bump for MEBIBIT.
	pub mint_mebibit_bump: u8,
	/// The mint account bump for GIBIBIT.
	pub mint_gibibit_bump: u8,
	/// There will be a maximum of 8 games.
	pub game_index: u8,
}
```

Into:

```rust
use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyAccount {
	ConfigState = 0,
	GameState = 1,
	SectionState = 2,
}

#[repr(C)]
#[derive(
	Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
	::pina::Pod,
	::pina::Zeroable,
	::pina::TypedBuilder,
)]
#[builder(builder_method(vis = "", name = __builder))]
#[bytemuck(crate = "::pina::bytemuck")]
pub struct ConfigState {
	// This discriminator is automatically injected as the first field in the struct. It must be
	// present.
	discriminator: [u8; MyAccount::BYTES],
	/// The version of the state.
	pub version: u8,
	/// The authority which can update this config.
	pub authority: Pubkey,
	/// Store the bump to save compute units.
	pub bump: u8,
	/// The treasury account bump where fees are sent and where the minted
	/// tokens are transferred.
	pub treasury_bump: u8,
	/// The mint account bump.
	pub mint_bit_bump: u8,
	/// The mint account bump for KIBIBIT.
	pub mint_kibibit_bump: u8,
	/// The mint account bump for MEBIBIT.
	pub mint_mebibit_bump: u8,
	/// The mint account bump for GIBIBIT.
	pub mint_gibibit_bump: u8,
	/// There will be a maximum of 8 games.
	pub game_index: u8,
}

// This type is generated to match the `TypedBuilder` type with the
// discriminator already set.
type ConfigStateBuilderType = ConfigStateBuilder<(
	([u8; MyAccount::BYTES],), /* `discriminator`: automatically applied in the builder method
	                            * below. */
	(), // `version`
	(), // `authority`
	(), // `bump`
	(), // `treasury_bump`
	(), // `mint_bit_bump`
	(), // `mint_kibibit_bump`
	(), // `mint_mebibit_bump`
	(), // `mint_gibibit_bump`
	(), // `game_index`
)>;

impl ConfigState {
	pub fn to_bytes(&self) -> &[u8] {
		::pina::bytemuck::bytes_of(self)
	}

	pub fn builder() -> ConfigStateBuilderType {
		let mut bytes = [0u8; MyAccount::BYTES];
		<Self as ::pina::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);

		Self::__builder().discriminator(bytes)
	}
}

impl ::pina::HasDiscriminator for ConfigState {
	type Type = MyAccount;

	const VALUE: Self::Type = MyAccount::ConfigState;
}

impl ::pina::AccountValidation for ConfigState {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}

		::pina::log!("Account is invalid");
		::pina::log_caller();

		Err(::pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match ::pina::assert(
			condition(self),
			::pina::ProgramError::InvalidAccountData,
			msg,
		) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			return Ok(self);
		}

		::pina::log!("Account is invalid");
		::pina::log_caller();

		Err(::pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match ::pina::assert(
			condition(self),
			::pina::ProgramError::InvalidAccountData,
			msg,
		) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
```

### `#[instruction]`

The instruction macro is used to annotate instruction data that will exist within a solana instruction.

#### Properties

- `discriminator` - the discriminator enum to use for this instruction. The variant should match the name of the instruction struct.

#### Codegen

It will transform the following:

```rust
use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyInstruction {
	Add = 0,
	FlipBit = 1,
}

#[instruction(crate = ::pina, discriminator = MyInstruction)]
#[derive(Debug)]
pub struct FlipBit {
	/// The data section being updated.
	pub section_index: u8,
	/// The index of the `u16` value in the array.
	pub array_index: u8,
	/// The offset of the bit being set.
	pub offset: u8,
	/// The value to set the bit to: `0` or `1`.
	pub value: u8,
}
```

Is transformed to:

```rust
use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyInstruction {
	Add = 0,
	FlipBit = 1,
}

#[repr(C)]
#[derive(
	Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
	::pina::Pod,
	::pina::Zeroable,
	::pina::TypedBuilder,
)]
#[builder(builder_method(vis = "", name = __builder))]
#[bytemuck(crate = "::pina::bytemuck")]
pub struct FlipBit {
	// This discriminator is automatically injected as the first field in the struct. It must be
	// present.
	discriminator: [u8; MyInstruction::BYTES],
	/// The data section being updated.
	pub section_index: u8,
	/// The index of the `u16` value in the array.
	pub array_index: u8,
	/// The offset of the bit being set.
	pub offset: u8,
	/// The value to set the bit to: `0` or `1`.
	pub value: u8,
}

// This type is generated to match the `TypedBuilder` type with the
// discriminator already set.
type FlipBitBuilderType = FlipBitBuilder<(
	([u8; MyInstruction::BYTES],), /* `discriminator`: automatically applied in the builder
	                                * method below. */
	(), // `section_index`
	(), // `array_index`
	(), // `offset`
	(), // `value`
)>;

impl FlipBit {
	pub fn to_bytes(&self) -> &[u8] {
		::pina::bytemuck::bytes_of(self)
	}

	pub fn try_from_bytes(data: &[u8]) -> Result<&Self, ::pina::ProgramError> {
		::pina::bytemuck::try_from_bytes::<Self>(data)
			.or(Err(::pina::ProgramError::InvalidInstructionData))
	}

	pub fn builder() -> FlipBitBuilderType {
		let mut bytes = [0u8; MyInstruction::BYTES];
		<Self as ::pina::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);

		Self::__builder().discriminator(bytes)
	}
}

impl ::pina::HasDiscriminator for FlipBit {
	type Type = MyInstruction;

	const VALUE: Self::Type = MyInstruction::FlipBit;
}
```

### `#[event]`

Annotates a struct as an event.

#### Properties

- `crate` - this defaults to `::pina` as the developer is expected to have access to the `pina` crate in the dependencies.
- `discriminator` - the discriminator enum to use for this event. The variant should match the name of the account struct.

#### Codegen

```rust
use pina::*;

#[discriminator(primitive = u8)]
pub enum Event {
	Initialize = 0,
	Abandon = 1,
}

#[event(discriminator = Event, variant = Initialize)]
pub struct InitializeEvent {
	pub choice: u8,
}
```

Is transformed into:

```rust
use pina::*;

#[discriminator(primitive = u8)]
pub enum Event {
	Initialize = 0,
	Abandon = 1,
}

#[repr(C)]
#[derive(
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
	::pina::Pod,
	::pina::Zeroable,
	::pina::TypedBuilder,
)]
#[builder(builder_method(vis = "", name = __builder))]
#[bytemuck(crate = "::pina::bytemuck")]
pub struct InitializeEvent {
	// This discriminator is automatically injected as the first field in the struct. It must be
	// present.
	discriminator: [u8; Event::BYTES],
	pub choice: u8,
}

// This type is generated to match the `TypedBuilder` type with the
// discriminator already set.
type InitializeEventBuilderType = InitializeEventBuilder<(
	([u8; Event::BYTES],), /* `discriminator`: automatically applied in the builder
	                        * method below. */
	(), // `choice`
)>;

impl InitializeEvent {
	pub fn to_bytes(&self) -> &[u8] {
		::pina::bytemuck::bytes_of(self)
	}

	pub fn try_from_bytes(data: &[u8]) -> Result<&Self, ::pina::ProgramError> {
		::pina::bytemuck::try_from_bytes::<Self>(data)
			.or(Err(::pina::ProgramError::InvalidInstructionData))
	}

	pub fn builder() -> InitializeEventBuilderType {
		let mut bytes = [0u8; Event::BYTES];
		<Self as ::pina::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);

		Self::__builder().discriminator(bytes)
	}
}

impl ::pina::HasDiscriminator for InitializeEvent {
	type Type = MyInstruction;

	const VALUE: Self::Type = MyInstruction::FlipBit;
}
```

### `#[error]`

`#[error]` is a lightweight modification to the provided enum acting as syntactic sugar to make it easier to manage your custom program errors.

#### Properties

- `crate` - this defaults to `::pina` as the developer is expected to have access to the `pina` crate in the dependencies.
- `final` - By default all error enums are marked as `non_exhaustive`. The `final` flag will remove this.

#### Codegen

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
	::core::fmt::Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
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
```

## Derive Macros

### `#[derive(Accounts)]`

This adds a `TryFrom` implementation to a struct of `AccountInfo`'s.

#### Properties

- `crate` - this defaults to `::pina` as the developer is expected to have access to the `pina` crate in the dependencies.
- `remaining` - a field level annotation that annotates the field as containing all the remaining accounts not specified in the struct. If not specified then the exact number of struct fields must be equal to the exact number of items in the provided `AccountInfo` slice.

#### Codegen

```rust
use pina::*;

#[derive(Accounts)]
#[pina(crate = ::pina)]
pub struct MakeOfferAccounts<'a> {
	pub maker: &'a AccountInfo,
	pub token_mint_a: &'a AccountInfo,
	pub token_mint_b: &'a AccountInfo,
	pub maker_ata_a: &'a AccountInfo,
	pub offer: &'a AccountInfo,
	pub vault: &'a AccountInfo,
	pub token_program: &'a AccountInfo,
	// If this is not present then the struct expects to consume all provided accounts.
	#[pina(remaining)]
	pub remaining: &'a [AccountInfo],
}
```

Into:

```rust
use pina::*;

pub struct MakeOfferAccounts<'a> {
	pub maker: &'a AccountInfo,
	pub token_mint_a: &'a AccountInfo,
	pub token_mint_b: &'a AccountInfo,
	pub maker_ata_a: &'a AccountInfo,
	pub offer: &'a AccountInfo,
	pub vault: &'a AccountInfo,
	pub token_program: &'a AccountInfo,
	pub remaining: &'a [AccountInfo],
}

impl<'a> ::pina::TryFromAccountInfos<'a> for MakeOfferAccounts<'a> {
	fn try_from_account_infos(
		accounts: &'a [::pina::AccountInfo],
	) -> ::core::result::Result<Self, ::pina::ProgramError> {
		let [maker, token_mint_a, token_mint_b, maker_ata_a, offer, vault, token_program, remaining @ ..] =
			accounts
		else {
			return ::core::result::Result::Err(::pina::ProgramError::NotEnoughAccountKeys);
		};

		Ok(Self {
			maker,
			token_mint_a,
			token_mint_b,
			maker_ata_a,
			offer,
			vault,
			token_program,
			remaining,
		})
	}
}

impl<'a> ::core::convert::TryFrom<&'a [::pina::AccountInfo]> for MakeOfferAccounts<'a> {
	type Error = ::pina::ProgramError;

	fn try_from(accounts: &'a [::pina::AccountInfo]) -> ::core::result::Result<Self, Self::Error> {
		<Self as ::pina::TryFromAccountInfos>::try_from_account_infos(accounts)
	}
}
```

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

```
```
