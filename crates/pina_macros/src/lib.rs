//! Procedural macros for Pina programs.
//!
//! Expansion code is organized by macro so each generated contract can be
//! reviewed independently. Public entry points stay here because procedural
//! macros must be exported from the crate root.

use proc_macro::TokenStream;

mod account;
mod accounts;
mod args;
mod discriminator;
mod error;
mod event;
mod instruction;
mod pda;
mod schema;
mod support;

#[cfg(test)]
mod tests;

#[cfg(test)]
use account::expand as account_impl;
#[cfg(test)]
use accounts::expand as accounts_derive_impl;
#[cfg(test)]
use discriminator::expand as discriminator_impl;
#[cfg(test)]
use error::expand as error_impl;
#[cfg(test)]
use event::expand as event_impl;
#[cfg(test)]
use instruction::expand as instruction_impl;
#[cfg(test)]
use pda::expand as pda_impl;

/// Parses an account slice into a named-field struct.
///
/// Fields can be shared or mutable `AccountView` references. A final slice
/// field annotated with `#[pina(remaining)]` captures all trailing accounts.
///
/// # Example
///
/// ```ignore
/// #[derive(Accounts)]
/// struct InitializeAccounts<'a> {
///     payer: &'a AccountView,
///     state: &'a mut AccountView,
///     #[pina(remaining)]
///     remaining: &'a [AccountView],
/// }
/// ```
#[proc_macro_derive(Accounts, attributes(pina))]
pub fn accounts_derive(input: TokenStream) -> TokenStream {
	accounts::expand(input.into()).into()
}

/// Defines a typed discriminator enum.
///
/// Every variant must have an explicit value. The storage primitive defaults
/// to `u8` and can be set to `u16`, `u32`, or `u64`.
///
/// # Example
///
/// ```ignore
/// #[discriminator]
/// enum Instruction {
///     Initialize = 0,
///     Update = 1,
/// }
/// ```
#[proc_macro_attribute]
pub fn discriminator(args: TokenStream, input: TokenStream) -> TokenStream {
	discriminator::expand(args.into(), input.into()).into()
}

/// Defines discriminator-first, fixed-size account data.
///
/// The macro validates Pina's closed schema grammar, derives the zeropod
/// companion, and generates checked `initialize` and `try_from_bytes`
/// helpers.
///
/// # Example
///
/// ```ignore
/// #[account(discriminator = AccountType::Counter)]
/// struct Counter {
///     authority: Address,
///     value: PodU64,
/// }
/// ```
#[proc_macro_attribute]
pub fn account(args: TokenStream, input: TokenStream) -> TokenStream {
	account::expand(args.into(), input.into()).into()
}

/// Defines typed PDA seeds for an account struct.
///
/// `seeds` accepts byte-string constants and typed dynamic seeds. An optional
/// `bump` field enables generated stored-bump verification.
///
/// # Example
///
/// ```ignore
/// #[pda(seeds = [b"counter", authority: Address], bump = bump)]
/// struct Counter {
///     authority: Address,
///     bump: u8,
/// }
/// ```
#[proc_macro_attribute]
pub fn pda(args: TokenStream, input: TokenStream) -> TokenStream {
	pda::expand(args.into(), input.into()).into()
}

/// Defines discriminator-first, fixed-size instruction data.
///
/// Generated helpers enforce exact length, discriminator, and zeropod field
/// validation at the instruction boundary.
///
/// # Example
///
/// ```ignore
/// #[instruction(discriminator = Instruction::Initialize)]
/// struct InitializeInstruction {
///     bump: u8,
/// }
/// ```
#[proc_macro_attribute]
pub fn instruction(args: TokenStream, input: TokenStream) -> TokenStream {
	instruction::expand(args.into(), input.into()).into()
}

/// Defines discriminator-first, fixed-size event data.
///
/// Event payloads use the same checked schema and byte-view helpers as
/// instruction payloads.
///
/// # Example
///
/// ```ignore
/// #[event(discriminator = EventType::Initialized)]
/// struct InitializedEvent {
///     authority: Address,
/// }
/// ```
#[proc_macro_attribute]
pub fn event(args: TokenStream, input: TokenStream) -> TokenStream {
	event::expand(args.into(), input.into()).into()
}

/// Maps a custom error enum to `ProgramError::Custom`.
///
/// The enum uses `repr(u32)` and is non-exhaustive unless the `final`
/// argument is present.
///
/// # Example
///
/// ```ignore
/// #[error]
/// enum ProgramError {
///     InvalidAuthority = 6000,
/// }
/// ```
#[proc_macro_attribute]
pub fn error(args: TokenStream, input: TokenStream) -> TokenStream {
	error::expand(args.into(), input.into()).into()
}
