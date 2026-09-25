//! Token escrow program built with pina.
//!
//! Flow:
//! 1. **Make** — the maker deposits token A into a PDA-owned vault and records
//!    the desired amount of token B in the escrow state.
//! 2. **Take** — the taker sends token B to the maker's ATA, then the vault
//!    releases token A to the taker's ATA. The escrow is closed and rent is
//!    returned to the maker.
//! 3. **Cancel** — the maker aborts the offer before a taker appears: the
//!    vault returns the full token A balance to the maker's ATA, then the
//!    vault and the escrow close and both rents return to the maker.

#![allow(missing_docs)]
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
	use super::*;

	nostd_entrypoint!(EscrowInstruction::process_instruction);
}

#[discriminator(entrypoint)]
pub enum EscrowInstruction {
	Make = 1,
	Take = 2,
	Cancel = 3,
}

#[discriminator]
pub enum EscrowAccount {
	EscrowState = 1,
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowError {
	/// The token accounts do not match the offer's recorded mint and maker.
	OfferKeyMismatch = 0,
	/// A supplied token account is not the one the offer references.
	TokenAccountMismatch = 1,
	/// One side of the offer is zero, so the exchange gives value away.
	EmptyOffer = 2,
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

#[instruction(discriminator = EscrowInstruction::Cancel)]
pub struct CancelInstruction {}

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

		// Reject an offer that gives nothing away before any CPI runs, because
		// either zero amount makes the exchange one-sided: `amount_b == 0` lets
		// the maker hand over token A for nothing, and `amount_a == 0` lets the
		// taker take token A for nothing. Only the positive-value shape is a
		// real offer. The check covers the requested amount, while the stored
		// `amount_a` is the measured vault delta; both must be nonzero for the
		// offer to be meaningful, so a requested zero is rejected here and an
		// unmeasurable transfer is rejected by the delta subtraction below.
		if args.amount_a.get() == 0 || args.amount_b.get() == 0 {
			return Err(EscrowError::EmptyOffer.into());
		}

		let maker_address = *self.maker.address();
		let escrow_seeds = EscrowState::seeds(&maker_address, args.seed.get());

		// Validate accounts
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		let token_program = *self.token_program.address();
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;
		self.maker.assert_signer()?;
		let mint_a = self
			.mint_a
			.as_token_mint_for_program(&token_program)?
			.assert_no_extensions()?;
		let decimals = mint_a.decimals();
		drop(mint_a);
		drop(
			self.mint_b
				.as_token_mint_for_program(&token_program)?
				.assert_no_extensions()?,
		);
		drop(self.maker_ata_a.as_associated_token_account(
			self.maker.address(),
			self.mint_a.address(),
			&token_program,
		)?);
		// The vault is the ATA of the escrow PDA (wallet = escrow), so anyone
		// can derive — and create — it before `Make` runs; rejecting any
		// pre-existing account outright lets a zero-cost griefer strand every
		// future `Make` for a chosen `(maker, seed)` slot (security sweep
		// finding D2). Tolerate a pre-created vault only when it is exactly
		// the account `Make` would have created itself: the derived ATA of
		// this escrow PDA and mint under the same token program — which also
		// pins the token-program owner and the stored wallet and mint —
		// holding zero tokens, with no delegate, and writable for the credit
		// below. Every rejection on this path keeps the original
		// `AccountAlreadyInitialized` error, so failure attribution is
		// unchanged for accounts the tolerance does not accept. An empty
		// account still follows the plain `assert_empty` + `assert_writable`
		// path; the address check for that path lives in the `Create` CPI
		// below: the associated token program derives the same
		// `[wallet, token_program, mint]` seeds and rejects a mismatch with
		// `InvalidSeeds` before it creates anything.
		let create_vault = match self.vault.assert_empty() {
			Ok(empty) => {
				empty.assert_writable()?;
				true
			}
			Err(rejected) => {
				let tolerated = self.vault.assert_writable().is_ok()
					&& self
						.vault
						.as_associated_token_account(
							self.escrow.address(),
							self.mint_a.address(),
							&token_program,
						)
						.is_ok_and(|vault| vault.amount() == 0 && vault.delegate().is_none());
				if !tolerated {
					return Err(rejected);
				}
				false
			}
		};

		// Create and initialize the escrow account atomically.
		//
		// The seeds bind `maker` and `Make` requires the maker to sign, so a
		// noncanonical bump could only duplicate the maker's own escrow. Every
		// later instruction validates the stored bump, the stored parties, and the
		// vault as the ATA of the passed escrow, so the two stay self-consistent
		// and a third party cannot be substituted into either.
		CreateProgramAccountWithUncheckedBump {
			account: self.escrow,
			payer: self.maker,
			owner: &ID,
			seeds: &escrow_seeds.as_slices(),
			bump: args.bump,
		}
		.invoke_with::<EscrowState>(|escrow| {
			escrow.maker = *self.maker.address();
			escrow.mint_a = *self.mint_a.address();
			escrow.mint_b = *self.mint_b.address();
			// Record the observed vault delta after the transfer. The temporary zero
			// prevents the requested amount from becoming protocol accounting before
			// the CPI has actually delivered tokens.
			escrow.amount_a.set(0);
			escrow.amount_b = args.amount_b;
			escrow.seed = args.seed;
			escrow.bump = args.bump;
			Ok(())
		})?;

		// Create the vault token account — skipped when a legitimate empty
		// vault already exists (see the tolerance above).
		if create_vault {
			associated_token_account::instructions::Create {
				account: self.vault,
				funding_account: self.maker,
				wallet: self.escrow,
				mint: self.mint_a,
				system_program: self.system_program,
				token_program: self.token_program,
			}
			.invoke()?;
		}
		let vault_before = self
			.vault
			.as_token_account_for_program(&token_program)?
			.amount();

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

		let vault_after = self
			.vault
			.as_token_account_for_program(&token_program)?
			.amount();
		let received = vault_after
			.checked_sub(vault_before)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		let mut escrow = self.escrow.as_account_mut::<EscrowState>(&ID)?;
		escrow.amount_a.set(received);

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
		drop(self.taker_ata_a.as_associated_token_account(
			self.taker.address(),
			self.mint_a.address(),
			&token_program,
		)?);
		// The taker's payment source must be the taker's canonical associated
		// token account for mint B, and this load is its only canonical pin:
		// the `CreateIdempotent` CPI below derives the *maker's* ATA, and the
		// `TransferChecked` CPI only proves the signer is authorized on
		// whichever account it receives. Without this check a taker could
		// fund the payment from any mint-B account they control rather than
		// the published account layout. The same shape is pinned by an
		// adversarial Surfpool test (`take_rejects_a_noncanonical_taker_...`).
		self.taker_ata_b.assert_writable()?;
		drop(self.taker_ata_b.as_associated_token_account(
			self.taker.address(),
			self.mint_b.address(),
			&token_program,
		)?);

		// Validate escrow state
		self.escrow.assert_not_empty()?;

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

		// Verify the escrow is the PDA for the maker and seed. The parse above
		// already captured `bump`, so this performs the identical
		// single-derivation check without re-parsing the account.
		EscrowState::assert_stored_bump(self.escrow, bump, &maker, u64::from(seed), &ID)?;

		// Validate maker and mint accounts
		//
		// The maker is credited twice — the vault close returns its rent and the
		// escrow close returns the escrow's rent — so it must be writable. The
		// `&mut AccountView` field already requires that at parse time; the
		// explicit assert keeps the requirement in the handler and reports the
		// framework's own error if the field type ever becomes shared.
		self.maker.assert_address(&maker)?.assert_writable()?;
		self.mint_a.assert_address(&mint_a)?;
		let mint_a = self
			.mint_a
			.as_token_mint_for_program(&token_program)?
			.assert_no_extensions()?;
		let decimals_a = mint_a.decimals();
		drop(mint_a);
		self.mint_b.assert_address(&mint_b)?;
		let mint_b = self
			.mint_b
			.as_token_mint_for_program(&token_program)?
			.assert_no_extensions()?;
		let decimals_b = mint_b.decimals();
		drop(mint_b);

		// Validate vault and maker ATA
		self.vault.assert_not_empty()?.assert_writable()?;
		let vault_amount = self
			.vault
			.as_associated_token_account(
				self.escrow.address(),
				self.mint_a.address(),
				&token_program,
			)?
			.amount();
		// The address check lives in the `CreateIdempotent` CPI below: the
		// associated token program derives the same seeds and rejects a mismatch
		// with `InvalidSeeds` before its idempotent branch.
		self.maker_ata_b.assert_writable()?;

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
		self.escrow.close_account_zeroed(&ID, self.maker)
	}
}

