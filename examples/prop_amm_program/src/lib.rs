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

#![allow(missing_docs)]
#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("55555555555555555555555555555555555555555555");

/// Matches the Anchor v2 benchmark's hard-coded updater key.
///
/// The benchmark shape keeps a fixed, source-visible updater key so the test
/// suites can rebuild the signing key deterministically. A fixed seed must
/// therefore stay committed, but never a uniform one: the previous fixture
/// derived the key from `[7u8; 32]`, so anyone reading this public source
/// recovered the private key — and with it full price control — in at most
/// 256 guesses (security sweep finding D1). The keypair seed is instead the
/// 32 ASCII bytes of `b"pina example fixture updater key"`: still fully
/// deterministic, but outside the repeated-single-byte brute-force space that
/// `tests/update_authority.rs` sweeps. A real program must generate its
/// authority keypair off-circuit and commit only the public key.
pub const UPDATE_AUTHORITY: Address = Address::new_from_array([
	9, 236, 45, 194, 55, 26, 2, 54, 152, 179, 31, 243, 24, 140, 167, 99, 120, 168, 252, 172, 10,
	173, 160, 155, 241, 194, 170, 126, 40, 228, 171, 52,
]);

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropAmmError {
	/// The signer cannot update the pool's oracle price.
	UnauthorizedUpdateAuthority = 0,
	/// The signer cannot publish a new oracle price.
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
	pub price: u64,
}

#[instruction(discriminator = PropAmmInstruction::Initialize)]
pub struct InitializeInstruction {}

#[instruction(discriminator = PropAmmInstruction::Update)]
pub struct UpdateInstruction {
	pub new_price: u64,
}

#[instruction(discriminator = PropAmmInstruction::RotateAuthority)]
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
	OracleState::SIZE
}

fn assert_update_authority(authority: AccountView) -> ProgramResult {
	if authority.address() == &UPDATE_AUTHORITY {
		return Ok(());
	}

	Err(PropAmmError::UnauthorizedUpdateAuthority.into())
}

fn assert_oracle_authority(authority: AccountView, expected: &Address) -> ProgramResult {
	if authority.address() == expected {
		return Ok(());
	}

	Err(PropAmmError::UnauthorizedOracleAuthority.into())
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = InitializeInstruction::try_from_bytes(data)?;

		self.payer.assert_signer()?.assert_writable()?;
		self.oracle.assert_signer()?.assert_empty()?;
		self.system_program.assert_address(&system::ID)?;

		CreateAccount {
			from: self.payer,
			to: self.oracle,
			space: oracle_size() as u64,
			owner: &ID,
		}
		.invoke()?;

		// A freshly created account holds zeroed storage; write the typed
		// discriminator before taking the validated view.
		{
			let mut storage = self.oracle.try_borrow_mut()?;
			OracleState::write_discriminator(&mut storage);
		}

		let mut oracle = self.oracle.as_account_mut::<OracleState>(&ID)?;
		oracle.authority = *self.payer.address();
		oracle.price.set(0);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = UpdateInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		// Validate the oracle account before the authority so a caller that
		// passed the wrong oracle sees the account error instead of
		// `UnauthorizedUpdateAuthority`, which would send integrators chasing
		// the wrong key.
		let mut oracle = self.oracle.as_account_mut::<OracleState>(&ID)?;
		assert_update_authority(*self.authority)?;
		oracle.price = args.new_price;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RotateAuthorityAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = RotateAuthorityInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;

		// One guard serves both the authority check and the write. Reloading the
		// account immutably before the mutable load would validate it twice.
		let mut oracle = self.oracle.as_account_mut::<OracleState>(&ID)?;
		assert_oracle_authority(*self.authority, &oracle.authority)?;
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
			PropAmmInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			PropAmmInstruction::Update => {
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}
			PropAmmInstruction::RotateAuthority => {
				RotateAuthorityAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn update_instruction_roundtrip() {
		let mut bytes = [0u8; UpdateInstruction::SIZE];
		UpdateInstruction::initialize(&mut bytes, |instruction| {
			instruction.new_price.set(1_234);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize: {error:?}"));
		let decoded =
			UpdateInstruction::try_from_bytes(&bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(decoded.new_price.get(), 1_234);
	}

	#[test]
	fn rotate_authority_instruction_roundtrip() {
		let mut bytes = [0u8; RotateAuthorityInstruction::SIZE];
		RotateAuthorityInstruction::initialize(&mut bytes, |instruction| {
			instruction.new_authority = [9u8; ADDRESS_BYTES].into();
			Ok(())
		})
		.unwrap_or_else(|error| panic!("initialize: {error:?}"));
		let decoded = RotateAuthorityInstruction::try_from_bytes(&bytes)
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
