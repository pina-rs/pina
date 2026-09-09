// aux-build: bytemuck.rs
// aux-build: pina.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

extern crate bytemuck;
extern crate pina;

use bytemuck::try_from_bytes as parse_bytes;
use pina::ProcessAccountInfos;

#[repr(C)]
struct VaultData {
	amount: u64,
}

struct AccountView;
struct BorrowedView;

const OWNER: () = ();

impl AccountView {
	fn assert_type<T>(&self, _program: &()) -> Result<(), ()> {
		Ok(())
	}

	fn try_borrow(&self) -> Result<BorrowedView, ()> {
		Ok(BorrowedView)
	}

	fn cast_ref(&self) -> Result<&'static VaultData, ()> {
		Ok(&VaultData { amount: 0 })
	}
}

impl BorrowedView {
	fn cast_ref(&self) -> Result<&'static VaultData, ()> {
		Ok(&VaultData { amount: 0 })
	}

	fn try_from_bytes(_bytes: &[u8]) -> Result<VaultData, ()> {
		// Associated conversion helpers are treated as safe conversions, so
		// this body compiles without lint friction.
		Ok(VaultData { amount: 0 })
	}
}

mod direct_handler {
	use super::*;

	pub fn process(data: &AccountView, bytes: &[u8]) -> Result<(), ()> {
		let view = data.cast_ref()?;
		let parsed = bytemuck::try_from_bytes::<VaultData>(bytes)?;
		let _ = (view.amount, parsed.amount);
		Ok(())
	}
}

fn process_instruction(data: &AccountView, bytes: &[u8]) -> Result<(), ()> {
	let guard = data.try_borrow()?;
	let view = guard.cast_ref()?;
	let parsed = parse_bytes::<VaultData>(bytes)?;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let _ = (view.amount, parsed.amount);
	Ok(())
}

struct Handler;

impl ProcessAccountInfos for Handler {
	fn process(self, bytes: &[u8]) -> Result<(), ()> {
		let data = AccountView;
		data.assert_type::<VaultData>(&OWNER)?;
		let guard = data.try_borrow()?;
		let _ = guard;
		// Validation does not bind this unrelated raw cast to the checked account.
		let view = bytemuck::try_from_bytes::<VaultData>(bytes)?;
		//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
		let _ = view.amount;
		Ok(())
	}
}

impl Handler {
	fn process(_bytes: &[u8]) -> Result<(), ()> {
		let _ = bytemuck::cast_ref::<VaultData>(&VaultData { amount: 0 })?;
		Ok(())
	}
}

// Similar substrings do not make ordinary conversion helpers instruction
// handlers. Resolving the callee and handler boundary avoids deny-by-default
// false positives in off-chain or domain code.
fn process_accounting(bytes: &[u8]) -> Result<(), ()> {
	let _ = bytemuck::try_from_bytes::<VaultData>(bytes)?;
	Ok(())
}

fn instruction_builder(bytes: &[u8]) -> Result<(), ()> {
	let _ = parse_bytes::<VaultData>(bytes)?;
	Ok(())
}

// The late-lint callback also visits closure bodies. They are not named
// instruction handlers and must be ignored without querying an item name.
fn closure_conversion(bytes: &[u8]) -> Result<(), ()> {
	let convert = || bytemuck::try_from_bytes::<VaultData>(bytes);
	let _ = convert()?;
	Ok(())
}

fn framework_conversion(bytes: &[u8]) -> Result<(), ()> {
	// Path-call conversions like `AccountData::try_from_bytes()` are the
	// framework's safe form and stay exempt, unlike receiver-shaped casts.
	let view = BorrowedView::try_from_bytes(bytes)?;
	let _ = view.amount;
	Ok(())
}

fn main() {}
