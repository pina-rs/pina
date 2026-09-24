use pina::*;

// The implicit discriminant after the last permitted code crosses into the
// reserved range even though no variant spells a reserved value.
#[error]
pub enum MyError {
	Highest = 0xFFFE_FFFF,
	Crosses,
}

fn main() {}
