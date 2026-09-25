//! SEC-02 audit regression: compact resizable updates must enforce the
//! migration envelope even when the `validation` feature is disabled.
//!
//! The 2026-09-22 deep audit found that `UpdateResizableAccount` routes
//! through the generated account-level `T::update` contract only under the
//! `validation` feature; without it, the builder calls the raw pinapod patch,
//! rewrites the discriminator, and never checks or advances the migration
//! envelope. This crate builds `pina` with exactly
//! `--no-default-features --features derive,compact,account-resize` (see
//! `Cargo.toml`), so the test below executes that branch.
//!
//! The two manifest versions share one physical shape on purpose: a stale
//! v0 envelope is structurally compatible with the current representation,
//! which is precisely the case the bypass mis-applies. Running this test
//! fails on the current tree (the update is accepted and the stale envelope
//! survives); it must pass once the fix routes every branch through the
//! migration-aware update contract.

#![allow(non_camel_case_types)]
#![allow(unsafe_code)]

use pina::Address;
use pina::UpdateResizableAccount;
use pina::account;
use pina::discriminator;
use pina::*;
use pinocchio::AccountView;
use pinocchio::account::NOT_BORROWED;
use pinocchio::account::RuntimeAccount;

const OWNER: Address = Address::new_from_array([9; 32]);

#[discriminator(crate = ::pina)]
enum AuditAccountType {
	LedgerState = 1,
}

#[account(crate = ::pina, discriminator = AuditAccountType, compact)]
struct LedgerState {
	pub label: pina::String<8>,
	pub codes: pina::Vec<u16, 3>,
}

/// The runtime-shaped account buffer used by `crates/pina`'s own CPI tests:
/// the account metadata precedes the data in one allocation so `AccountView`
/// borrows both. `repr(C)` pins that field order, because
/// `AccountView::new_unchecked` reads the header and the data at fixed
/// offsets behind one pointer.
#[repr(C)]
struct TestAccount<const N: usize> {
	header: RuntimeAccount,
	data: [u8; N],
}

impl<const N: usize> TestAccount<N> {
	fn new(address: Address, owner: Address, is_signer: bool, is_writable: bool) -> Self {
		Self {
			header: RuntimeAccount {
				borrow_state: NOT_BORROWED,
				is_signer: u8::from(is_signer),
				is_writable: u8::from(is_writable),
				executable: 0,
				padding: [0; 4],
				address,
				owner,
				lamports: 1,
				data_len: N as u64,
			},
			data: [0u8; N],
		}
	}

	fn view(&mut self) -> AccountView {
		unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
	}
}

/// A stale migration envelope must be rejected by a compact resizable update
/// regardless of the `validation` feature, and a rejected update must leave
/// every account byte unchanged.
#[test]
fn audit_sec_02_stale_envelope_compact_update_is_rejected() {
	// The buffer is exactly the encoded size of the fixture value, so the
	// same-size patch performs no realloc and no rent adjustment: the only
	// thing under test is the migration-envelope contract.
	const STALE_ENCODED: usize = 5 + 2; // header + label "v0"
	let mut stored =
		TestAccount::<STALE_ENCODED>::new(Address::new_from_array([1; 32]), OWNER, false, true);
	LedgerState::initialize(&mut stored.data, &LedgerStatePatch::new().label("v0"))
		.unwrap_or_else(|error| panic!("initialize current compact state: {error:?}"));

	// Stamp the stale v0 envelope. The physical shape is identical, so this
	// buffer remains a valid *structural* representation — only the envelope
	// version byte is stale.
	stored.data[1] = 0;
	let before = stored.data;

	let mut rent_stored =
		TestAccount::<8>::new(Address::new_from_array([6; 32]), OWNER, true, true);
	let mut account = stored.view();
	let mut rent_account = rent_stored.view();

	let result = UpdateResizableAccount {
		account: &mut account,
		rent_account: &mut rent_account,
		program_id: &OWNER,
		patch: LedgerStatePatch::new().label("v1"),
	}
	.invoke::<LedgerState>();

	assert!(
		result.is_err(),
		"SEC-02: without the `validation` feature the update was accepted on a stale migration \
		 envelope; it must fail closed with MigrationRequired"
	);
	assert_eq!(
		stored.data, before,
		"a rejected update must leave every account byte unchanged"
	);
}
