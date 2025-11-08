#![no_std]
#![allow(clippy::inline_always)]

mod cpi;
mod error;
mod loaders;
mod pod;
mod traits;
mod utils;

pub use bytemuck;
pub use bytemuck::Pod;
pub use bytemuck::Zeroable;
#[cfg(feature = "derive")]
pub use pina_macros::*;
pub use pinocchio;
pub use pinocchio::account_info::AccountInfo;
pub use pinocchio::entrypoint;
pub use pinocchio::instruction::AccountMeta;
pub use pinocchio::instruction::Instruction;
pub use pinocchio::instruction::Seed;
pub use pinocchio::instruction::Signer;
pub use pinocchio::program_entrypoint;
pub use pinocchio::program_error::ProgramError;
pub use pinocchio::pubkey::Pubkey;
pub use pinocchio::pubkey::*;
pub use pinocchio::sysvars;
pub use pinocchio::ProgramResult;
#[cfg(feature = "token")]
pub use pinocchio_associated_token_account as associated_token_account;
#[cfg(feature = "logs")]
pub use pinocchio_log;
#[cfg(feature = "logs")]
pub use pinocchio_log::log_cu_usage;
pub use pinocchio_log::logger::Logger;
pub use pinocchio_pubkey::*;
pub use pinocchio_system as system;
#[cfg(feature = "token")]
pub use pinocchio_token as token;
#[cfg(feature = "token")]
pub use pinocchio_token_2022 as token_2022;
pub use pod::*;
pub use typed_builder;
pub use typed_builder::TypedBuilder;

pub use crate::cpi::*;
pub use crate::error::*;
pub use crate::traits::*;
pub use crate::utils::*;

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

#[cfg(feature = "logs")]
#[macro_export]
macro_rules! log {
	($($arg:tt)*) => {
		$crate::pinocchio_log::log!($($arg)*);
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
	pub use super::Logger;
	pub use crate::traits::*;
}
