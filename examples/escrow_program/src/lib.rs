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
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: EscrowInstruction = parse_instruction(program_id, &ID, data)?;

		match instruction {
			EscrowInstruction::Make => MakeAccounts::try_from(accounts)?.process(data),
			EscrowInstruction::Take => TakeAccounts::try_from(accounts)?.process(data),
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
pub struct EscrowState {
	pub maker: Address,
	pub mint_a: Address,
	pub mint_b: Address,
	/// The amount of token A that was sent by sender.
	pub amount_a: PodU64,
	/// The amount of token B to be received by the recipient.
	pub amount_b: PodU64,
	pub seed: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = EscrowInstruction, variant = Make)]
pub struct MakeInstruction {
	/// An ID of the transaction.
	pub seed: PodU64,
	/// The amount of token A to be sent.
	pub amount_a: PodU64,
	/// The amount of token B to be received.
	pub amount_b: PodU64,
	pub bump: u8,
}

#[instruction(discriminator = EscrowInstruction, variant = Take)]
pub struct TakeInstruction {}

#[derive(Accounts, Debug)]
pub struct MakeAccounts<'a> {
	pub maker: &'a AccountView,
	pub mint_a: &'a AccountView,
	pub mint_b: &'a AccountView,
	pub maker_ata_a: &'a AccountView,
	pub escrow: &'a AccountView,
	pub vault: &'a AccountView,
	pub system_program: &'a AccountView,
	pub token_program: &'a AccountView,
}

const SEED_PREFIX: &[u8] = b"escrow";
const SPL_PROGRAM_IDS: [Address; 2] = [token::ID, token_2022::ID];

#[macro_export]
macro_rules! seeds_escrow {
	($maker:expr, $seed:expr) => {
		&[SEED_PREFIX, $maker, $seed]
	};

	($maker:expr, $seed:expr, $bump:expr) => {
		&[SEED_PREFIX, $maker, $seed, &[$bump]]
	};
	(true, $maker:expr, $seed:expr, $bump:expr) => {
		[
			Seed::from(SEED_PREFIX),
			Seed::from($maker),
			Seed::from($seed),
			Seed::from($bump),
		]
	};
}

