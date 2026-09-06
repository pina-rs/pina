use pina::*;

#[discriminator(crate = ::pina)]
enum AccountType {
	Invalid = 1,
}

#[account(crate = ::pina, discriminator = AccountType, compact)]
struct InvalidCompactAccount {
	value: u64,
}

fn main() {}
