//! Generated-style CPI builders for the `prop_amm_program` example.
//!
//! This module is a concrete prototype for future Pina/Codama-generated
//! on-chain CPI helpers. The exported surface mirrors Pinocchio's instruction
//! structs: build an instruction value, then call `.invoke()` or
//! `.invoke_signed()` with a validated program account.
//!
//! The builders stay allocator-free by using `pina::CpiHandle`,
//! `pina::ToCpiAccounts`, and const-generic account counts under the hood.

use pina::*;

use crate::ID;
use crate::InitializeInstruction;
use crate::RotateAuthorityInstruction;
use crate::UpdateInstruction;

/// Marker for the Prop AMM program ID used by generated CPI builders.
#[derive(Clone, Copy, Debug)]
pub struct PropAmmProgram;

impl CpiProgramId for PropAmmProgram {
	const ID: Address = ID;
}

/// A validated Prop AMM executable program account.
pub type ProgramAccount<'a> = Program<'a, PropAmmProgram>;

pub mod accounts {
	use super::*;

	/// Accounts required to create and initialize the oracle account.
	#[derive(Clone, Copy, Debug)]
	pub struct Initialize<'a> {
		/// Writable signer that funds the oracle account.
		pub payer: CpiHandle<'a>,

		/// Writable signer account that will store the oracle state.
		pub oracle: CpiHandle<'a>,

		/// System program used to create the oracle account.
		pub system_program: CpiHandle<'a>,
	}

	impl<'a> Initialize<'a> {
		/// Validates writable privileges and builds the CPI account set.
		pub fn new(
			payer: &'a AccountView,
			oracle: &'a AccountView,
			system_program: &'a AccountView,
		) -> Result<Self, ProgramError> {
			Ok(Self {
				payer: CpiHandle::writable_signer(payer)?,
				oracle: CpiHandle::writable_signer(oracle)?,
				system_program: CpiHandle::readonly(system_program),
			})
		}
	}

	impl<'a> ToCpiAccounts<'a, 3> for Initialize<'a> {
		fn to_cpi_handles(&self) -> [CpiHandle<'a>; 3] {
			[self.payer, self.oracle, self.system_program]
		}
	}

	/// Accounts required to update the oracle price.
	#[derive(Clone, Copy, Debug)]
	pub struct Update<'a> {
		/// Writable oracle account whose price will change.
		pub oracle: CpiHandle<'a>,

		/// Current oracle authority, required to sign.
		pub authority: CpiHandle<'a>,
	}

	impl<'a> Update<'a> {
		/// Validates writable privileges and builds the CPI account set.
		pub fn new(
			oracle: &'a AccountView,
			authority: &'a AccountView,
		) -> Result<Self, ProgramError> {
			Ok(Self {
				oracle: CpiHandle::writable(oracle)?,
				authority: CpiHandle::readonly_signer(authority),
			})
		}
	}

	impl<'a> ToCpiAccounts<'a, 2> for Update<'a> {
		fn to_cpi_handles(&self) -> [CpiHandle<'a>; 2] {
			[self.oracle, self.authority]
		}
	}

	/// Accounts required to rotate the oracle authority.
	#[derive(Clone, Copy, Debug)]
	pub struct RotateAuthority<'a> {
		/// Writable oracle account whose authority will change.
		pub oracle: CpiHandle<'a>,

		/// Current oracle authority, required to sign.
		pub authority: CpiHandle<'a>,
	}

	impl<'a> RotateAuthority<'a> {
		/// Validates writable privileges and builds the CPI account set.
		pub fn new(
			oracle: &'a AccountView,
			authority: &'a AccountView,
		) -> Result<Self, ProgramError> {
			Ok(Self {
				oracle: CpiHandle::writable(oracle)?,
				authority: CpiHandle::readonly_signer(authority),
			})
		}
	}

	impl<'a> ToCpiAccounts<'a, 2> for RotateAuthority<'a> {
		fn to_cpi_handles(&self) -> [CpiHandle<'a>; 2] {
			[self.oracle, self.authority]
		}
	}
}

pub mod instructions {
	use super::*;

