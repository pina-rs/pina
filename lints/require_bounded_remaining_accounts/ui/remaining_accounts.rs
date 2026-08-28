#![allow(dead_code)]

fn process(remaining: &[u8]) {
	for account in remaining.iter().take(8) {
		let _ = account;
	}
}

fn process_unbounded(remaining: &[u8]) {
	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}
}

fn main() {}