#[derive(Accounts, Debug)]
pub struct CancelAccounts<'a> {
	pub maker: &'a mut AccountView,
	pub mint_a: &'a AccountView,
	pub maker_ata_a: &'a AccountView,
	pub escrow: &'a mut AccountView,
	pub vault: &'a AccountView,
	pub token_program: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for CancelAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// Parse instruction data
		let _ = CancelInstruction::try_from_bytes(data)?;

		// Validate program accounts
		self.token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
		let token_program = *self.token_program.address();
		self.associated_token_program
			.assert_address(&associated_token_account::ID)?;
		self.system_program.assert_address(&system::ID)?;

		// Validate the maker. It signs the cancellation and is credited three
		// times — the refunded token A, the vault close rent, and the escrow
		// close rent — so it must be writable. The `&mut AccountView` field
		// already requires that at parse time; the explicit assert keeps the
		// requirement in the handler and reports the framework's own error if
		// the field type ever becomes shared.
		self.maker.assert_signer()?.assert_writable()?;

		// Validate escrow state
		self.escrow.assert_not_empty()?;

		let (maker, mint_a, seed, bump) = {
			let escrow = self.escrow.as_account::<EscrowState>(&ID)?;
			(escrow.maker, escrow.mint_a, escrow.seed, escrow.bump)
		};

		// Verify the escrow is the PDA for the maker and seed. The parse above
		// already captured `bump`, so this performs the identical
		// single-derivation check without re-parsing the account.
		EscrowState::assert_stored_bump(self.escrow, bump, &maker, u64::from(seed), &ID)?;

		// Only the recorded maker may abort its own offer, and the mint must be
		// the one the offer escrowed.
		self.maker.assert_address(&maker)?;
		self.mint_a.assert_address(&mint_a)?;
		let mint_a = self
			.mint_a
			.as_token_mint_for_program(&token_program)?
			.assert_no_extensions()?;
		let decimals_a = mint_a.decimals();
		drop(mint_a);

		// Validate vault and maker ATA
		self.vault.assert_not_empty()?.assert_writable()?;
		let vault_amount = self
			.vault
			.as_associated_token_account(
				self.escrow.address(),
				self.mint_a.address(),
				&token_program,
			)?
			.amount();
		// The address check lives in the `CreateIdempotent` CPI below: the
		// associated token program derives the same seeds and rejects a mismatch
		// with `InvalidSeeds` before its idempotent branch.
		self.maker_ata_a.assert_writable()?;

		// Create the maker's token A account if needed
		associated_token_account::instructions::CreateIdempotent {
			funding_account: self.maker,
			account: self.maker_ata_a,
			wallet: self.maker,
			mint: self.mint_a,
			system_program: self.system_program,
			token_program: self.token_program,
		}
		.invoke()?;

		// Prepare escrow signer for vault operations
		let escrow_seeds = EscrowState::seeds(&maker, u64::from(seed)).with_bump(bump);
		let escrow_signer = escrow_seeds.to_signer();
		let signers = [escrow_signer.as_signer()];

		// Refund the full vault balance to the maker
		token::instructions::TransferChecked::new(
			self.vault,
			self.mint_a,
			self.maker_ata_a,
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
		self.escrow.close_account_zeroed(&ID, self.maker)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn instruction_discriminators_are_stable() {
		assert_eq!(EscrowInstruction::Make as u8, 1);
		assert_eq!(EscrowInstruction::Take as u8, 2);
		assert_eq!(EscrowInstruction::Cancel as u8, 3);
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
		let data = [EscrowInstruction::Make as u8, 0];
		let result = parse_instruction::<EscrowInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