	/// Creates and initializes an oracle account through a Prop AMM CPI.
	#[derive(Clone, Copy, Debug)]
	#[must_use = "the CPI has no effect until invoke or invoke_signed is called"]
	pub struct Initialize<'a> {
		/// Validated accounts required by the initialize instruction.
		pub accounts: accounts::Initialize<'a>,
	}

	impl<'a> Initialize<'a> {
		/// Builds an initialize CPI instruction.
		#[inline(always)]
		pub const fn new(accounts: accounts::Initialize<'a>) -> Self {
			Self { accounts }
		}

		/// Invokes the Prop AMM program using transaction-level signatures.
		#[inline(always)]
		pub fn invoke(&self, program: &ProgramAccount<'_>) -> ProgramResult {
			self.invoke_signed(program, &[])
		}

		/// Invokes the Prop AMM program with additional PDA signer seeds.
		#[inline(always)]
		pub fn invoke_signed(
			&self,
			program: &ProgramAccount<'_>,
			signers: &[Signer<'_, '_>],
		) -> ProgramResult {
			let data = [0u8; InitializeInstruction::SIZE];
			let ctx = CpiContext::new(*program, self.accounts);

			ctx.invoke(&data, signers)
		}
	}

	/// Updates an oracle price through a Prop AMM CPI.
	#[derive(Clone, Copy, Debug)]
	#[must_use = "the CPI has no effect until invoke or invoke_signed is called"]
	pub struct Update<'a> {
		/// Validated accounts required by the update instruction.
		pub accounts: accounts::Update<'a>,

		/// New integer price to store in the oracle.
		pub new_price: PodU64,
	}

	impl<'a> Update<'a> {
		/// Builds an update CPI instruction.
		#[inline(always)]
		pub const fn new(accounts: accounts::Update<'a>, new_price: PodU64) -> Self {
			Self {
				accounts,
				new_price,
			}
		}

		/// Invokes the Prop AMM program using transaction-level signatures.
		#[inline(always)]
		pub fn invoke(&self, program: &ProgramAccount<'_>) -> ProgramResult {
			self.invoke_signed(program, &[])
		}

		/// Invokes the Prop AMM program with additional PDA signer seeds.
		#[inline(always)]
		pub fn invoke_signed(
			&self,
			program: &ProgramAccount<'_>,
			signers: &[Signer<'_, '_>],
		) -> ProgramResult {
			let mut data = [0u8; UpdateInstruction::SIZE];
			UpdateInstruction::initialize(&mut data)?.new_price = self.new_price;
			let ctx = CpiContext::new(*program, self.accounts);

			ctx.invoke(&data, signers)
		}
	}

	/// Changes an oracle's authority through a Prop AMM CPI.
	#[derive(Clone, Copy, Debug)]
	#[must_use = "the CPI has no effect until invoke or invoke_signed is called"]
	pub struct RotateAuthority<'a> {
		/// Validated accounts required by the authority-rotation instruction.
		pub accounts: accounts::RotateAuthority<'a>,

		/// Address that becomes the new oracle authority.
		pub new_authority: Address,
	}

	impl<'a> RotateAuthority<'a> {
		/// Builds an authority-rotation CPI instruction.
		#[inline(always)]
		pub const fn new(accounts: accounts::RotateAuthority<'a>, new_authority: Address) -> Self {
			Self {
				accounts,
				new_authority,
			}
		}

		/// Invokes the Prop AMM program using transaction-level signatures.
		#[inline(always)]
		pub fn invoke(&self, program: &ProgramAccount<'_>) -> ProgramResult {
			self.invoke_signed(program, &[])
		}

		/// Invokes the Prop AMM program with additional PDA signer seeds.
		#[inline(always)]
		pub fn invoke_signed(
			&self,
			program: &ProgramAccount<'_>,
			signers: &[Signer<'_, '_>],
		) -> ProgramResult {
			let mut data = [0u8; RotateAuthorityInstruction::SIZE];
			RotateAuthorityInstruction::initialize(&mut data)?.new_authority = self.new_authority;
			let ctx = CpiContext::new(*program, self.accounts);

			ctx.invoke(&data, signers)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn update_cpi_instruction_roundtrip() {
		let mut data = [0u8; UpdateInstruction::SIZE];
		UpdateInstruction::initialize(&mut data)
			.unwrap_or_else(|e| panic!("initialize update cpi bytes: {e:?}"))
			.new_price
			.set(99);
		let decoded = UpdateInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("decode update cpi bytes: {e:?}"));

		assert_eq!(decoded.new_price.get(), 99);
	}

	#[test]
	fn rotate_authority_cpi_instruction_roundtrip() {
		let next_authority = Address::new_from_array([7u8; ADDRESS_BYTES]);
		let mut data = [0u8; RotateAuthorityInstruction::SIZE];
		RotateAuthorityInstruction::initialize(&mut data)
			.unwrap_or_else(|e| panic!("initialize rotate cpi bytes: {e:?}"))
			.new_authority = next_authority;
		let decoded = RotateAuthorityInstruction::try_from_bytes(&data)
			.unwrap_or_else(|e| panic!("decode rotate cpi bytes: {e:?}"));

		assert_eq!(decoded.new_authority, next_authority);
	}

	#[test]
	fn generated_program_marker_uses_program_id() {
		assert_eq!(PropAmmProgram::ID, ID);
	}
}
