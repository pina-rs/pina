//! # pina
//!
//! A performant Solana smart contract framework built on top of
//! [`pinocchio`](https://docs.rs/pinocchio) — a lightweight alternative to
//! `solana-program` that massively reduces dependency bloat and compute units.
//!
//! ## Features
//!
//! - **Zero-copy account deserialization** via `bytemuck` — no heap allocation.
//! - **`no_std` compatible** — designed for on-chain deployment to the SBF
//!   target.
//! - **Discriminator system** — every account, instruction, and event type
//!   carries a discriminator as its first field, enabling safe type
//!   identification.
//! - **Validation chaining** — chain assertions on `AccountView` references
//!   (e.g. `account.assert_signer()?.assert_writable()?.assert_owner(&id)?`).
//! - **Proc-macro sugar** — `#[account]`, `#[instruction]`, `#[event]`,
//!   `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` reduce
//!   boilerplate.
//! - **CPI helpers** — account creation, PDA allocation, and token operations.
//!
//! ## Crate features
//!
//! - `logs` *(default)* — enables on-chain logging via `solana-program-log`.
//! - `derive` *(default)* — enables the `pina_macros` proc-macro crate.
//! - `token` — enables SPL token / token-2022 helpers and associated token
//!   account utilities.

#![no_std]
#![allow(clippy::inline_always)]

mod cpi;
mod error;
pub mod introspection;
mod loaders;
mod pda;
mod pod;
mod traits;
mod utils;

/// Re-export of the [`bytemuck`] crate for zero-copy serialization.
pub use bytemuck;
/// Marker trait for types that can be safely cast from any byte pattern.
pub use bytemuck::Pod;
/// Marker trait for types that can be safely zeroed.
pub use bytemuck::Zeroable;
/// Re-export all proc macros from `pina_macros` when the `derive` feature is
/// enabled.
#[cfg(feature = "derive")]
pub use pina_macros::*;
/// Macro for implementing bidirectional conversion between Pod wrappers and
/// standard integers.
pub use pina_pod_primitives::impl_int_conversion;
/// Re-export of the [`pinocchio`] crate for low-level Solana program
/// primitives.
pub use pinocchio;
/// A Solana account as seen by the runtime during instruction execution.
pub use pinocchio::AccountView;
/// A 32-byte Solana public key / address.
pub use pinocchio::Address;
/// The result type returned by Solana program entrypoints and instruction
/// handlers.
pub use pinocchio::ProgramResult;
/// Number of bytes in a Solana address (32).
pub use pinocchio::address::ADDRESS_BYTES;
/// Maximum length in bytes of a single PDA seed.
pub use pinocchio::address::MAX_SEED_LEN;
/// Maximum number of seeds allowed when deriving a PDA.
pub use pinocchio::address::MAX_SEEDS;
/// A single seed byte slice used in PDA signing.
pub use pinocchio::cpi::Seed;
/// A set of seeds that identifies a PDA signer for CPI calls.
pub use pinocchio::cpi::Signer;
/// The Solana program entrypoint attribute.
pub use pinocchio::entrypoint;
/// Error type returned by Solana programs.
pub use pinocchio::error::ProgramError;
/// An account reference passed as part of an instruction.
pub use pinocchio::instruction::InstructionAccount;
/// A view of a cross-program invocation instruction.
pub use pinocchio::instruction::InstructionView;
/// Macro for declaring a Solana program entrypoint.
pub use pinocchio::program_entrypoint;
/// Solana sysvar access utilities.
pub use pinocchio::sysvars;
/// Re-export of `pinocchio_associated_token_account` for ATA operations.
#[cfg(feature = "token")]
pub use pinocchio_associated_token_account as associated_token_account;
/// Re-export of `pinocchio_system` for system program CPI helpers.
pub use pinocchio_system as system;
/// Re-export of `pinocchio_token` for SPL Token program CPI helpers.
#[cfg(feature = "token")]
pub use pinocchio_token as token;
/// Re-export of `pinocchio_token_2022` for Token-2022 program CPI helpers.
#[cfg(feature = "token")]
pub use pinocchio_token_2022 as token_2022;
/// Alignment-safe Pod primitive wrappers (`PodBool`, `PodU16`, `PodU64`,
/// etc.).
pub use pod::*;
/// Macro for creating a compile-time [`Address`] from a base-58 string
/// literal.
pub use solana_address::address;
/// Macro for declaring a program ID constant with associated `ID` and `id()`
/// items.
pub use solana_address::declare_id;
/// Re-export of `solana_program_log` for on-chain logging utilities.
#[cfg(feature = "logs")]
pub use solana_program_log;
/// A logger instance for formatting on-chain log messages.
#[cfg(feature = "logs")]
pub use solana_program_log::Logger;
/// Logs the current compute unit usage to the Solana runtime.
#[cfg(feature = "logs")]
pub use solana_program_log::log_cu_usage;
/// Re-export of the [`typed_builder`] crate for compile-time checked builder
/// patterns.
pub use typed_builder;
/// Derive macro that generates a type-safe builder with compile-time field
/// checking.
pub use typed_builder::TypedBuilder;

