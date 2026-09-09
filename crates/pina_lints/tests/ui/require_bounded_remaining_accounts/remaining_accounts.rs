// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

fn process(remaining: &[u8]) {
	for account in remaining.iter().take(8) {
		let _ = account;
	}
}

const MAX_REMAINING_ACCOUNTS: usize = 8;

fn process_rejects_oversized(remaining: &[u8]) -> Result<(), ()> {
	if remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	for account in remaining {
		let _ = account;
	}

	Ok(())
}

fn process_reversed_guard(remaining: &[u8]) -> Result<(), ()> {
	if MAX_REMAINING_ACCOUNTS < remaining.len() {
		return Err(());
	}

	for account in remaining {
		let _ = account;
	}

	Ok(())
}

fn process_unbounded(remaining: &[u8]) {
	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}
}

fn process_loop_initializer(remaining: &[u8]) {
	loop {
		let _value = {
			for account in remaining {
				//~^ ERROR: remaining accounts are processed without an explicit bound
				let _ = account;
			}
		};
		break;
	}
}

fn process_non_dominating_guard(remaining: &[u8], check_limit: bool) -> Result<(), ()> {
	if check_limit && remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

fn process_runtime_limit(remaining: &[u8], limit: usize) -> Result<(), ()> {
	if remaining.len() > limit {
		return Err(());
	}

	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

fn process_return_payload(remaining: &[u8]) {
	return for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	};
}

fn process_break_payload(remaining: &[u8]) {
	loop {
		break for account in remaining {
			//~^ ERROR: remaining accounts are processed without an explicit bound
			let _ = account;
		};
	}
}

fn process_runtime_take(remaining: &[u8], limit: usize) {
	for account in remaining.iter().take(limit) {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}
}

fn process_take_before_chain(remaining: &[u8]) {
	for account in remaining.iter().take(8).chain([].iter()) {
		let _ = account;
	}
}

fn process_take_before_into_iter_chain(remaining: &[u8]) {
	for account in remaining.iter().take(8).chain([&0_u8; 0].into_iter()) {
		let _ = account;
	}
}

fn process_take_on_other_iterator(remaining: &[u8], other: &[u8]) {
	for account in remaining.iter().chain(other.iter().take(8)) {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}
}

fn process_two_bounded_iterators(remaining: &[u8], other: &[u8]) {
	for account in remaining.iter().take(8).chain(other.iter().take(8)) {
		let _ = account;
	}
}

fn process_filter_after_guard(remaining: &[u8]) -> Result<(), ()> {
	if remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	for account in remaining.iter().filter(|_| true) {
		let _ = account;
	}

	Ok(())
}

struct Passthrough<'a> {
	remaining: core::slice::Iter<'a, u8>,
}

impl Passthrough<'_> {
	fn take(self, _: usize) -> Self {
		self
	}
}

impl<'a> Iterator for Passthrough<'a> {
	type Item = &'a u8;

	fn next(&mut self) -> Option<Self::Item> {
		self.remaining.next()
	}
}

fn passthrough(remaining: &[u8]) -> Passthrough<'_> {
	Passthrough {
		remaining: remaining.iter(),
	}
}

fn process_custom_take(remaining: &[u8]) {
	for account in passthrough(remaining).take(8) {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}
}

fn main() {}

// compile-fail
