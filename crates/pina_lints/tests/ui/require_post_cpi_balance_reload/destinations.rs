// aux-build: pinocchio_token.rs
// normalize-stderr-test: "\n$" -> ""

//! The snapshot tier: a pre-CPI balance snapshot of any destination, whatever
//! it is called, must not be trusted after a value-moving token CPI.

#![allow(dead_code, unused_assignments)]

extern crate pinocchio_token;

struct Account;
struct TokenState;
struct Token2022State {
	base: TokenState,
}
struct Transfer;
struct TransferChecked;
struct MintTo;
struct MintToChecked;
struct Token2022Program;
struct CloseAccount;

/// A re-export alias still resolves to the `TransferChecked` builder type.
type StakeDeposit = TransferChecked;
type Token2022TransferChecked =
	pinocchio_token::instructions::transfer_checked::TransferChecked<Token2022Program>;

const EMPTY: u64 = 0;

impl Account {
	fn amount(&self) -> u64 {
		0
	}

	fn as_token_account(&self) -> Result<&Account, ()> {
		Ok(self)
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

impl CloseAccount {
	fn new(_: &Account, _: &Account, _: &Account) -> Self {
		Self
	}

	fn invoke(&self) -> Result<(), ()> {
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

fn process_constant_comparison_of_snapshot(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let prior = user_stake_ata.amount();
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	Ok(if prior == 0 || EMPTY == prior { 1 } else { 0 })
}

fn process_aliased_builder_type(
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

fn process_unused_reload(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let _after = user_ata.amount();
	user_ata.amount();
	before.checked_add(10).ok_or(())
}

fn process_reload_in_one_branch(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
	flag: bool,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let mut total = 0;
	if flag {
		let after = user_ata.amount();
		total = after;
	}
	before.checked_add(total).ok_or(())
}

fn process_reload_in_condition(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	if user_ata.amount() < before {
		return Err(());
	}
	Ok(0)
}

fn process_copied_snapshot(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	let snapshot = before;
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	snapshot.checked_add(10).ok_or(())
}

fn process_function_call_syntax_read(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = Account::amount(user_ata);
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_assigned_snapshot(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let mut before = 0;
	before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_snapshot_overwritten_after_cpi(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let mut balance = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	balance = user_ata.amount();
	Ok(balance)
}

fn process_tuple_snapshot(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let (before, decimals) = (user_ata.amount(), 6u8);
	TransferChecked::new(source, mint, user_ata, owner, 10, decimals).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	before.checked_add(u64::from(decimals)).ok_or(())
}

fn process_token_2022_base_through_alias(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let token = user_ata.as_token_2022_account()?;
	let before = token.base.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_token_2022_base_direct(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.as_token_2022_account()?.base.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_token_2022_base_with_reload(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.as_token_2022_account()?.base.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.as_token_2022_account()?.base.amount();
	after.checked_sub(before).ok_or(())
}

fn process_bound_builder_then_snapshot(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let transfer = TransferChecked::new(source, mint, user_ata, owner, 10, 0);
	let before = user_ata.amount();
	transfer.invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_cpi_in_other_branch(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
	deposit: bool,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	if deposit {
		TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
		let after = user_ata.amount();
		after.checked_sub(before).ok_or(())
	} else {
		Ok(before)
	}
}

fn process_cpi_in_diverging_block(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
	deposit: bool,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	if !deposit {
		TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
		return Ok(0);
	}
	Ok(before)
}

fn process_two_deposits_one_reload(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	TransferChecked::new(source, mint, user_ata, owner, 5, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	after.checked_sub(before).ok_or(())
}

fn process_reload_after_unrelated_cpi(
	source: &Account,
	mint: &Account,
	user_stake_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_stake_ata.amount();
	TransferChecked::new(source, mint, user_stake_ata, owner, 10, 0).invoke_with_program(owner)?;
	CloseAccount::new(source, owner, owner).invoke()?;
	let after = user_stake_ata.amount();
	after.checked_sub(before).ok_or(())
}

fn process_legacy_program_snapshot(
	source: &Account,
	mint: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	pinocchio_token::instructions::TransferChecked::new(source, mint, treasury, owner, 10, 0)
		.invoke()?;
	before.checked_add(10).ok_or(())
}

fn process_legacy_builder_with_dynamic_program(
	source: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	pinocchio_token::instructions::Transfer::new(source, treasury, owner, 10)
		.invoke_with_program(owner)?;
	//~^^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn process_token_2022_alias_static_invoke(
	source: &Account,
	mint: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	Token2022TransferChecked::new(source, mint, treasury, owner, 10, 0).invoke()?;
	//~^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
	before.checked_add(10).ok_or(())
}

fn main() {}

// compile-fail
