#![deny(warnings)]
#![no_std]

use pina::*;

/// Trailing optionals are the supported shape: nothing positional follows
/// them, so a dropped filler cannot shift any binding.
#[derive(Accounts)]
pub struct TrailingOptionals<'a> {
	pub authority: &'a AccountView,
	pub vault: &'a mut AccountView,
	pub watcher: Option<&'a AccountView>,
	pub note: Option<&'a mut AccountView>,
}

/// An optional may also precede a trailing `remaining` slice: the slice
/// absorbs whatever follows, so no binding depends on filler presence.
#[derive(Accounts)]
pub struct OptionalBeforeRemaining<'a> {
	pub authority: &'a AccountView,
	pub treasury: Option<&'a mut AccountView>,
	#[pina(remaining)]
	pub members: &'a [AccountView],
}

fn main() {}
