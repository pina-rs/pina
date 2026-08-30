// aux-build: pina_macros.rs
// normalize-stderr-test: "\n$" -> ""

extern crate pina_macros;

use pina_macros::Accounts;

struct AccountView;

#[derive(Accounts)]
struct DefaultDistinct<'a> {
	#[pina(remaining)]
	remaining: &'a mut [AccountView],
}

#[derive(Accounts)]
struct ExplainedDuplicates<'a> {
	/// Duplicate accounts represent repeated weights and are intentionally counted per entry.
	#[pina(remaining, distinct = false)]
	remaining: &'a mut [AccountView],
}

#[derive(Accounts)]
struct UnexplainedDuplicates<'a> {
	#[pina(remaining, distinct = false)]
	//~^ ERROR: `distinct = false` permits duplicate writable accounts without explaining why
	remaining: &'a mut [AccountView],
}

#[derive(Accounts)]
struct EmptyExplanation<'a> {
	/// Duplicates allowed.
	#[pina(remaining, distinct = false)]
	//~^ ERROR: `distinct = false` permits duplicate writable accounts without explaining why
	remaining: &'a mut [AccountView],
}

fn main() {}
