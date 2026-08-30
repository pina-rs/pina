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

fn main() {}
