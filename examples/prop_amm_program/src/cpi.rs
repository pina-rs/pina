//! Generated-style CPI helpers for the `prop_amm_program` example.
//!
//! This module is a concrete prototype for what future Pina/Codama-generated
//! on-chain CPI account structs could look like when paired with
//! `pina::CpiHandle`, `pina::ToCpiAccounts`, and `pina::CpiContext`.
//!
//! Unlike the off-chain Codama client, these structs are allocator-free and use
//! const-generic account counts so they remain compatible with Pina's on-chain
//! `no_std` constraints.

use pina::*;

use crate::ID;
use crate::InitializeInstruction;
use crate::RotateAuthorityInstruction;
use crate::UpdateInstruction;

pub mod accounts {
	use super::*;

	#[derive(Clone, Copy, Debug)]
	pub struct Initialize<'a> {
		pub payer: CpiHandle<'a>,
		pub oracle: CpiHandle<'a>,
		pub system_program: CpiHandle<'a>,
	}

	impl<'a> Initialize<'a> {
		pub fn new(
			payer: &'a AccountView,
			oracle: &'a AccountView,
			system_program: &'a AccountView,
		) -> Result<Self, ProgramError> {
			Ok(Self {
				payer: CpiHandle::writable(payer)?,
				oracle: CpiHandle::writable(oracle)?,
				system_program: CpiHandle::readonly(system_program),
			})
		}
	}

	impl<'a> ToCpiAccounts<'a, 3> for Initialize<'a> {
		fn to_cpi_handles(&self) -> [CpiHandle<'a>; 3] {
			[self.payer, self.oracle, self.system_program]
		}
	}

	#[derive(Clone, Copy, Debug)]
	pub struct Update<'a> {
		pub oracle: CpiHandle<'a>,
		pub authority: CpiHandle<'a>,
	}

	impl<'a> Update<'a> {
		pub fn new(
			oracle: &'a AccountView,
			authority: &'a AccountView,
		) -> Result<Self, ProgramError> {
			Ok(Self {
				oracle: CpiHandle::writable(oracle)?,
				authority: CpiHandle::readonly(authority),
			})
		}
	}

	impl<'a> ToCpiAccounts<'a, 2> for Update<'a> {
		fn to_cpi_handles(&self) -> [CpiHandle<'a>; 2] {
			[self.oracle, self.authority]
		}
	}

	#[derive(Clone, Copy, Debug)]
	pub struct RotateAuthority<'a> {
		pub oracle: CpiHandle<'a>,
		pub authority: CpiHandle<'a>,
	}

	impl<'a> RotateAuthority<'a> {
		pub fn new(
			oracle: &'a AccountView,
			authority: &'a AccountView,
		) -> Result<Self, ProgramError> {
			Ok(Self {
				oracle: CpiHandle::writable(oracle)?,
				authority: CpiHandle::readonly(authority),
			})
		}
	}

	impl<'a> ToCpiAccounts<'a, 2> for RotateAuthority<'a> {
		fn to_cpi_handles(&self) -> [CpiHandle<'a>; 2] {
			[self.oracle, self.authority]
		}
	}
}

#[derive(Clone, Copy, Debug)]
struct ProgramAddressCheck<'a> {
	address: &'a Address,
}

impl<'a> ProgramAddressCheck<'a> {
	#[inline(always)]
	const fn new(address: &'a Address) -> Self {
		Self { address }
	}

	#[inline(always)]
	fn assert_address(&self, expected: &Address) -> ProgramResult {
		if self.address != expected {
			return Err(ProgramError::IncorrectProgramId);
		}

		Ok(())
	}
}

#[inline(always)]
pub fn initialize<'a>(ctx: &CpiContext<'a, accounts::Initialize<'a>, 3>) -> ProgramResult {
	let program_account = ProgramAddressCheck::new(ctx.program);
	program_account.assert_address(&ID)?;

	let data = InitializeInstruction::builder().build();
	ctx.invoke(data.to_bytes(), &[])
}

#[inline(always)]
pub fn update<'a>(
	ctx: &CpiContext<'a, accounts::Update<'a>, 2>,
	new_price: PodU64,
) -> ProgramResult {
	let program_account = ProgramAddressCheck::new(ctx.program);
	program_account.assert_address(&ID)?;

	let data = UpdateInstruction::builder().new_price(new_price).build();
	ctx.invoke(data.to_bytes(), &[])
}

#[inline(always)]
pub fn rotate_authority<'a>(
	ctx: &CpiContext<'a, accounts::RotateAuthority<'a>, 2>,
	new_authority: Address,
) -> ProgramResult {
	let program_account = ProgramAddressCheck::new(ctx.program);
	program_account.assert_address(&ID)?;

	let data = RotateAuthorityInstruction::builder()
		.new_authority(new_authority)
		.build();
	ctx.invoke(data.to_bytes(), &[])
}

#[inline(always)]
pub fn initialize_context<'a>(
	accounts: accounts::Initialize<'a>,
) -> CpiContext<'a, accounts::Initialize<'a>, 3> {
	CpiContext::new(&ID, accounts)
}

#[inline(always)]
pub fn update_context<'a>(
	accounts: accounts::Update<'a>,
) -> CpiContext<'a, accounts::Update<'a>, 2> {
	CpiContext::new(&ID, accounts)
}

#[inline(always)]
pub fn rotate_authority_context<'a>(
	accounts: accounts::RotateAuthority<'a>,
) -> CpiContext<'a, accounts::RotateAuthority<'a>, 2> {
	CpiContext::new(&ID, accounts)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn update_cpi_instruction_roundtrip() {
		let data = UpdateInstruction::builder()
			.new_price(PodU64::from_primitive(99))
			.build();
		let decoded = UpdateInstruction::try_from_bytes(data.to_bytes())
			.unwrap_or_else(|e| panic!("decode update cpi bytes: {e:?}"));

		assert_eq!(u64::from(decoded.new_price), 99);
	}

	#[test]
	fn rotate_authority_cpi_instruction_roundtrip() {
		let next_authority = Address::new_from_array([7u8; ADDRESS_BYTES]);
		let data = RotateAuthorityInstruction::builder()
			.new_authority(next_authority)
			.build();
		let decoded = RotateAuthorityInstruction::try_from_bytes(data.to_bytes())
			.unwrap_or_else(|e| panic!("decode rotate cpi bytes: {e:?}"));

		assert_eq!(decoded.new_authority, next_authority);
	}

	#[test]
	fn program_address_check_rejects_wrong_program() {
		let wrong_program = Address::new_from_array([3u8; ADDRESS_BYTES]);
		let program_account = ProgramAddressCheck::new(&wrong_program);
		let error = program_account
			.assert_address(&ID)
			.expect_err("reject wrong cpi program id");

		assert_eq!(error, ProgramError::IncorrectProgramId);
	}
}
