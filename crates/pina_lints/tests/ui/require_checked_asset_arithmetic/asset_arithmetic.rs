// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

fn process(balance: u64, amount: u64) -> Result<u64, ()> {
	balance.checked_sub(amount).ok_or(())
}

fn process_non_asset(raw_value: u64) -> u64 {
	raw_value + 1
}

fn process_name_contains_asset_fragment(rebalance_attempts: u64, unstaked_epochs: u64) -> u64 {
	rebalance_attempts + unstaked_epochs
}

fn process_unchecked(balance: u64, amount: u64) -> u64 {
	balance - amount
	//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
}

fn process_saturating(balance: u64, amount: u64) -> u64 {
	balance.saturating_sub(amount)
	//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
}

fn process_saturating_alias(current: u64, amount: u64) -> u64 {
	current.saturating_sub(amount)
	//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
}

fn process_assignment(balance: &mut u64, amount: u64) {
	*balance += amount;
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

fn process_closure(balance: u64, amount: u64) -> u64 {
	let update = || balance - amount;
	//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
	update()
}

fn process_match_guard(balance: u64, amount: u64) -> u64 {
	match balance {
		value if balance - amount > 0 => value,
		//~^ ERROR: asset arithmetic can overflow, underflow, or silently saturate
		_ => 0,
	}
}

fn process_non_asset_saturating(raw_value: u64, offset: u64) -> u64 {
	raw_value.saturating_add(offset)
}

struct CustomAccumulator;

impl CustomAccumulator {
	fn saturating_add(self, _amount: u64) -> Self {
		self
	}
}

fn process_custom_method(accumulator: CustomAccumulator, amount: u64) -> CustomAccumulator {
	accumulator.saturating_add(amount)
}

struct AssetAmount(u64);

impl core::ops::Add for AssetAmount {
	type Output = Self;

	fn add(self, amount: Self) -> Self {
		Self(self.0.saturating_add(amount.0))
	}
}

impl core::ops::AddAssign for AssetAmount {
	fn add_assign(&mut self, amount: Self) {
		self.0 = self.0.saturating_add(amount.0);
	}
}

fn process_custom_operators(mut balance: AssetAmount, amount: AssetAmount) -> AssetAmount {
	balance += AssetAmount(1);
	balance + amount
}

fn process_non_asset_assignment(raw_value: &mut u64, offset: u64) {
	*raw_value += offset;
}

fn process_asset_replacement(balance: &mut u64, amount: u64) {
	*balance = amount;
}

fn process_non_asset_remainder_assignment(raw_value: &mut u64, offset: u64) {
	*raw_value %= offset;
}

fn main() {}

// compile-fail
