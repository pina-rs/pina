use pina::*;

#[derive(Accounts)]
pub struct BasicAccounts<'a> {
	pub payer: &'a AccountView,
}

#[derive(Accounts)]
pub struct DefaultDistinctRemaining<'a> {
	#[pina(remaining)]
	pub remaining: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct ExplicitDistinctRemaining<'a> {
	#[pina(remaining, distinct)]
	pub remaining: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct DuplicateRemaining<'a> {
	/// Duplicate entries intentionally represent repeated weighted positions.
	#[pina(remaining, distinct = false)]
	pub remaining: &'a mut [AccountView],
}

fn main() {}
