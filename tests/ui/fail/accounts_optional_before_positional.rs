#![deny(warnings)]
#![no_std]

use pina::*;

#[derive(Accounts)]
pub struct ShiftedAccounts<'a> {
	pub authority: &'a AccountView,
	/// A mid-list optional: dropping its filler shifts `vault` by one slot.
	pub watcher: Option<&'a AccountView>,
	pub vault: &'a mut AccountView,
}

fn main() {}
