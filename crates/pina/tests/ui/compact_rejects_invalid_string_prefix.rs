use pina::*;

#[discriminator(crate = ::pina)]
enum AccountType {
	Invalid = 1,
}

#[account(crate = ::pina, discriminator = AccountType, compact)]
struct InvalidCompactAccount {
	title: PodString<8, 3>,
}

fn main() {}
