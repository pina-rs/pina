#![allow(clippy::inline_always)]
#![no_std]

use pina::*;

declare_id!("4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT");

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Pubkey,
		accounts: &[AccountInfo],
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
	pub maker: Pubkey,
	pub mint_a: Pubkey,
	pub mint_b: Pubkey,
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
	pub maker: &'a AccountInfo,
	pub mint_a: &'a AccountInfo,
	pub mint_b: &'a AccountInfo,
	pub maker_ata_a: &'a AccountInfo,
	pub escrow: &'a AccountInfo,
	pub vault: &'a AccountInfo,
	pub system_program: &'a AccountInfo,
	pub token_program: &'a AccountInfo,
}

const SEED_PREFIX: &[u8] = b"escrow";
const SPL_PROGRAM_IDS: [Pubkey; 2] = [token::ID, token_2022::ID];

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
		let escrow_seeds = seeds_escrow!(self.maker.key().as_ref(), &args.seed.0);
		let escrow_seeds_with_bump =
			seeds_escrow!(self.maker.key().as_ref(), &args.seed.0, args.bump);

		// assertions
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.maker.assert_signer()?;
		self.mint_a.assert_owners(&SPL_PROGRAM_IDS)?;
		self.mint_b.assert_owners(&SPL_PROGRAM_IDS)?;
		self.maker_ata_a.assert_associated_token_address(
			self.maker.key(),
			self.mint_a.key(),
			self.token_program.key(),
		)?;
		self.escrow
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(escrow_seeds_with_bump, &ID)?;
		self.vault
			.assert_empty()?
			.assert_writable()?
			.assert_associated_token_address(
				self.escrow.key(),
				self.mint_a.key(),
				self.token_program.key(),
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
			.maker(*self.maker.key())
			.mint_a(*self.mint_a.key())
			.mint_b(*self.mint_b.key())
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
			token_program: self.token_program.key(),
		}
		.invoke()?;

		Ok(())
	}
}

#[derive(Accounts, Debug)]
pub struct TakeAccounts<'a> {
	pub taker: &'a AccountInfo,
	pub mint_a: &'a AccountInfo,
	pub mint_b: &'a AccountInfo,
	pub taker_ata_a: &'a AccountInfo,
	pub taker_ata_b: &'a AccountInfo,
	pub maker: &'a AccountInfo,
	pub maker_ata_b: &'a AccountInfo,
	pub escrow: &'a AccountInfo,
	pub vault: &'a AccountInfo,
	pub token_program: &'a AccountInfo,
	pub system_program: &'a AccountInfo,
}

impl<'a> ProcessAccountInfos<'a> for TakeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
		// This is purely to validate the discriminator as there are no needed
		// arguments.
		let _ = TakeInstruction::try_from_bytes(data)?;

		// assertions
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		self.taker.assert_signer()?.assert_writable()?;
		self.taker_ata_a
			.assert_owners(&SPL_PROGRAM_IDS)?
			.assert_data_len(token::state::TokenAccount::LEN)?
			.assert_associated_token_address(
				self.taker.key(),
				self.mint_a.key(),
				self.token_program.key(),
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
		let escrow_seeds_with_bump = seeds_escrow!(self.maker.key().as_ref(), &seed.0, *bump);

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
				self.escrow.key(),
				self.mint_a.key(),
				self.token_program.key(),
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
			from: self.taker_ata_a,
			mint: self.mint_b,
			to: self.maker_ata_b,
			authority: self.taker,
			amount: (*amount_b).into(),
			decimals: self.mint_b.as_token_2022_mint()?.decimals(),
			token_program: self.token_program.key(),
		}
		.invoke()?;

		let bump_as_seeds = [*bump];
		let escrow_seeds = seeds_escrow!(true, self.maker.key().as_ref(), &seed.0, &bump_as_seeds);
		let escrow_signer = Signer::from(&escrow_seeds);
		let signers = [escrow_signer];

		token_2022::instructions::TransferChecked {
			from: self.vault,
			mint: self.mint_a,
			to: self.taker_ata_a,
			authority: self.escrow,
			amount: self.vault.as_token_2022_account()?.amount(),
			decimals: self.mint_a.as_token_2022_mint()?.decimals(),
			token_program: self.token_program.key(),
		}
		.invoke_signed(&signers)?;
		token_2022::instructions::CloseAccount {
			account: self.vault,
			destination: self.maker,
			authority: self.escrow,
			token_program: self.token_program.key(),
		}
		.invoke_signed(&signers)?;

		{
			self.escrow.as_account_mut::<EscrowState>(&ID)?.zeroed();
		}

		self.escrow.close_with_recipient(self.maker)
	}
}
