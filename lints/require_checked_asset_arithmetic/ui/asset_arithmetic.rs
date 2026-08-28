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

fn process_return_payload(balance: u64, amount: u64) -> u64 {
	return balance - amount;
	//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
}

fn process_loop_initializer(balance: u64, amount: u64) {
	loop {
		let _next_balance = balance - amount;
		//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
		break;
	}
}

fn process_break_payload(balance: u64, amount: u64) -> u64 {
	loop {
		break balance - amount;
		//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
	}
}

fn main() {}
