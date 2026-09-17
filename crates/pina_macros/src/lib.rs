//! Procedural macros for Pina programs.
//!
//! Expansion code is organized by macro so each generated contract can be
//! reviewed independently. Public entry points stay here because procedural
//! macros must be exported from the crate root.
//!
//! ## Macro index
//!
//! - [`macro@account`] defines fixed or compact account data.
//! - [`macro@instruction`] defines instruction data.
//! - [`macro@event`] defines event data.
//! - [`derive@Accounts`] parses instruction accounts.
//! - [`macro@discriminator`] defines typed discriminator enums.
//! - [`macro@instruction_dispatch`] generates instruction dispatch.
//! - [`macro@pda`] defines typed PDA seeds and loaders.
//! - [`macro@error`] maps custom errors to Solana program errors.
//!
//! Enable the `validation` feature to use `#[pina(validate(...))]` with the
//! first four macros. The generated code is allocation-free syntactic sugar
//! over Pina's runtime validation traits; direct validation remains available.

use proc_macro::TokenStream;

mod account;
mod accounts;
mod args;
mod discriminator;
mod dispatch;
mod error;
mod event;
mod instruction;
mod migration;
mod pda;
mod schema;
mod support;
mod validation;

/// Parses an account slice into a named-field struct.
///
/// Fields can be shared or mutable `AccountView` references. A final slice
/// field annotated with `#[pina(remaining)]` captures all trailing accounts.
/// Mutable trailing-account addresses are distinct by default. Use
/// `#[pina(remaining, distinct = false)]` only when duplicate addresses are an
/// intentional part of the instruction contract, and document the safety
/// invariant on the field.
///
/// With the `validation` feature, fields accept `signer`, `writable`,
/// `executable`, `address`, `addresses`, `owner`, `owners`, `program`,
/// `sysvar`, `empty`, `not_empty`, `data_len`, `distinct_from`, and `error`
/// inside `#[pina(validate(...))]`. Mutable account references already enforce
/// writability. Add `#[pina(validate(with = function))]` to the struct for a
/// final cross-field hook. Parsing calls the generated `PinaValidate`
/// implementation automatically.
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
///
/// #[derive(Accounts)]
/// struct SettleAccounts<'a> {
///     authority: &'a AccountView,
///     #[pina(remaining)]
///     positions: &'a mut [AccountView],
/// }
///
/// #[derive(Accounts)]
/// struct WeightedAccounts<'a> {
///     /// Duplicate entries intentionally apply the same account's weight more than once.
///     #[pina(remaining, distinct = false)]
///     positions: &'a mut [AccountView],
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

/// Defines discriminator-first fixed or compact account data.
///
/// The macro validates Pina's closed schema grammar, derives the `PinaPod`
/// companion, and generates checked `initialize` and `try_from_bytes`
/// helpers. Add `compact` to permit a suffix of bounded `String<N>` and
/// `Vec<T, N>` fields whose active contents, rather than their full capacities,
/// occupy account data. Compact accounts require the `compact` crate feature.
///
/// With the `validation` feature, schema fields accept inclusive `min` and
/// `max` rules for integers, plus `min_len`, `max_len`, and `exact_len` for
/// bounded strings, vectors, and arrays. Add `error = ERROR` to override
/// `ProgramError::InvalidAccountData`, or add `validate(with = function)` to
/// the outer macro for a final application hook. Generated read and initialize
/// helpers validate automatically; the zero-copy view also implements
/// `PinaValidate` for explicit checks.
///
/// # Example
///
/// ```ignore
/// #[account(discriminator = AccountType::Counter)]
/// struct Counter {
///     authority: Address,
///     value: PodU64,
/// }
///
/// #[account(discriminator = AccountType::History, compact)]
/// struct History {
///     authority: Address,
///     featured: Option<u64>,
///     title: PodString<32>,
///     values: Vec<u64, 64>,
///     tags: Vec<u8, 128>,
/// }
/// ```
#[proc_macro_attribute]
pub fn account(args: TokenStream, input: TokenStream) -> TokenStream {
	account::expand(args.into(), input.into()).into()
}

