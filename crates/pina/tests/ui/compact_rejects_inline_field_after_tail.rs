use pina::*;

#[discriminator(crate = ::pina)]
enum AccountType {
	Invalid = 1,
}

#[account(crate = ::pina, discriminator = AccountType, compact)]
struct InvalidCompactAccount {
	values: Vec<u64, 4>,
	fixed_after_tail: u8,
}

fn main() {}
