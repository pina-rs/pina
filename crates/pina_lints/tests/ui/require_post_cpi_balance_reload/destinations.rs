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
const CAP: u64 = 100;

/// An account wrapper that is not named `AccountView` and derefs to one.
struct Tok<'a>(&'a Account);

impl core::ops::Deref for Tok<'_> {
	type Target = Account;

	fn deref(&self) -> &Account {
		self.0
	}
}

struct WrappedContext<'a> {
	user_ata: Tok<'a>,
	fee_ata: Tok<'a>,
}

struct RawContext<'a> {
	user_ata: &'a Account,
	fee_ata: &'a Account,
}

struct Receipt {
	observed: u64,
	credited: u64,
}

fn pick<A, B>(_first: A, second: B) -> B {
	second
}

/// Accounts reached through `&mut self` accessors.
struct AccessorContext<'a> {
	user_ata: &'a Account,
}

impl<'a> AccessorContext<'a> {
	fn user_ata_mut(&mut self) -> &'a Account {
		self.user_ata
	}
}

/// A wrapper whose `invoke()` could target any program.
struct Relay<T>(T);

impl<T> Relay<T> {
	fn invoke(&self) -> Result<(), ()> {
		Ok(())
	}
}

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
	pinocchio_token::instructions::Transfer::new(source, treasury, owner, 10)
		.invoke_with_program(owner)?;
	//~^^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
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

