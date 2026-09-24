// aux-build: pinocchio_token.rs
// aux-build: pinocchio_system.rs
// normalize-stderr-test: "\n$" -> ""

//! Builder recognition: builders are classified by the type their constructor
//! returns, whatever it is called or however many arguments it takes.

#![allow(dead_code)]

extern crate pinocchio_system;
extern crate pinocchio_token;

struct Account;
struct TokenState;
struct Token2022State {
	base: TokenState,
}
struct SplTransfer;
struct LamportTransfer;
struct AuthorityTransfer;
struct SystemLikeTransfer;

impl SystemLikeTransfer {
	/// Payer, recipient, system program, lamports: three accounts and an
	/// amount, but no mint, so it is not a token transfer.
	fn new(_: &Account, _: &Account, _: &Account, _: u64) -> Self {
		Self
	}

	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

/// An account wrapper that is not named `AccountView` and derefs to one.
struct Tok<'a>(&'a Account);

impl core::ops::Deref for Tok<'_> {
	type Target = Account;

	fn deref(&self) -> &Account {
		self.0
	}
}

struct WrappedContext<'a> {
	fee_ata: Tok<'a>,
	vault: Tok<'a>,
}

impl AuthorityTransfer {
	fn new(_: &Account, _: &Account, _: &Account) -> Self {
		Self
	}

	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

mod fallible {
	pub(crate) struct TransferChecked;

	impl TransferChecked {
		pub(crate) fn new(
			_: &super::Account,
			_: &super::Account,
			_: &super::Account,
			_: &super::Account,
			_: u64,
			_: u8,
		) -> Result<Self, ()> {
			Ok(Self)
		}

		pub(crate) fn invoke_with_program(&self, _program: &super::Account) -> Result<(), ()> {
			Ok(())
		}
	}
}

mod without_decimals {
	pub(crate) struct TransferChecked;

	impl TransferChecked {
		pub(crate) fn new(
			_: &super::Account,
			_: &super::Account,
			_: &super::Account,
			_: &super::Account,
			_: u64,
		) -> Self {
			Self
		}

		pub(crate) fn invoke_with_program(&self, _program: &super::Account) -> Result<(), ()> {
			Ok(())
		}
	}
}

impl Account {
	fn amount(&self) -> u64 {
		0
	}

	fn as_token_2022_account(&self) -> Result<Token2022State, ()> {
		Ok(Token2022State { base: TokenState })
	}
}

impl TokenState {
	fn amount(&self) -> u64 {
		0
	}
}

impl SplTransfer {
	fn new(_: &Account, _: &Account, _: &Account, _: &Account, _: u64, _: u8) -> Self {
		Self
	}

