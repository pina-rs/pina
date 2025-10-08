#![no_std]

mod errors;
mod traits;

pub use bytemuck;
pub use bytemuck::Pod;
pub use bytemuck::Zeroable;
pub use num_enum::IntoPrimitive;
pub use num_enum::TryFromPrimitive;
#[cfg(feature = "derive")]
pub use pina_macros::*;
pub use pinocchio;
pub use pinocchio::account_info::AccountInfo;
pub use pinocchio::entrypoint;
pub use pinocchio::instruction::AccountMeta;
pub use pinocchio::instruction::Instruction;
pub use pinocchio::program_entrypoint;
pub use pinocchio::program_error::ProgramError;
pub use pinocchio::pubkey::Pubkey;
pub use pinocchio::ProgramResult;
pub use pinocchio_pubkey::*;
pub use pinocchio_system;

pub use crate::errors::*;
pub use crate::traits::*;

// #[cfg(not(feature = "std"))]
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

/// Make sure all traits are available.
pub mod prelude {
	pub use crate::traits::*;
	pub use crate::PinaError;
}
