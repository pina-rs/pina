// aux-build: pinocchio_token.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

extern crate pinocchio_token;

struct Account;
struct Transfer;
struct TransferChecked;
struct MintTo;
struct MintToChecked;
struct Token2022Program;

/// A re-export alias still resolves to the `TransferChecked` builder type.
type StakeDeposit = TransferChecked;
type Token2022Transfer = pinocchio_token::generic::Transfer<Token2022Program>;

mod system {
	/// Lamport transfers share the `Transfer` name but take no token amount.
	pub(crate) struct Transfer;

	impl Transfer {
		pub(crate) fn new(_: &super::Account, _: &super::Account, _: u64) -> Self {
			Self
		}

		pub(crate) fn invoke(&self) -> Result<(), ()> {
			Ok(())
		}
	}
}

impl Account {
	fn amount(&self) -> u64 {
		0
	}

	fn as_token_account(&self) -> Result<&Account, ()> {
		Ok(self)
	}
}

impl Transfer {
	fn new(_: &Account, _: &Account, _: &Account, _: u64) -> Self {
		Self
	}

	fn invoke_with_program(&self, _program: &Account) -> Result<(), ()> {
		Ok(())
	}
}

impl TransferChecked {
	fn new(_: &Account, _: &Account, _: &Account, _: &Account, _: u64, _: u8) -> Self {
		Self
	}

	fn with_multisig_signers(
		_: &Account,
		_: &Account,
		_: &Account,
		_: &Account,
		_: u64,
		_: u8,
		_: &[Account],
	) -> Self {
		Self
	}

	fn invoke_with_program(&self, _program: &Account) -> Result<(), ()> {
		Ok(())
	}

	fn invoke_with_unverified_program(&self, _program: &Account) -> Result<(), ()> {
		Ok(())
	}
}

impl MintTo {
	fn new(_: &Account, _: &Account, _: &Account, _: u64) -> Self {
		Self
	}

	fn invoke_with_program(&self, _program: &Account) -> Result<(), ()> {
		Ok(())
	}
}

impl MintToChecked {
	fn new(_: &Account, _: &Account, _: &Account, _: u64, _: u8) -> Self {
		Self
	}

	fn invoke_with_program(&self, _program: &Account) -> Result<(), ()> {
		Ok(())
	}
}

fn process_user_stake_ata_without_reload(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_stake_ata.as_token_account()?.amount();
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_stake_ata` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_user_stake_ata_with_reload(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_stake_ata.as_token_account()?.amount();
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_stake_ata.as_token_account()?.amount();
	after.checked_sub(before).ok_or(())
}

fn process_treasury_without_reload(
	source: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let treasury_balance = treasury.amount();
	Transfer::new(source, treasury, owner, 10).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
	if treasury_balance.checked_add(10).is_none() {
		return Err(());
	}
	Ok(())
}

fn process_fee_receiver_mint_without_reload(
	mint: &Account,
	fee_receiver: &Account,
	authority: &Account,
) -> Result<u64, ()> {
	let fees = fee_receiver.amount();
	MintTo::new(mint, fee_receiver, authority, 10).invoke_with_program(authority)?;
	//~^ ERROR: mint into `fee_receiver` makes an earlier read of its balance stale
	Ok(fees)
}

fn process_fee_receiver_mint_with_reload(
	mint: &Account,
	fee_receiver: &Account,
	authority: &Account,
) -> Result<u64, ()> {
	let before = fee_receiver.amount();
	MintToChecked::new(mint, fee_receiver, authority, 10, 0).invoke_with_program(authority)?;
	let after = fee_receiver.amount();
	after.checked_sub(before).ok_or(())
}

fn process_payout_without_snapshot(
	source: &Account,
	mint: &Account,
	user_reward_ata: &Account,
	owner: &Account,
) -> Result<(), ()> {
	TransferChecked::new(source, mint, user_reward_ata, owner, 10, 0).invoke_with_program(owner)
}

fn process_snapshot_used_only_before_cpi(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let before = user_stake_ata.amount();
	if before > 100 {
		return Err(());
	}
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)
}

fn process_non_integer_snapshot(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<bool, ()> {
	let was_empty = user_stake_ata.amount() == 0;
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	Ok(was_empty)
}

fn process_aliased_destination(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let stake_account = user_stake_ata.as_token_account()?;
	let before = stake_account.amount();
	StakeDeposit::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_stake_ata` makes an earlier read of its balance stale
	Ok(before)
}

fn process_multisig_unverified_program(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_stake_ata.amount();
	TransferChecked::with_multisig_signers(source, mint, user_stake_ata, owner, 10, 0, &[])
		.invoke_with_unverified_program(owner)?;
	//~^^ ERROR: transfer into `user_stake_ata` makes an earlier read of its balance stale
	Ok(before)
}

fn process_snapshot_captured_by_closure(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_stake_ata.amount();
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_stake_ata` makes an earlier read of its balance stale
	let expected = || before + 10;
	Ok(expected())
}

fn process_reload_after_unrelated_cpi(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_stake_ata.amount();
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_stake_ata` makes an earlier read of its balance stale
	MintTo::new(mint, source, owner, 10).invoke_with_program(owner)?;
	let after = user_stake_ata.amount();
	after.checked_sub(before).ok_or(())
}

fn process_legacy_program_snapshot(
	source: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	pinocchio_token::instructions::LegacyTransfer::new(source, treasury, owner, 10).invoke()?;
	before.checked_add(10).ok_or(())
}

fn process_legacy_builder_with_dynamic_program(
	source: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	pinocchio_token::instructions::LegacyTransfer::new(source, treasury, owner, 10)
		.invoke_with_program(owner)?;
	//~^^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_token_2022_alias_snapshot(
	source: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	Token2022Transfer::new(source, treasury, owner, 10).invoke()?;
	//~^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_lamport_transfer_into_vault(payer: &Account, vault: &Account) -> Result<(), ()> {
	system::Transfer::new(payer, vault, 10).invoke()
}

fn main() {}

// compile-fail
