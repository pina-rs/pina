// aux-build: pina.rs
// normalize-stderr-test: "\n$" -> ""

// check-pass

#![allow(dead_code)]

extern crate pina;

#[repr(u8)]
pub enum VaultKind {
	Vault = 31,
}

#[repr(u8)]
pub enum RegistryKind {
	Registry = 32,
}

pub struct VaultLedger {
	pub authority: [u8; 32],
	pub amount: u64,
}

pub struct AdminRegistry {
	pub admin: [u8; 32],
	pub nonce: u64,
}

impl pina::PinaAccount for VaultLedger {}

impl pina::HasDiscriminator for VaultLedger {
	type Type = VaultKind;

	const VALUE: VaultKind = VaultKind::Vault;
}

impl pina::PinaAccount for AdminRegistry {}

impl pina::HasDiscriminator for AdminRegistry {
	type Type = RegistryKind;

	const VALUE: RegistryKind = RegistryKind::Registry;
}

// Events share the `HasDiscriminator` impl shape but live in the log
// namespace, not account data: sharing a value with an account type is not
// a type-cosplay path and must stay unflagged.
pub struct PriceEvent {
	pub price: u64,
}

impl pina::HasDiscriminator for PriceEvent {
	type Type = RegistryKind;

	const VALUE: RegistryKind = RegistryKind::Registry;
}

fn main() {}
