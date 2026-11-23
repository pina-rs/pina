// aux-build: solana_account_view.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code, unused_variables)]

extern crate solana_account_view;

use solana_account_view::AccountView;

struct ParsedAccounts<'a> {
	readonly: &'a AccountView,
	mutable: &'a mut AccountView,
	optional_readonly: Option<&'a AccountView>,
	readonly_slice: &'a [AccountView],
	mutable_slice: &'a mut [AccountView],
}

fn mutate_ref(account: &mut AccountView) {
	account.set_lamports(0);
}

// --- MUST DENY: escalation from shared `&AccountView` / `&[AccountView]` ---

fn escalate_via_deref_local(accounts: &ParsedAccounts<'_>) -> Result<(), ()> {
	let mut copy = *accounts.readonly;
	let _guard = copy.try_borrow_mut()?;
	Ok(())
}

fn escalate_via_inline_deref(accounts: &ParsedAccounts<'_>) {
	{ *accounts.readonly }.set_lamports(0);
}

fn escalate_via_clone(accounts: &ParsedAccounts<'_>) -> Result<(), ()> {
	accounts.readonly.clone().close()?;
	Ok(())
}

fn escalate_via_ufcs_clone(accounts: &ParsedAccounts<'_>) {
	let mut copy = Clone::clone(accounts.readonly);
	mutate_ref(&mut copy);
}

fn escalate_via_slice_index(accounts: &ParsedAccounts<'_>) {
	let mut copy = accounts.readonly_slice[0];
	copy.set_lamports(1);
}

fn escalate_via_option_copied(accounts: &ParsedAccounts<'_>) {
	if let Some(mut copy) = accounts.optional_readonly.copied() {
		copy.set_lamports(2);
	}
}

fn escalate_via_assert_chain(accounts: &ParsedAccounts<'_>) -> Result<(), ()> {
	let mut copy = (*accounts.readonly).assert_writable()?;
	copy.set_lamports(3);
	Ok(())
}

fn escalate_via_ref_mut_pattern(accounts: &ParsedAccounts<'_>) {
	let ref mut copy = { *accounts.readonly };
	copy.set_lamports(4);
}

fn escalate_via_closure_capture(accounts: &ParsedAccounts<'_>) {
	let mut copy = *accounts.readonly;
	let mut run = || {
		copy.set_lamports(5);
	};
	run();
}

fn escalate_via_block_and_rebind(accounts: &ParsedAccounts<'_>) {
	let mut copy = {
		let alias = *accounts.readonly;
		alias
	};
	copy.set_lamports(6);
}

fn escalate_via_raw_mut(accounts: &ParsedAccounts<'_>) {
	let mut copy = *accounts.readonly;
	let _ptr = &raw mut copy;
}

fn escalate_via_match_arm(accounts: &mut ParsedAccounts<'_>, cond: bool) {
	let mut acc = *accounts.mutable;
	match cond {
		true => {
			acc = *accounts.readonly;
		}
		false => {}
	}
	acc.set_lamports(7);
}

fn escalate_via_for_loop(accounts: &mut ParsedAccounts<'_>) -> Result<(), ()> {
	let mut acc = *accounts.mutable;
	for r in accounts.readonly_slice {
		acc = *r;
	}
	acc.close()?;
	Ok(())
}

fn escalate_via_loop_second_iteration(accounts: &mut ParsedAccounts<'_>, mut count: u8) {
	let mut acc = *accounts.mutable;
	loop {
		if count == 0 {
			break;
		}
		acc.set_lamports(8);
		acc = *accounts.readonly;
		count -= 1;
	}
}

fn escalate_via_four_binding_loop_chain(accounts: &mut ParsedAccounts<'_>, mut count: u8) {
	let mut a = *accounts.mutable;
	let mut b = *accounts.mutable;
	let mut c = *accounts.mutable;
	loop {
		if count == 0 {
			break;
		}
		a.set_lamports(9);
		a = b;
		b = c;
		c = *accounts.readonly;
		count -= 1;
	}
}

fn escalate_after_loop_break_taint(accounts: &mut ParsedAccounts<'_>) -> Result<(), ()> {
	let mut acc = *accounts.mutable;
	loop {
		// Safe: runs once before `acc` is tainted, then breaks unconditionally.
		acc.close()?;
		acc = *accounts.readonly;
		break;
	}
	// Unsafe: `acc` exited the loop tainted via `break`.
	acc.set_lamports(11);
	Ok(())
}

// --- MUST PASS: legitimate readonly reads, mutable slots, and untainted copies ---

fn ok_readonly_copy_for_reads(accounts: &ParsedAccounts<'_>) -> bool {
	let copy = *accounts.readonly;
	let _addr = copy.address();
	copy.is_writable()
}

fn ok_mutable_slot_mutation(accounts: &mut ParsedAccounts<'_>) -> Result<(), ()> {
	accounts.mutable.set_lamports(10);
	let mut copy = *accounts.mutable;
	copy.close()?;
	Ok(())
}

fn ok_mutable_slice_mutation(accounts: &mut ParsedAccounts<'_>) {
	let mut copy = accounts.mutable_slice[0];
	copy.set_lamports(20);
}

fn ok_owned_param_mutation(mut account: AccountView) {
	account.set_lamports(30);
}

fn ok_reassigned_from_mutable(accounts: &mut ParsedAccounts<'_>) {
	let mut copy = *accounts.readonly;
	let _ = copy.address();
	copy = *accounts.mutable;
	copy.set_lamports(40);
}

fn ok_untainted_loop_mutation(accounts: &mut ParsedAccounts<'_>, mut count: u8) -> Result<(), ()> {
	let mut acc = *accounts.mutable;
	loop {
		if count == 0 {
			break;
		}
		acc.close()?;
		count -= 1;
	}
	Ok(())
}

fn ok_loop_taint_before_unconditional_break(accounts: &mut ParsedAccounts<'_>) -> Result<(), ()> {
	let mut acc = *accounts.mutable;
	loop {
		acc.close()?;
		acc = *accounts.readonly;
		break;
	}
	let _ = acc.address();
	Ok(())
}

fn main() {}

// compile-fail
