// aux-build: bytemuck.rs
// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

extern crate bytemuck;

use bytemuck::try_from_bytes as parse_bytes;

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

fn process_cast_without_guard(data: &AccountView, bytes: &[u8]) -> Result<(), ()> {
	let view = data.cast_ref()?;
	let parsed = bytemuck::try_from_bytes::<VaultData>(bytes)?;
	//~^ ERROR: raw zero-copy casts should be preceded by
	let _ = (view.amount, parsed.amount);
	Ok(())
}

fn process_borrowed_cast(data: &AccountView, bytes: &[u8]) -> Result<(), ()> {
	let guard = data.try_borrow()?;
	let view = guard.cast_ref()?;
	let parsed = parse_bytes::<VaultData>(bytes)?;
	//~^ ERROR: raw zero-copy casts should be preceded by
	let _ = (view.amount, parsed.amount);
	Ok(())
}

fn process_assertion_does_not_guard_cast(data: &AccountView) -> Result<(), ()> {
	data.assert_type::<VaultData>(&OWNER)?;
	let guard = data.try_borrow()?;
	let _ = guard;
	// Validation does not bind this unrelated raw cast to the checked account.
	let view = bytemuck::cast_ref::<VaultData>(&VaultData { amount: 0 })?;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let _ = view.amount;
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
