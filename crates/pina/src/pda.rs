//! PDA (Program Derived Address) functions.
//!
//! These wrapper functions provide PDA derivation that compiles on both native
//! and Solana targets. On-chain (Solana BPF), the runtime syscalls are used.
//! On native targets, these return stub values (`None`/`Err`) since the
//! syscalls are not available.

use crate::Address;
use crate::ProgramError;

/// Find a valid program derived address and its corresponding bump seed.
///
/// Returns `None` if no valid PDA exists. On native targets (non-Solana), this
/// always returns `None` since PDA derivation requires Solana runtime syscalls.
#[inline]
pub fn try_find_program_address(seeds: &[&[u8]], program_id: &Address) -> Option<(Address, u8)> {
	#[cfg(any(target_os = "solana", target_arch = "bpf"))]
	{
		Address::try_find_program_address(seeds, program_id)
	}

	#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
	{
		core::hint::black_box((seeds, program_id));
		None
	}
}

/// Find a valid program derived address and its corresponding bump seed.
///
/// # Panics
///
/// Panics if no valid PDA exists. On native targets (non-Solana), this always
/// panics since PDA derivation requires Solana runtime syscalls.
#[inline]
pub fn find_program_address(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
	try_find_program_address(seeds, program_id)
		.unwrap_or_else(|| panic!("could not find program address from seeds"))
}

/// Create a valid program derived address without searching for a bump seed.
///
/// On native targets (non-Solana), this always returns
/// `Err(ProgramError::InvalidSeeds)`.
#[inline]
pub fn create_program_address(
	seeds: &[&[u8]],
	program_id: &Address,
) -> Result<Address, ProgramError> {
	#[cfg(any(target_os = "solana", target_arch = "bpf"))]
	{
		Address::create_program_address(seeds, program_id).map_err(|_| ProgramError::InvalidSeeds)
	}

	#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
	{
		core::hint::black_box((seeds, program_id));
		Err(ProgramError::InvalidSeeds)
	}
}
