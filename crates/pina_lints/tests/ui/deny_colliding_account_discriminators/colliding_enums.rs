// aux-build: pina.rs
// normalize-stderr-test: "\n$" -> ""

// compile-fail

#![allow(dead_code)]

extern crate pina;

#[repr(u8)]
pub enum VaultKind {
	Vault = 31,
}

#[repr(u8)]
pub enum RegistryKind {
	Registry = 31,
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
	//~^ ERROR: account discriminator value 31 (1 byte) is also claimed by AdminRegistry
}

impl pina::PinaAccount for AdminRegistry {}

impl pina::HasDiscriminator for AdminRegistry {
	type Type = RegistryKind;

	const VALUE: RegistryKind = RegistryKind::Registry;
	//~^ ERROR: account discriminator value 31 (1 byte) is also claimed by VaultLedger
}

fn main() {}
