//! `#[derive(Accounts)]` contract tests through the public derive.
//!
//! The derive generates `TryFrom<(&Address, &mut [AccountView])> for` the
//! accounts struct, mapping positional accounts to named fields. The tests
//! here are compile-time checks: they prove the derive expands for each
//! declaration shape and that the generated types exist. Runtime behavior
//! is covered by the Surfpool integration tests in `crates/pina_test`.

use pina::ParseAccounts;
use pina::*;

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct InitAccounts<'a> {
	pub payer: &'a AccountView,
	pub config: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct TransferAccounts<'a> {
	pub authority: &'a AccountView,
	pub source: &'a AccountView,
	pub destination: &'a AccountView,
	#[pina(remaining)]
	pub extra: &'a [AccountView],
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct MutableTransferAccounts<'a> {
	pub authority: &'a mut AccountView,
	#[pina(remaining)]
	pub extra: &'a mut [AccountView],
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct DuplicateMutableRemainingAccounts<'a> {
	pub authority: &'a AccountView,
	/// Duplicate accounts represent repeated weights in this instruction.
	#[pina(remaining, distinct = false)]
	pub extra: &'a mut [AccountView],
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct SingleAccount<'a> {
	pub account: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct EscrowAccounts<'a> {
	pub maker: &'a AccountView,
	pub escrow: &'a AccountView,
	pub mint_a: &'a AccountView,
	pub mint_b: &'a AccountView,
	pub maker_ata_a: &'a AccountView,
	pub vault: &'a AccountView,
	pub token_program: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct DefaultCrateAccounts<'a> {
	pub authority: &'a AccountView,
	pub data: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct MakeAccounts<'a> {
	pub maker: &'a mut AccountView,
	pub escrow: Option<&'a mut AccountView>,
	pub witness: Option<&'a AccountView>,
	pub system_program: &'a AccountView,
}

/// A basic three-account layout derives without error.
#[test]
fn basic() {
	// Compile-time proof: the type exists and has the expected size bound.
	#[allow(dead_code)]
	fn assert_parses<'a, T: ParseAccounts<'a>>() {}
	assert_parses::<InitAccounts>();
}

/// `#[pina(remaining)]` derives with a trailing slice field.
#[test]
fn with_remaining() {
	assert_parses::<TransferAccounts>();
}

/// Mutable remaining accounts derive without error.
#[test]
fn with_mutable_remaining() {
	assert_parses::<MutableTransferAccounts>();
}

/// `distinct = false` is documented on the field and accepted.
#[test]
fn with_duplicate_mutable_remaining() {
	assert_parses::<DuplicateMutableRemainingAccounts>();
}

/// A single-field layout derives without error.
#[test]
fn single_field() {
	assert_parses::<SingleAccount>();
}

/// Many fields derive without error.
#[test]
fn many_fields() {
	assert_parses::<EscrowAccounts>();
}

/// The default crate path resolves without an explicit `crate` argument.
#[test]
fn default_crate() {
	assert_parses::<DefaultCrateAccounts>();
}

/// Optional accounts accept `Option<&mut AccountView>` and
/// `Option<&AccountView>` fields.
#[test]
fn with_optional_accounts() {
	assert_parses::<MakeAccounts>();
}

/// Helper to prove the derive generated a usable `ParseAccounts` impl.
#[allow(dead_code)]
fn assert_parses<'a, T: ParseAccounts<'a>>() {}
