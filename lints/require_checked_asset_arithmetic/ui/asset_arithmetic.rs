#![allow(dead_code)]

fn process(balance: u64, amount: u64) -> Result<u64, ()> {
	balance.checked_sub(amount).ok_or(())
}

fn process_unchecked(balance: u64, amount: u64) -> u64 {
	balance - amount
	//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
}

fn process_saturating(balance: u64, amount: u64) -> u64 {
	balance.saturating_sub(amount)
	//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
}

fn main() {}
