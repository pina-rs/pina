//! Token escrow program built with pina.
//!
//! Flow:
//! 1. **Make** — the maker deposits token A into a PDA-owned vault and records
//!    the desired amount of token B in the escrow state.
//! 2. **Take** — the taker sends token B to the maker's ATA, then the vault
//!    releases token A to the taker's ATA. The escrow is closed and rent is
//!    returned to the maker.

#![allow(clippy::inline_always)]
#![no_std]

// On native builds the cdylib target needs std for unwinding and panic
// handling. On BPF, `nostd_entrypoint!()` provides the panic handler and
// allocator. Tests link against std automatically.
#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT");

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: EscrowInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			EscrowInstruction::Make => {
				MakeAccounts::try_from((program_id, accounts))?.process(data)
			}
			EscrowInstruction::Take => {
				TakeAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[discriminator]
pub enum EscrowInstruction {
	Make = 1,
	Take = 2,
}

#[discriminator]
pub enum EscrowAccount {
	EscrowState = 1,
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowError {
	OfferKeyMismatch = 0,
	TokenAccountMismatch = 1,
}

#[account(discriminator = EscrowAccount)]
#[pda(seeds = [SEED_PREFIX, maker: Address, seed: u64], bump = bump)]
pub struct EscrowState {
	pub maker: Address,
	pub mint_a: Address,
	pub mint_b: Address,
	/// The amount of token A that was sent by sender.
	pub amount_a: u64,
	/// The amount of token B to be received by the recipient.
	pub amount_b: u64,
	pub seed: u64,
	pub bump: u8,
}

#[instruction(discriminator = EscrowInstruction::Make)]
pub struct MakeInstruction {
	/// An ID of the transaction.
	pub seed: u64,
	/// The amount of token A to be sent.
	pub amount_a: u64,
	/// The amount of token B to be received.
	pub amount_b: u64,
	pub bump: u8,
}

#[instruction(discriminator = EscrowInstruction::Take)]
pub struct TakeInstruction {}

#[derive(Accounts, Debug)]
pub struct MakeAccounts<'a> {
	pub maker: &'a mut AccountView,
	pub mint_a: &'a AccountView,
	pub mint_b: &'a AccountView,
	pub maker_ata_a: &'a mut AccountView,
	pub escrow: &'a mut AccountView,
	pub vault: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}

/// Seed prefix for escrow PDAs.
const SEED_PREFIX: &[u8] = b"escrow";

const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

impl<'a> ProcessAccountInfos<'a> for MakeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction and prepare PDA seeds
		let args = MakeInstruction::try_from_bytes(data)?;
		let maker_address = *self.maker.address();
		let escrow_seeds = EscrowState::seeds(&maker_address, args.seed.get());
		let escrow_seeds_with_bump = escrow_seeds.with_bump(args.bump);

		// Validate accounts
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		let token_program = *self.token_program.address();
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.maker.assert_signer()?;
		let decimals = self
			.mint_a
			.as_token_mint_for_program(&token_program)?
			.decimals();
		drop(self.mint_b.as_token_mint_for_program(&token_program)?);
		drop(self.maker_ata_a.as_associated_token_account_checked(
			self.maker.address(),
			self.mint_a.address(),
			&token_program,
		)?);
		self.escrow
			.assert_empty()?
			.assert_seeds_with_bump(&escrow_seeds_with_bump.as_slices(), &ID)?;
		self.vault
			.assert_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.escrow.address(),
				self.mint_a.address(),
				self.token_program.address(),
			)?;

		// Create the escrow account
		CreateProgramAccountWithBump {
			account: self.escrow,
			payer: self.maker,
			owner: &ID,
			seeds: &escrow_seeds.as_slices(),
			bump: args.bump,
		}
		.invoke::<EscrowState>()?;

		// Initialize escrow state
		let mut escrow = self.escrow.as_account_mut::<EscrowState>(&ID)?;
		escrow.maker = *self.maker.address();
		escrow.mint_a = *self.mint_a.address();
		escrow.mint_b = *self.mint_b.address();
		escrow.amount_a = args.amount_a;
		escrow.amount_b = args.amount_b;
		escrow.seed = args.seed;
		escrow.bump = args.bump;
		drop(escrow);

		// Create the vault token account
		associated_token_account::instructions::Create {
			account: self.vault,
			funding_account: self.maker,
			wallet: self.escrow,
			mint: self.mint_a,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		// Transfer tokens to vault
		token::instructions::TransferChecked::new(
			self.maker_ata_a,
			self.mint_a,
			self.vault,
			self.maker,
			args.amount_a.into(),
			decimals,
		)
		.invoke_with_program(&token_program)?;

		Ok(())
	}
}

