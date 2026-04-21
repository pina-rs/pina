//! Prop AMM oracle example ported from Anchor v2's `prop-amm` benchmark.
//!
//! The upstream benchmark combines a tiny oracle account with a hand-written
//! assembly fast path for the hot `Update` instruction. This pina port keeps
//! the program semantics and validation model, but intentionally stays inside
//! the workspace's safe Rust constraints:
//!
//! - no handwritten assembly
//! - no unstable features
//! - no `unsafe`
//!
//! That means this example is best read as a **semantic port** and a
//! framework-comparison fixture rather than a byte-for-byte benchmark clone.
//!
//! ## Instructions
//!
//! - `Initialize` — create and initialize an oracle account
//! - `Update` — allow a fixed global updater to publish a new price
//! - `RotateAuthority` — let the oracle authority rotate its control key

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use core::mem::size_of;

use pina::*;

pub mod cpi;

declare_id!("55555555555555555555555555555555555555555555");

/// Matches the Anchor v2 benchmark's hard-coded updater key.
pub const UPDATE_AUTHORITY: Address = Address::new_from_array([
	234, 74, 108, 99, 226, 156, 82, 10, 190, 245, 80, 123, 19, 46, 197, 249, 149, 71, 118, 174,
	190, 190, 123, 146, 66, 30, 234, 105, 20, 70, 210, 44,
]);

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropAmmError {
	UnauthorizedUpdateAuthority = 0,
	UnauthorizedOracleAuthority = 1,
}

#[discriminator]
pub enum PropAmmInstruction {
	Initialize = 0,
	Update = 1,
	RotateAuthority = 2,
}

#[discriminator]
pub enum PropAmmAccountType {
	OracleState = 1,
}

#[account(discriminator = PropAmmAccountType)]
pub struct OracleState {
	pub authority: Address,
	pub price: PodU64,
}

#[instruction(discriminator = PropAmmInstruction, variant = Initialize)]
pub struct InitializeInstruction {}

#[instruction(discriminator = PropAmmInstruction, variant = Update)]
pub struct UpdateInstruction {
	pub new_price: PodU64,
}

#[instruction(discriminator = PropAmmInstruction, variant = RotateAuthority)]
pub struct RotateAuthorityInstruction {
	pub new_authority: Address,
}

#[derive(Accounts, Debug)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountView,
	pub oracle: &'a mut AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct UpdateAccounts<'a> {
	pub oracle: &'a mut AccountView,
	pub authority: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct RotateAuthorityAccounts<'a> {
	pub oracle: &'a mut AccountView,
	pub authority: &'a AccountView,
}

fn oracle_size() -> usize {
	size_of::<OracleState>()
}

fn assert_update_authority(authority: &AccountView) -> ProgramResult {
	if authority.address() == &UPDATE_AUTHORITY {
		return Ok(());
	}

	Err(PropAmmError::UnauthorizedUpdateAuthority.into())
}

fn assert_oracle_authority(authority: &AccountView, expected: &Address) -> ProgramResult {
	if authority.address() == expected {
		return Ok(());
	}

	Err(PropAmmError::UnauthorizedOracleAuthority.into())
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = InitializeInstruction::try_from_bytes(data)?;

		self.payer.assert_signer()?.assert_writable()?;
		self.oracle
			.assert_signer()?
			.assert_writable()?
			.assert_empty()?;
		self.system_program.assert_address(&system::ID)?;

		create_account(self.payer, self.oracle, oracle_size(), &ID)?;

		let mut oracle = self.oracle.as_account_mut::<OracleState>(&ID)?;
		*oracle = OracleState::builder()
			.authority(*self.payer.address())
			.price(PodU64::from_primitive(0))
			.build();

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = UpdateInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		assert_update_authority(self.authority)?;
		self.oracle
			.assert_writable()?
			.assert_type::<OracleState>(&ID)?;

		let mut oracle = self.oracle.as_account_mut::<OracleState>(&ID)?;
		oracle.price = args.new_price;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RotateAuthorityAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = RotateAuthorityInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.oracle
			.assert_writable()?
			.assert_type::<OracleState>(&ID)?;

		{
			let oracle = self.oracle.as_account::<OracleState>(&ID)?;
			assert_oracle_authority(self.authority, &oracle.authority)?;
		}

		let mut oracle = self.oracle.as_account_mut::<OracleState>(&ID)?;
		oracle.authority = args.new_authority;

		Ok(())
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: PropAmmInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			PropAmmInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			PropAmmInstruction::Update => UpdateAccounts::try_from(accounts)?.process(data),
			PropAmmInstruction::RotateAuthority => {
				RotateAuthorityAccounts::try_from(accounts)?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn update_instruction_roundtrip() {
		let instruction = UpdateInstruction::builder()
			.new_price(PodU64::from_primitive(1_234))
			.build();
		let bytes = instruction.to_bytes();
		let decoded =
			UpdateInstruction::try_from_bytes(bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(u64::from(decoded.new_price), 1_234);
	}

	#[test]
	fn rotate_authority_instruction_roundtrip() {
		let instruction = RotateAuthorityInstruction::builder()
			.new_authority([9u8; ADDRESS_BYTES].into())
			.build();
		let bytes = instruction.to_bytes();
		let decoded = RotateAuthorityInstruction::try_from_bytes(bytes)
			.unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(decoded.new_authority, Address::from([9u8; ADDRESS_BYTES]));
	}

	#[test]
	fn update_authority_is_stable() {
		let bytes: &[u8] = UPDATE_AUTHORITY.as_ref();
		assert_eq!(bytes.len(), ADDRESS_BYTES);
		assert_ne!(UPDATE_AUTHORITY, Address::default());
	}
}
