//! `#[account]` contract tests through the public macro.
//!
//! The macro generates a zeropod-backed account with a discriminator field,
//! `SIZE`, `try_from_bytes`, `initialize`, and a `HasDiscriminator` impl
//! linking back to the discriminator enum. These tests exercise that surface
//! through real invocations.

use pina::*;

#[discriminator]
#[derive(Debug)]
pub enum AccountDisc {
	ConfigState = 1,
	GameState = 2,
	DataAccount = 3,
	BalanceAccount = 4,
	Custom = 5,
	LargeState = 6,
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct ConfigState {
	pub version: u8,
	pub bump: u8,
}

#[account(crate = pina, discriminator = AccountDisc)]
#[derive(Debug)]
pub struct GameState {
	pub score: u8,
	pub level: u8,
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct DataAccount {
	pub authority: [u8; 32],
	pub data: [u8; 64],
	pub flags: [u8; 4],
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct BalanceAccount {
	pub owner: [u8; 32],
	pub amount: PodU64,
	pub decimals: u8,
	pub is_frozen: PodBool,
}

#[account(crate = pina, discriminator = AccountDisc, variant = Custom)]
pub struct MyStruct {
	pub value: u8,
}

#[account(crate = pina, discriminator = AccountDisc)]
pub struct LargeState {
	pub authority: [u8; 32],
	pub bump: u8,
	pub treasury_bump: u8,
	pub mint_bump: u8,
	pub version: u8,
	pub padding: [u8; 3],
	pub total_supply: PodU64,
	pub name: [u8; 32],
}

/// A basic account round-trips through initialize and try_from_bytes.
#[test]
fn basic_roundtrip() {
	let mut bytes: std::vec::Vec<u8> = std::vec![0xFF; ConfigState::SIZE];
	{
		let view = ConfigState::initialize(&mut bytes).unwrap();
		view.version = 42;
		view.bump = 255;
	}
	let parsed = ConfigState::try_from_bytes(&bytes).unwrap();
	assert_eq!(parsed.version, 42);
	assert_eq!(parsed.bump, 255);
}

/// An account with existing derives keeps them alongside the generated impls.
#[test]
fn with_existing_derives() {
	let mut bytes: std::vec::Vec<u8> = std::vec![0; GameState::SIZE];
	GameState::initialize(&mut bytes).unwrap();
	let parsed = GameState::try_from_bytes(&bytes).unwrap();
	assert_eq!(parsed.score, 0);
}

/// Array fields contribute their exact byte length to `SIZE`.
#[test]
fn with_array_fields() {
	assert_eq!(DataAccount::SIZE, 1 + 32 + 64 + 4);
	let mut bytes: std::vec::Vec<u8> = std::vec![0; DataAccount::SIZE];
	DataAccount::initialize(&mut bytes).unwrap();
	assert!(DataAccount::try_from_bytes(&bytes).is_ok());
}

/// Pod types (`PodU64`, `PodBool`) contribute their zeropod byte widths.
#[test]
fn with_pod_types() {
	assert_eq!(BalanceAccount::SIZE, 1 + 32 + 8 + 1 + 1);
	let mut bytes: std::vec::Vec<u8> = std::vec![0; BalanceAccount::SIZE];
	BalanceAccount::initialize(&mut bytes).unwrap();
	assert!(BalanceAccount::try_from_bytes(&bytes).is_ok());
}

/// A custom variant name resolves in the `HasDiscriminator` impl.
#[test]
fn with_custom_variant() {
	let disc = &<MyStruct as HasDiscriminator>::VALUE;
	assert_eq!(*disc, AccountDisc::Custom);
}

/// A path variant resolves identically.
#[test]
fn with_path_variant() {
	let mut bytes: std::vec::Vec<u8> = std::vec![0; MyStruct::SIZE];
	MyStruct::initialize(&mut bytes).unwrap();
	assert!(MyStruct::try_from_bytes(&bytes).is_ok());
}

/// Many fields produce the correct cumulative `SIZE`.
#[test]
fn many_fields() {
	assert_eq!(LargeState::SIZE, 1 + 32 + 1 + 1 + 1 + 1 + 3 + 8 + 32);
	let mut bytes: std::vec::Vec<u8> = std::vec![0; LargeState::SIZE];
	LargeState::initialize(&mut bytes).unwrap();
	assert!(LargeState::try_from_bytes(&bytes).is_ok());
}

/// `try_from_bytes` rejects wrong-length and wrong-discriminator input.
#[test]
fn rejects_bad_input() {
	assert!(ConfigState::try_from_bytes(&[0; 2]).is_err());
	assert!(ConfigState::try_from_bytes(&[0; 4]).is_err());
	// Wrong discriminator: use all-zero for a mismatch.
	let mut bytes: std::vec::Vec<u8> = std::vec![0; ConfigState::SIZE];
	bytes[0] = 0xFF;
	assert!(ConfigState::try_from_bytes(&bytes).is_err());
}