#[derive(Accounts, Debug)]
pub struct TakeAccounts<'a> {
	pub taker: &'a AccountView,
	pub mint_a: &'a AccountView,
	pub mint_b: &'a AccountView,
	pub taker_ata_a: &'a AccountView,
	pub taker_ata_b: &'a AccountView,
	pub maker: &'a mut AccountView,
	pub maker_ata_b: &'a AccountView,
	pub escrow: &'a mut AccountView,
	pub vault: &'a AccountView,
	pub token_program: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for TakeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction data
		let _ = TakeInstruction::try_from_bytes(data)?;

		// Validate program accounts
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		let token_program = *self.token_program.address();
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;

		// Validate taker accounts
		self.taker.assert_signer()?.assert_writable()?;
		self.taker_ata_a.assert_writable()?;
		drop(self.taker_ata_a.as_associated_token_account_checked(
			self.taker.address(),
			self.mint_a.address(),
			&token_program,
		)?);
		self.taker_ata_b.assert_writable()?;
		drop(self.taker_ata_b.as_associated_token_account_checked(
			self.taker.address(),
			self.mint_b.address(),
			&token_program,
		)?);

		// Validate escrow state
		self.escrow
			.assert_not_empty()?
			.assert_type::<EscrowState>(&ID)?;

		let (maker, mint_a, mint_b, amount_b, seed, bump) = {
			let escrow = self.escrow.as_account::<EscrowState>(&ID)?;
			(
				escrow.maker,
				escrow.mint_a,
				escrow.mint_b,
				escrow.amount_b,
				escrow.seed,
				escrow.bump,
			)
		};

		// Verify the escrow is the PDA for the maker and seed, using the
		// stored bump field (avoids re-deriving the canonical bump on-chain).
		EscrowState::assert_seeds(self.escrow, &maker, u64::from(seed), &ID)?;

		// Validate maker and mint accounts
		self.maker.assert_address(&maker)?;
		self.mint_a.assert_address(&mint_a)?;
		let decimals_a = self
			.mint_a
			.as_token_mint_for_program(&token_program)?
			.decimals();
		self.mint_b.assert_address(&mint_b)?;
		let decimals_b = self
			.mint_b
			.as_token_mint_for_program(&token_program)?
			.decimals();

		// Validate vault and maker ATA
		self.vault.assert_not_empty()?.assert_writable()?;
		let vault_amount = self
			.vault
			.as_associated_token_account_checked(
				self.escrow.address(),
				self.mint_a.address(),
				&token_program,
			)?
			.amount();
		self.maker_ata_b
			.assert_writable()?
			.assert_associated_token_address(
				self.maker.address(),
				self.mint_b.address(),
				self.token_program.address(),
			)?;

		// Create maker's token B account if needed
		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.taker,
			account: self.maker_ata_b,
			wallet: self.maker,
			mint: self.mint_b,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		// Transfer token B from taker to maker
		token::instructions::TransferChecked::new(
			self.taker_ata_b,
			self.mint_b,
			self.maker_ata_b,
			self.taker,
			u64::from(amount_b),
			decimals_b,
		)
		.invoke_with_program(&token_program)?;

		// Prepare escrow signer for vault operations
		let escrow_seeds = EscrowState::seeds(&maker, u64::from(seed)).with_bump(bump);
		let escrow_signer = escrow_seeds.to_signer();
		let signers = [escrow_signer.as_signer()];

		// Transfer token A from vault to taker
		token::instructions::TransferChecked::new(
			self.vault,
			self.mint_a,
			self.taker_ata_a,
			self.escrow,
			vault_amount,
			decimals_a,
		)
		.invoke_signed_with_program(&signers, &token_program)?;

		// Close vault account
		token::instructions::CloseAccount::new(self.vault, self.maker, self.escrow)
			.invoke_signed_with_program(&signers, &token_program)?;

		// Clear the raw backing bytes while closing; typed zero-copy views never
		// expose inactive storage for blanket mutation.
		self.escrow.close_account_zeroed(self.maker)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn instruction_discriminators_are_stable() {
		assert_eq!(EscrowInstruction::Make as u8, 1);
		assert_eq!(EscrowInstruction::Take as u8, 2);
	}

	#[test]
	fn spl_program_ids_are_expected() {
		assert_eq!(SPL_PROGRAM_IDS, [token::ID, token_2022::ID]);
	}

	#[test]
	fn seeds_build_expected_seed_arrays() {
		let maker = Address::new_from_array([3u8; 32]);
		let seed = 42u64;
		let bump = 7u8;

		let seeds = EscrowState::seeds(&maker, seed);
		let slices = seeds.as_slices();
		assert_eq!(slices.len(), 3);
		assert_eq!(slices[0], b"escrow");
		assert_eq!(slices[1], maker.as_ref());
		assert_eq!(slices[2], seed.to_le_bytes());

		let with_bump = seeds.with_bump(bump);
		let slices_with_bump = with_bump.as_slices();
		assert_eq!(slices_with_bump.len(), 4);
		assert_eq!(slices_with_bump[0], b"escrow");
		assert_eq!(slices_with_bump[1], maker.as_ref());
		assert_eq!(slices_with_bump[2], seed.to_le_bytes());
		assert_eq!(slices_with_bump[3], &[bump]);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [9u8; 32].into();
		let data = [EscrowInstruction::Make as u8];
		let result = parse_instruction::<EscrowInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
