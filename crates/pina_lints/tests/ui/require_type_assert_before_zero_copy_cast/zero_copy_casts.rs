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

fn process_instruction(data: &AccountView, bytes: &mut [u8]) -> Result<(), ()> {
	let guard = data.try_borrow()?;
	let view = guard.cast_ref()?;
	let parsed_amount = parse_bytes::<VaultData>(bytes)?.amount;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let indirect: fn(&[u8]) -> Result<&VaultData, ()> = parse_bytes::<VaultData>;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let indirect_parsed_amount = indirect(bytes)?.amount;
	let closure_amount = (|| bytemuck::from_bytes::<VaultData>(bytes).amount)();
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let cast_value = bytemuck::cast::<VaultData>([0; 8]);
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let fallible_value = bytemuck::try_cast::<VaultData>([0; 8])?;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let unchecked_amount = bytemuck::from_bytes::<VaultData>(bytes).amount;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let unchecked_mut_amount = bytemuck::from_bytes_mut::<VaultData>(bytes).amount;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let fallible_from_mut_amount = bytemuck::try_from_bytes_mut::<VaultData>(bytes)?.amount;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let copied = bytemuck::pod_read_unaligned::<VaultData>(bytes);
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let fallible_copy = bytemuck::try_pod_read_unaligned::<VaultData>(bytes)?;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let aligned_len = bytemuck::pod_align_to::<VaultData>(bytes).1.len();
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let aligned_mut_len = bytemuck::pod_align_to_mut::<VaultData>(bytes).1.len();
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let direct_cast = bytemuck::cast_ref::<VaultData>(&VaultData { amount: 0 })?;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let mut mutable_value = VaultData { amount: 0 };
	let mutable_cast_amount = bytemuck::cast_mut::<VaultData>(&mut mutable_value)?.amount;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let fallible_cast = bytemuck::try_cast_ref::<VaultData>(&VaultData { amount: 0 })?;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let fallible_cast_mut_amount = bytemuck::try_cast_mut::<VaultData>(&mut mutable_value)?.amount;
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let slice_len = bytemuck::cast_slice::<VaultData>(bytes).len();
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let slice_mut_len = bytemuck::cast_slice_mut::<VaultData>(bytes).len();
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let fallible_slice_len = bytemuck::try_cast_slice::<VaultData>(bytes)?.len();
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let fallible_slice_mut_len = bytemuck::try_cast_slice_mut::<VaultData>(bytes)?.len();
	//~^ ERROR: raw zero-copy account casts bypass guard-backed account validation
	let _ = (view.amount, parsed_amount);
	let _ = (
		indirect_parsed_amount,
		closure_amount,
		cast_value.amount,
		fallible_value.amount,
	);
	let _ = (
		unchecked_amount,
		unchecked_mut_amount,
		fallible_from_mut_amount,
		copied.amount,
		fallible_copy.amount,
		aligned_len,
		aligned_mut_len,
		direct_cast.amount,
		mutable_cast_amount,
		fallible_cast.amount,
		fallible_cast_mut_amount,
		slice_len,
		slice_mut_len,
		fallible_slice_len,
		fallible_slice_mut_len,
	);
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
