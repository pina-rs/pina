use pina::*;

#[discriminator]
pub enum AccountKind {
	TupleAccount = 0,
}

#[account(discriminator = AccountKind, variant = TupleAccount)]
pub struct TupleAccount(u8);

fn main() {}