impl<'a> ProcessAccountInfos<'a> for MakeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		let args = MakeInstruction::try_from_bytes(data)?;
		let escrow_seeds = seeds_escrow!(self.maker.address().as_ref(), &args.seed.0);
		let escrow_seeds_with_bump =
			seeds_escrow!(self.maker.address().as_ref(), &args.seed.0, args.bump);

		// assertions
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.maker.assert_signer()?;
		self.mint_a.assert_owners(&SPL_PROGRAM_IDS)?;
		self.mint_b.assert_owners(&SPL_PROGRAM_IDS)?;
		self.maker_ata_a.assert_associated_token_address(
			self.maker.address(),
			self.mint_a.address(),
			self.token_program.address(),
		)?;
		self.escrow
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(escrow_seeds_with_bump, &ID)?;
		self.vault
			.assert_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.escrow.address(),
				self.mint_a.address(),
				self.token_program.address(),
			)?;

		// create the program account
		create_program_account_with_bump::<EscrowState>(
			self.escrow,
			self.maker,
			&ID,
			escrow_seeds,
			args.bump,
		)?;

		let escrow = self.escrow.as_account_mut::<EscrowState>(&ID)?;
		*escrow = EscrowState::builder()
			.maker(*self.maker.address())
			.mint_a(*self.mint_a.address())
			.mint_b(*self.mint_b.address())
			.amount_a(args.amount_a)
			.amount_b(args.amount_b)
			.seed(args.seed)
			.bump(args.bump)
			.build();

		associated_token_account::instructions::Create {
			account: self.vault,
			funding_account: self.maker,
			wallet: self.escrow,
			mint: self.mint_a,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		let decimals = self.mint_a.as_token_mint()?.decimals();
		token_2022::instructions::TransferChecked {
			from: self.maker_ata_a,
			to: self.vault,
			authority: self.maker,
			amount: args.amount_a.into(),
			mint: self.mint_a,
			decimals,
			token_program: self.token_program.address(),
		}
		.invoke()?;

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
	pub maker: &'a AccountView,
	pub maker_ata_b: &'a AccountView,
	pub escrow: &'a AccountView,
	pub vault: &'a AccountView,
	pub token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for TakeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		// Validate the discriminator; TakeInstruction has no payload fields.
		let _ = TakeInstruction::try_from_bytes(data)?;

		// -- assertions --
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.taker.assert_signer()?.assert_writable()?;
		// TODO: add validation for `self.taker_ata_b` — currently it is only
		// validated implicitly by the token program during the transfer CPI. An
		// explicit ATA address check here would catch mismatches earlier.
		self.taker_ata_a
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_data_len(token::state::TokenAccount::LEN)?
			.assert_associated_token_address(
				self.taker.address(),
				self.mint_a.address(),
				self.token_program.address(),
			)?;
		self.escrow
			.assert_not_empty()?
			.assert_writable()?
			.assert_type::<EscrowState>(&ID)?;

		let EscrowState {
			maker,
			mint_a,
			mint_b,
			amount_b,
			seed,
			bump,
			..
		} = self.escrow.as_account::<EscrowState>(&ID)?;
		let escrow_seeds_with_bump = seeds_escrow!(self.maker.address().as_ref(), &seed.0, *bump);

		self.escrow
			.assert_seeds_with_bump(escrow_seeds_with_bump, &ID)?;
		self.maker.assert_address(maker)?;
		self.mint_a
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_address(mint_a)?;
		self.mint_b
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_address(mint_b)?;

		self.vault
			.assert_not_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.escrow.address(),
				self.mint_a.address(),
				self.token_program.address(),
			)?;

		// create token account if none exists
		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.taker,
			account: self.maker_ata_b,
			wallet: self.maker,
			mint: self.mint_b,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		token_2022::instructions::TransferChecked {
			from: self.taker_ata_b,
			mint: self.mint_b,
			to: self.maker_ata_b,
			authority: self.taker,
			amount: (*amount_b).into(),
			decimals: self.mint_b.as_token_2022_mint()?.decimals(),
			token_program: self.token_program.address(),
		}
		.invoke()?;

		let bump_as_seeds = [*bump];
		let escrow_seeds =
			seeds_escrow!(true, self.maker.address().as_ref(), &seed.0, &bump_as_seeds);
		let escrow_signer = Signer::from(&escrow_seeds);
		let signers = [escrow_signer];

		token_2022::instructions::TransferChecked {
			from: self.vault,
			mint: self.mint_a,
			to: self.taker_ata_a,
			authority: self.escrow,
			amount: self.vault.as_token_2022_account()?.amount(),
			decimals: self.mint_a.as_token_2022_mint()?.decimals(),
			token_program: self.token_program.address(),
		}
		.invoke_signed(&signers)?;
		token_2022::instructions::CloseAccount {
			account: self.vault,
			destination: self.maker,
			authority: self.escrow,
			token_program: self.token_program.address(),
		}
		.invoke_signed(&signers)?;

		{
			self.escrow.as_account_mut::<EscrowState>(&ID)?.zeroed();
		}

		self.escrow.close_with_recipient(self.maker)
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
	fn seeds_macro_builds_expected_seed_arrays() {
		let maker = [3u8; 32];
		let seed = PodU64::from_primitive(42);
		let bump = 7u8;

		let seeds = seeds_escrow!(&maker, &seed.0);
		assert_eq!(seeds.len(), 3);
		assert_eq!(seeds[0], SEED_PREFIX);
		assert_eq!(seeds[1], &maker);
		assert_eq!(seeds[2], &seed.0);

		let seeds_with_bump = seeds_escrow!(&maker, &seed.0, bump);
		assert_eq!(seeds_with_bump.len(), 4);
		assert_eq!(seeds_with_bump[0], SEED_PREFIX);
		assert_eq!(seeds_with_bump[1], &maker);
		assert_eq!(seeds_with_bump[2], &seed.0);
		assert_eq!(seeds_with_bump[3], &[bump]);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [9u8; 32].into();
		let data = [EscrowInstruction::Make as u8];
		let result = parse_instruction::<EscrowInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
