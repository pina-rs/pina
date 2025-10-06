#![cfg_attr(not(feature = "std"), no_std)] // Conditionally apply no_std

pub use bytemuck::Pod;
pub use bytemuck::Zeroable;
pub use num_enum::IntoPrimitive;
pub use num_enum::TryFromPrimitive;
pub use pinocchio;
pub use pinocchio::ProgramResult;
pub use pinocchio::account_info::AccountInfo;
pub use pinocchio::entrypoint;
pub use pinocchio::instruction::AccountMeta;
pub use pinocchio::instruction::Instruction;
#[cfg(not(feature = "std"))]
pub use pinocchio::no_allocator;
#[cfg(not(feature = "std"))]
pub use pinocchio::nostd_panic_handler;
pub use pinocchio::program_entrypoint;
pub use pinocchio::program_error::ProgramError;
pub use pinocchio::pubkey::Pubkey;
pub use pinocchio_pubkey::*;
pub use pinocchio_system;

// Re-export macros
pub use solapino_macros::error;

// #[cfg(not(feature = "std"))]
#[macro_export]
macro_rules! nostd_entrypoint {
	($process_instruction:expr) => {
		$crate::entrypoint!($process_instruction, { $crate::pinocchio::MAX_TX_ACCOUNTS });
	};
	($process_instruction:expr, $maximum:expr) => {
		$crate::program_entrypoint!($process_instruction, $maximum);
		$crate::no_allocator!();
		$crate::nostd_panic_handler!();
	};
}
