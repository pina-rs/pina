use pina::*;

const ID: Address = address!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(crate = ::pina)]
enum AccountType {
	CompactState = 1,
}

#[account(crate = ::pina, discriminator = AccountType, compact)]
#[pda(
	crate = ::pina,
	seeds = [b"compact", authority: Address],
	bump = bump,
)]
struct CompactState {
	pub bump: u8,
	pub values: Vec<u64, 4>,
}

fn leak_view<'account>(
	account: &'account AccountView,
	authority: &Address,
) -> Result<CompactStateRef<'account>, ProgramError> {
	CompactState::with_pda(account, authority, &ID, |state| Ok(state))
}

fn main() {}
