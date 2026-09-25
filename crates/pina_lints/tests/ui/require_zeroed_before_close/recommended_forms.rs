// aux-build: solana_account_view.rs
// check-pass

//! The exact forms the lint's help text recommends, against the real-shaped
//! `AccountView` data borrow, produce no diagnostic.

#![allow(dead_code)]

extern crate solana_account_view;

use solana_account_view::AccountView;
use solana_account_view::ProgramError;

const ID: () = ();

trait CloseAccountWithRecipient {
	fn close_with_recipient(
		&mut self,
		program_id: &(),
		recipient: &mut AccountView,
	) -> Result<(), ProgramError>;
	fn close_account_zeroed(
		&mut self,
		program_id: &(),
		recipient: &mut AccountView,
	) -> Result<(), ProgramError>;
}

impl CloseAccountWithRecipient for AccountView {
	fn close_with_recipient(&mut self, _: &(), _: &mut AccountView) -> Result<(), ProgramError> {
		self.close()
	}

	fn close_account_zeroed(&mut self, _: &(), _: &mut AccountView) -> Result<(), ProgramError> {
		self.close()
	}
}

fn process_combined(
	account: &mut AccountView,
	recipient: &mut AccountView,
) -> Result<(), ProgramError> {
	account.close_account_zeroed(&ID, recipient)
}

fn process_separate(
	account: &mut AccountView,
	recipient: &mut AccountView,
) -> Result<(), ProgramError> {
	account.try_borrow_mut()?.fill(0);
	account.close_with_recipient(&ID, recipient)
}

fn process_separate_raw_close(account: &mut AccountView) -> Result<(), ProgramError> {
	account.try_borrow_mut()?.fill(0);
	account.close()
}

fn main() {}
