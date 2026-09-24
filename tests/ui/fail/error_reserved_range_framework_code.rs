use pina::*;

// `0xFFFF_FFF5` is `PinaProgramError::MigrationBudgetExceeded`; a client could
// not tell the two errors apart.
#[error]
pub enum MyError {
	Valid = 0,
	Collides = 0xFFFF_FFF5,
}

fn main() {}