	fn invoke_with_program(&self, _program: &Account) -> Result<(), ()> {
		Ok(())
	}
}

impl LamportTransfer {
	fn new(_: &Account, _: &Account, _: u64) -> Self {
		Self
	}

	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

fn process_suffix_named_builder(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	SplTransfer::new(source, mint, vault, owner, 10, 0).invoke_with_program(owner)
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
}

fn process_fallible_constructor(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	fallible::TransferChecked::new(source, mint, vault, owner, 10, 0)?.invoke_with_program(owner)
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
}

fn process_constructor_without_decimals(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	without_decimals::TransferChecked::new(source, mint, vault, owner, 10)
		.invoke_with_program(owner)
	//~^^ ERROR: transfer into `vault` is not accounted from its observed balance delta
}

fn process_legacy_transfer_with_dynamic_program(
	source: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	pinocchio_token::instructions::Transfer::new(source, vault, owner, 10)
		.invoke_with_program(owner)
	//~^^ ERROR: transfer into `vault` is not accounted from its observed balance delta
}

fn process_legacy_static_invoke_into_vault(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	// Static `invoke()` of a builder bound to `pinocchio_token::TokenProgram`
	// targets the legacy SPL Token program, which cannot deduct a fee.
	pinocchio_token::instructions::TransferChecked::new(source, mint, vault, owner, 10, 0).invoke()
}

fn process_legacy_static_invoke_signed_into_vault(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	pinocchio_token::instructions::TransferChecked::new(source, mint, vault, owner, 10, 0)
		.invoke_signed(&[])
}

fn process_custody_token_2022_base_reads(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = vault.as_token_2022_account()?.base.amount();
	SplTransfer::new(source, mint, vault, owner, 10, 0).invoke_with_program(owner)?;
	let after = vault.as_token_2022_account()?.base.amount();
	after.checked_sub(before).ok_or(())
}

fn process_system_program_transfer_into_vault(
	payer: &Account,
	base: &Account,
	vault: &Account,
) -> Result<(), ()> {
	pinocchio_system::instructions::Transfer::new(payer, base, vault, 10).invoke()
}

fn process_lamport_transfer_into_vault(payer: &Account, vault: &Account) -> Result<(), ()> {
	LamportTransfer::new(payer, vault, 10).invoke()
}

fn process_authority_transfer_is_not_a_token_builder(
	config: &Account,
	new_pool_authority: &Account,
	signer: &Account,
) -> Result<(), ()> {
	AuthorityTransfer::new(config, new_pool_authority, signer).invoke()
}

fn process_custody_reads_of_other_wrapped_account(
	ctx: &WrappedContext<'_>,
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = ctx.fee_ata.amount();
	SplTransfer::new(source, mint, &ctx.vault, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `ctx.vault` is not accounted from its observed balance delta
	let after = ctx.fee_ata.amount();
	Ok(before + after)
}

fn process_custody_reads_only_in_closures(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let _before = || vault.amount();
	SplTransfer::new(source, mint, vault, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
	let _after = || vault.amount();
	Ok(())
}

fn process_local_three_account_transfer_into_vault(
	payer: &Account,
	vault: &Account,
	system_program: &Account,
) -> Result<(), ()> {
	SystemLikeTransfer::new(payer, vault, system_program, 10).invoke()
}

fn process_iterator_custody_reads_of_other_account(
	accounts: &[Account],
	mint: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let mut remaining = accounts.iter();
	let fee = remaining.next().ok_or(())?;
	let vault = remaining.next().ok_or(())?;
	let _before = fee.amount();
	SplTransfer::new(owner, mint, vault, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `vault` is not accounted from its observed balance delta
	let _after = fee.amount();
	Ok(())
}

fn process_custody_deposit_through_token_crate_loader(
	source: &Account,
	mint: &Account,
	vault: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = pinocchio_token::state::TokenAccount::from_account_view(vault)?.amount();
	SplTransfer::new(source, mint, vault, owner, 10, 0).invoke_with_program(owner)?;
	let after = pinocchio_token::state::TokenAccount::from_account_view(vault)?.amount();
	after.checked_sub(before).ok_or(())
}

fn process_custody_with_dynamic_index_bracketed(
	vaults: &[Account],
	index: usize,
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	// The typed identity cannot name `vaults.get(index)`; the reads written
	// against the same receiver text still bracket the transfer.
	let before = vaults.get(index).ok_or(())?.amount();
	SplTransfer::new(source, mint, vaults.get(index).ok_or(())?, owner, 10, 0)
		.invoke_with_program(owner)?;
	let after = vaults.get(index).ok_or(())?.amount();
	after.checked_sub(before).ok_or(())
}

fn process_custody_with_dynamic_index_unbracketed(
	vaults: &[Account],
	index: usize,
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<(), ()> {
	SplTransfer::new(source, mint, vaults.get(index).ok_or(())?, owner, 10, 0)
		.invoke_with_program(owner)
	//~^^ ERROR: transfer into `vaults.get(index).ok_or(())?` is not accounted from its observed balance delta
}

fn main() {}

// compile-fail
