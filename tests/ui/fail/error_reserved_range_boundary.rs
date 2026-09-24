use pina::*;

// `0xFFFF_0000` is the first code Pina reserves for its framework errors.
#[error]
pub enum MyError {
	Valid = 0,
	Boundary = 0xFFFF_0000,
}

fn main() {}
