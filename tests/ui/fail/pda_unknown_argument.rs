use pina::*;

#[account(discriminator = TestAccount)]
#[pda(seeds = [b"test"], unknown = true)]
pub struct TestAccount {}

fn main() {}
