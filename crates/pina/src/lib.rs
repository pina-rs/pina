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
mod loaders;
mod pda;
mod pod;
mod traits;
mod utils;

pub use bytemuck;
pub use bytemuck::Pod;
pub use bytemuck::Zeroable;
#[cfg(feature = "derive")]
pub use pina_macros::*;
pub use pinocchio;
pub use pinocchio::AccountView;
pub use pinocchio::Address;
pub use pinocchio::ProgramResult;
pub use pinocchio::address::ADDRESS_BYTES;
pub use pinocchio::address::MAX_SEEDS;
pub use pinocchio::address::MAX_SEED_LEN;
pub use pinocchio::cpi::Seed;
pub use pinocchio::cpi::Signer;
pub use pinocchio::entrypoint;
pub use pinocchio::error::ProgramError;
pub use pinocchio::instruction::InstructionAccount;
pub use pinocchio::instruction::InstructionView;
pub use pinocchio::program_entrypoint;
pub use pinocchio::sysvars;
pub use pod::*;
#[cfg(feature = "token")]
pub use pinocchio_associated_token_account as associated_token_account;
pub use pinocchio_system as system;
#[cfg(feature = "token")]
pub use pinocchio_token as token;
#[cfg(feature = "token")]
pub use pinocchio_token_2022 as token_2022;
pub use solana_address::address;
pub use solana_address::declare_id;
#[cfg(feature = "logs")]
pub use solana_program_log;
#[cfg(feature = "logs")]
pub use solana_program_log::Logger;
#[cfg(feature = "logs")]
pub use solana_program_log::log_cu_usage;
pub use typed_builder;
pub use typed_builder::TypedBuilder;

pub use crate::cpi::*;
pub use crate::error::*;
pub use crate::pda::*;
pub use crate::traits::*;
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

/// Make sure all traits are available.
pub mod prelude {
	#[cfg(feature = "logs")]
	pub use solana_program_log::Logger;

	pub use crate::traits::*;
}
