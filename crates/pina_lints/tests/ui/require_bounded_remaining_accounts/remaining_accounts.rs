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

fn process_unbounded_alias(remaining: &[u8]) {
	let accounts = remaining;
	for account in accounts {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}
}

fn process_bounded_alias(remaining: &[u8]) -> Result<(), ()> {
	let accounts = remaining;
	if accounts.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	for account in accounts {
		let _ = account;
	}

	Ok(())
}

fn process_stored_bounded_iterator(remaining: &[u8]) {
	let accounts = remaining.iter().take(MAX_REMAINING_ACCOUNTS);

	for account in accounts {
		let _ = account;
	}
}

fn process_reassigned_after_guard<'a>(
	mut remaining: &'a [u8],
	attacker: &'a [u8],
) -> Result<(), ()> {
	if remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	remaining = attacker;
	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

fn replace_remaining<'a>(remaining: &mut &'a [u8], replacement: &'a [u8]) {
	*remaining = replacement;
}

fn process_mutably_borrowed_after_guard<'a>(
	mut remaining: &'a [u8],
	attacker: &'a [u8],
) -> Result<(), ()> {
	if remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	replace_remaining(&mut remaining, attacker);
	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

fn process_stored_mutable_reference_after_guard<'a>(
	mut remaining: &'a [u8],
	attacker: &'a [u8],
) -> Result<(), ()> {
	let remaining_ref = &mut remaining;
	if remaining_ref.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	replace_remaining(remaining_ref, attacker);
	for account in *remaining_ref {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

fn process_closure_replacement_after_guard<'a>(
	mut remaining: &'a [u8],
	attacker: &'a [u8],
) -> Result<(), ()> {
	if remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	let mut replace = || remaining = attacker;
	replace();
	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

struct RemainingHolder<'a> {
	remaining: &'a [u8],
}

impl<'a> RemainingHolder<'a> {
	fn replace(&mut self, replacement: &'a [u8]) {
		self.remaining = replacement;
	}
}

fn process_mutable_receiver_after_guard(remaining: &[u8], attacker: &[u8]) -> Result<(), ()> {
	let mut holder = RemainingHolder { remaining };
	if holder.remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	holder.replace(attacker);
	for account in holder.remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

fn process_branch_replacement_after_guard<'a>(
	mut remaining: &'a [u8],
	attacker: &'a [u8],
	replace: bool,
) -> Result<(), ()> {
	if remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	if replace {
		remaining = attacker;
	} else {
		let _ = remaining.first();
	}

	for account in remaining {
		//~^ ERROR: remaining accounts are processed without an explicit bound
		let _ = account;
	}

	Ok(())
}

fn process_bounded_assignment<'a>(remaining: &'a [u8], attacker: &'a [u8]) -> Result<(), ()> {
	if remaining.len() > MAX_REMAINING_ACCOUNTS {
		return Err(());
	}

	let mut accounts = attacker;
	let _ = accounts.len();
	accounts = remaining;
	for account in accounts {
		let _ = account;
	}

	Ok(())
}

struct AnalysisShapes<'a> {
	accounts: &'a [u8],
	count: usize,
}

fn exercise_analysis_shapes(remaining: &[u8]) {
	let mut count = 0;
	count += remaining.len();
	let _ = remaining[0];
	let _ = (remaining, count);
	if let Some(account) = remaining.first() {
		let _ = account;
	}
	match remaining.first() {
		Some(account) if *account == 0 => {}
		_ => {}
	}

	let base = AnalysisShapes {
		accounts: &[],
		count,
	};
	let _ = AnalysisShapes {
		accounts: remaining,
		..base
	};
}

fn main() {}

// compile-fail