fn process_wrapper_field_reload_of_other_account(
	ctx: &WrappedContext<'_>,
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = ctx.user_ata.amount();
	TransferChecked::new(source, mint, &ctx.user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `ctx.user_ata` makes an earlier read of its balance stale
	let fee = ctx.fee_ata.amount();
	Ok(before + 10 + fee)
}

fn process_raw_field_reload_of_other_account(
	ctx: &RawContext<'_>,
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = ctx.user_ata.amount();
	TransferChecked::new(source, mint, ctx.user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `ctx.user_ata` makes an earlier read of its balance stale
	let fee = ctx.fee_ata.amount();
	Ok(before + 10 + fee)
}

fn process_wrapper_field_snapshot_of_other_account(
	ctx: &WrappedContext<'_>,
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let fee_before = ctx.fee_ata.amount();
	TransferChecked::new(source, mint, &ctx.user_ata, owner, 10, 0).invoke_with_program(owner)?;
	Ok(fee_before)
}

fn process_indexed_reload_of_other_account(
	accounts: &[Account],
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = accounts.get(2).ok_or(())?.amount();
	TransferChecked::new(source, mint, accounts.get(2).ok_or(())?, owner, 10, 0)
		.invoke_with_program(owner)?;
	//~^^ ERROR: transfer into `accounts.get(2)` makes an earlier read of its balance stale
	let other = accounts.get(3).ok_or(())?.amount();
	Ok(before + other)
}

fn process_reload_in_plain_loop(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let mut total = 0u64;
	loop {
		let before = user_ata.amount();
		TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
		let after = user_ata.amount();
		total += after - before;
		if total > 5 {
			break;
		}
	}
	Ok(total)
}

fn process_reload_in_for_loop(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let mut total = 0u64;
	for _ in 0..3 {
		let before = user_ata.amount();
		TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
		let after = user_ata.amount();
		total += after - before;
	}
	Ok(total)
}

fn process_reload_only_checked_then_snapshot_credited(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let after = user_ata.amount();
	if after < before {
		return Err(());
	}
	Ok(before + 10)
}

fn process_snapshot_combined_with_reload_delta(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	before.checked_add(after - before).ok_or(())
}

fn process_snapshot_compared_through_cast(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	if before as u128 == 0 || before >= CAP {
		return Ok(CAP);
	}
	Ok(0)
}

fn process_legacy_builder_behind_relay(
	source: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	Relay(pinocchio_token::instructions::Transfer::new(
		source, treasury, owner, 10,
	))
	.invoke()?;
	//~^^^^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
	Ok(before + 10)
}

fn process_iterator_accounts_stay_distinct(
	accounts: &[Account],
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let mut remaining = accounts.iter();
	let source = remaining.next().ok_or(())?;
	let destination = remaining.next().ok_or(())?;
	let before = destination.amount();
	TransferChecked::new(source, mint, destination, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `destination` makes an earlier read of its balance stale
	let source_after = source.amount();
	source_after.checked_add(before + 10).ok_or(())
}

fn process_shadowed_pattern_bindings_stay_distinct(
	accounts: &[Account],
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let Some(user) = accounts.get(2) else {
		return Err(());
	};
	let before = user.amount();
	TransferChecked::new(source, mint, user, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user` makes an earlier read of its balance stale
	let Some(user) = accounts.get(3) else {
		return Err(());
	};
	let other = user.amount();
	Ok(other + before)
}

fn process_token_crate_loader_snapshot(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = pinocchio_token::state::TokenAccount::from_account_view(user_ata)?.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	Ok(before + 10)
}

fn process_snapshot_escapes_through_tuple(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let after = user_ata.amount();
	let credited = (after, before + 10).1;
	Ok(credited)
}

fn process_snapshot_escapes_through_struct(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<Receipt, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let after = user_ata.amount();
	Ok(Receipt {
		observed: after,
		credited: before + 10,
	})
}

fn process_snapshot_escapes_beside_direct_reload(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	Ok((user_ata.amount(), before + 10).1)
}

fn process_plain_delta(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	Ok(after - before)
}

fn process_two_step_delta(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	let delta = after.checked_sub(before).ok_or(())?;
	let total = before.checked_add(delta).ok_or(())?;
	Ok(total)
}

fn process_legacy_builder_swapped_for_token_2022(
	source: &Account,
	mint: &Account,
	treasury: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = treasury.amount();
	let token_2022 = Token2022TransferChecked::new(source, mint, treasury, owner, 10, 0);
	pick(
		pinocchio_token::instructions::TransferChecked::new(source, mint, treasury, owner, 10, 0),
		token_2022,
	)
	.invoke()?;
	//~^^^^^ ERROR: transfer into `treasury` makes an earlier read of its balance stale
	Ok(before + 10)
}

fn process_snapshot_through_mut_accessor(
	ctx: &mut AccessorContext<'_>,
	source: &Account,
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = ctx.user_ata_mut().amount();
	TransferChecked::new(source, mint, ctx.user_ata_mut(), owner, 10, 0)
		.invoke_with_program(owner)?;
	//~^^ ERROR: transfer into `ctx.user_ata_mut()` makes an earlier read of its balance stale
	Ok(before + 10)
}

fn process_reload_multiplied_by_zero(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let after = user_ata.amount();
	Ok(before + after * 0 + 10)
}

fn process_reload_minus_itself(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let after = user_ata.amount();
	Ok(before.wrapping_add(after - after).wrapping_add(10))
}

fn process_snapshot_added_to_bare_reload(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `user_ata` makes an earlier read of its balance stale
	let after = user_ata.amount();
	before.checked_add(after).ok_or(())
}

fn process_next_item_is_not_a_reload(
	accounts: &[Account],
	mint: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let mut remaining = accounts.iter();
	let destination = remaining.next().ok_or(())?;
	let before = destination.amount();
	TransferChecked::new(owner, mint, destination, owner, 10, 0).invoke_with_program(owner)?;
	//~^ ERROR: transfer into `destination` makes an earlier read of its balance stale
	let after = remaining.next().ok_or(())?.amount();
	after.checked_sub(before).ok_or(())
}

fn process_function_call_syntax_delta(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	u64::checked_sub(after, before).ok_or(())
}

fn process_absolute_difference_delta(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	Ok(after.abs_diff(before))
}

fn process_exact_arrival_check(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	if after != before + 10 {
		return Err(());
	}
	Ok(())
}

fn process_expected_balance_local_check(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<(), ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let expected = before.checked_add(10).ok_or(())?;
	let after = user_ata.amount();
	if after != expected {
		return Err(());
	}
	Ok(())
}

fn process_widened_delta(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u128, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	Ok(after as u128 - before as u128)
}

fn process_saturating_delta(
	source: &Account,
	mint: &Account,
	user_ata: &Account,
	owner: &Account,
) -> Result<u64, ()> {
	let before = user_ata.amount();
	TransferChecked::new(source, mint, user_ata, owner, 10, 0).invoke_with_program(owner)?;
	let after = user_ata.amount();
	let received = after.saturating_sub(before);
	Ok(received)
}

fn main() {}

// compile-fail