/// Defines typed PDA seeds for an account struct.
///
/// `seeds` accepts byte-string constants and typed dynamic seeds. An optional
/// `bump` field enables generated stored-bump verification. Fixed `#[account]`
/// schemas with a stored bump receive `load_pda` and `load_pda_mut`. Compact
/// schemas receive `with_pda` instead because their generated views borrow
/// variable-length data. Each helper validates the account representation and
/// PDA address in one pass.
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
/// Generated helpers enforce exact length, discriminator, and `PinaPod` field
/// validation at the instruction boundary.
///
/// Enable `validation` for field-level `min`, `max`, `min_len`, `max_len`,
/// `exact_len`, and `error` rules. A `validate(with = function)` macro
/// argument adds cross-field validation. Generated helpers return
/// `ProgramError::InvalidInstructionData` by default and call validation
/// automatically after structural decoding.
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
/// Enable `validation` for field-level `min`, `max`, `min_len`, `max_len`,
/// `exact_len`, and `error` rules, plus a type-level
/// `validate(with = function)` hook. Event views implement `PinaValidate` and
/// generated read/initialize helpers validate automatically.
///
/// Add `migrations` to generate exact historical decoders and
/// `with_current_event_data`. The helper projects immutable historical bytes to
/// the current representation and supplies their source version as provenance.
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

/// Generates instruction dispatch from a discriminator enum.
///
/// Generates a `process_instruction` compatible with `nostd_entrypoint!`, plus
/// the account-count constant the hand-written entrypoint used to carry. Apply
/// it above `#[discriminator]` on the instruction enum: the outer attribute
/// expands first, and the enum it emits still carries `#[discriminator]`, so
/// the two compose into one type.
///
/// Variant `Foo` routes to `FooAccounts`. Override that per variant with
/// `#[dispatch(accounts = BarAccounts)]`.
///
/// `MAX_INSTRUCTION_ACCOUNTS` is the largest `ACCOUNT_BOUND` across every
/// routed accounts struct, saturated at `pinocchio::MAX_TX_ACCOUNTS`. A
/// `#[cfg(test)]` assertion block makes the constant verify itself: it fails
/// to compile if any route's bound exceeds the constant, which catches a route
/// that was forgotten when the enum grew.
///
/// # Arguments
///
/// - `crate = path` — the Pina crate path. Defaults to `::pina`.
/// - `program_id = EXPR` — the checked program id. Defaults to `ID`.
/// - `maximum_accounts = EXPR` — the saturation cap. Defaults to
///   `pinocchio::MAX_TX_ACCOUNTS`.
/// - `capacity_test` / `capacity_test = false` — emit or suppress the
///   self-verifying assertions. Emitted by default.
/// - `migrations(Account, ...)` — emit the reserved `Migrate` prelude,
///   listing the migratable contracts in reserved-instruction slot order.
/// - `migrations_max_lamports = EXPR` — the lamport budget shared by every
///   slot. Required with `migrations`.
/// - `inline = "hint"` — emit `#[inline]` instead of the default
///   `#[inline(always)]` on the generated dispatcher. The two spellings differ
///   at the codegen level, so match whichever the program was measured with.
///
/// # Example
///
/// ```ignore
/// #[instruction_dispatch]
/// #[discriminator]
/// pub enum CounterInstruction {
///     Initialize = 0,
///     Increment = 1,
/// }
///
/// #[derive(Accounts)]
/// pub struct InitializeAccounts<'a> { /* ... */ }
///
/// #[derive(Accounts)]
/// pub struct IncrementAccounts<'a> { /* ... */ }
///
/// #[cfg(feature = "bpf-entrypoint")]
/// pub mod entrypoint {
///     use super::*;
///
///     // Keep the default account array. Sizing it to `MAX_INSTRUCTION_ACCOUNTS`
///     // would make the loader skip accounts beyond the array instead of
///     // letting `finish_exact` reject them.
///     nostd_entrypoint!(process_instruction);
/// }
/// ```
#[proc_macro_attribute]
pub fn instruction_dispatch(args: TokenStream, input: TokenStream) -> TokenStream {
	dispatch::expand(args.into(), input.into()).into()
}
