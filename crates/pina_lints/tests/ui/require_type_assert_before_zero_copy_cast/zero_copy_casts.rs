// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

#[repr(C)]
struct VaultData {
	amount: u64,
}

struct AccountView;
struct BorrowedView;

mod bytemuck {
	pub unsafe fn try_from_bytes<T>(bytes: &[u8]) -> Result<&'static T, ()> {
		unsafe { Ok(&*(bytes.as_ptr() as *const T)) }
	}

	pub unsafe fn cast_ref<T>(value: &T) -> Result<&T, ()> {
		Ok(value)
	}
}

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
	//~^ ERROR: raw zero-copy casts should be preceded by
	let parsed = unsafe { bytemuck::try_from_bytes::<VaultData>(bytes) }?;
	//~^ ERROR: raw zero-copy casts should be preceded by
	let _ = (view.amount, parsed.amount);
	Ok(())
}

fn process_borrowed_cast(data: &AccountView, bytes: &[u8]) -> Result<(), ()> {
	let guard = data.try_borrow()?;
	let view = guard.cast_ref()?;
	//~^ ERROR: raw zero-copy casts should be preceded by
	let parsed = unsafe { bytemuck::try_from_bytes::<VaultData>(bytes) }?;
	//~^ ERROR: raw zero-copy casts should be preceded by
	let _ = (view.amount, parsed.amount);
	Ok(())
}

fn process_asserted_cast(data: &AccountView) -> Result<(), ()> {
	data.assert_type::<VaultData>(&OWNER)?;
	let guard = data.try_borrow()?;
	let _ = guard;
	// A guard established on the borrowed receiver satisfies the cast.
	let view = unsafe { bytemuck::cast_ref::<VaultData>(&VaultData { amount: 0 }) }?;
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