/// CPI helpers for account creation, PDA allocation, and account closure.
pub use crate::cpi::*;
/// Built-in framework error types.
pub use crate::error::*;
/// PDA (Program Derived Address) derivation and verification functions.
pub use crate::pda::*;
/// Core traits for account validation, deserialization, and instruction
/// processing.
pub use crate::traits::*;
/// Utility functions for instruction parsing, assertions, and token address
/// derivation.
pub use crate::utils::*;

/// Sets up a `no_std` Solana program entrypoint.
///
/// This macro wires up the BPF entrypoint, disables the default allocator, and
/// installs a minimal panic handler. The entry function receives:
///
/// ```ignore
/// fn process_instruction(
///     program_id: &Address,
///     accounts: &[AccountView],
///     data: &[u8],
/// ) -> ProgramResult
/// ```
///
/// Usage:
///
/// ```ignore
/// nostd_entrypoint!(process_instruction);
/// ```
///
/// An optional second argument overrides the maximum number of transaction
/// accounts (defaults to `pinocchio::MAX_TX_ACCOUNTS`).
#[macro_export]
macro_rules! nostd_entrypoint {
	($process_instruction:expr) => {
		$crate::nostd_entrypoint!($process_instruction, { $crate::pinocchio::MAX_TX_ACCOUNTS });
	};
	($process_instruction:expr, $maximum:expr) => {
		$crate::pinocchio::program_entrypoint!($process_instruction, $maximum);
		$crate::pinocchio::no_allocator!();
		$crate::pinocchio::nostd_panic_handler!();
	};
}

/// Logs a message to the Solana runtime.
///
/// Supports two forms:
/// - `log!("simple string literal")` — works in all crates
/// - `log!("format: {}", value)` — works in pina and crates that depend on
///   `solana-program-log` directly (the proc macro generates absolute paths)
///
/// When the `logs` feature is disabled this is a no-op that compiles to
/// nothing.
#[cfg(feature = "logs")]
#[macro_export]
macro_rules! log {
	($msg:literal) => {
		$crate::solana_program_log::logger::log_message($msg.as_bytes())
	};
	($($arg:tt)*) => {
		$crate::solana_program_log::log!($($arg)*);
	};
}

#[cfg(not(feature = "logs"))]
#[macro_export]
macro_rules! log {
	($($arg:tt)*) => {};
}

/// Re-exports commonly used traits and helpers for instruction modules.
///
/// `use pina::prelude::*;` is the recommended import style inside on-chain
/// modules that want validation traits without long import lists.
///
/// <!-- {=pinaMdtManagedDocNote|trim|linePrefix:"/// ":true} -->/// This section is synchronized by `mdt` from `api-docs.t.md`.<!-- {/pinaMdtManagedDocNote} -->
pub mod prelude {
	#[cfg(feature = "logs")]
	pub use solana_program_log::Logger;

	pub use crate::traits::*;
}
