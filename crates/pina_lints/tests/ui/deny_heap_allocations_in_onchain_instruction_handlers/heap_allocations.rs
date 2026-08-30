// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

#[derive(Clone, Copy)]
struct AccountView;

fn process_transfer(accounts: &[AccountView]) -> Result<(), ()> {
	let owned = accounts.to_vec();
	//~^ ERROR: heap allocation patterns should be avoided
	let copies = accounts.iter().collect::<Vec<_>>();
	let _ = copies;
	//~^ ERROR: heap allocation patterns should be avoided
	let label = format!("{} accounts", owned.len());
	//~^ ERROR: heap allocation patterns should be avoided
	let account = owned[0].clone();
	//~^ ERROR: heap allocation patterns should be avoided
	let _ = (label, account);
	Ok(())
}

fn process_register(accounts: &[AccountView]) -> Result<(), ()> {
	let mut entries: Vec<AccountView> = Vec::new();
	//~^ ERROR: heap allocation patterns should be avoided
	entries.push(accounts[0].clone());
	//~^ ERROR: heap allocation patterns should be avoided
	let name = String::from("entries");
	//~^ ERROR: heap allocation patterns should be avoided
	let rendered = name.to_string();
	//~^ ERROR: heap allocation patterns should be avoided
	let _ = rendered;
	Ok(())
}

fn process_allowlisted(accounts: &[AccountView]) -> Result<(), ()> {
	#[allow(deny_heap_allocations_in_onchain_instruction_handlers)]
	let owned = accounts.to_vec();
	let _ = owned;
	Ok(())
}

fn helper_for_offchain_use(account: &AccountView) -> AccountView {
	// Handler-shaped names only; this helper allocates freely.
	account.clone()
}

fn main() {}
