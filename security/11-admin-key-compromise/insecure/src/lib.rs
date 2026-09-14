//! INSECURE: Ungated administrative powers.
//!
//! A single `authority` key can sweep the vault's entire balance to any
//! recipient and rotate authority to any address in one transaction. If that
//! key leaks, the whole program is drained instantly; if it is lost, the
//! funds are frozen forever.

#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("GfwrpoXN9FhCdkexx7EQdjRkxD9X38AqamPW3rGX47qF");

#[discriminator]
pub enum VaultInstruction {
	Sweep = 0,
	RotateAuthority = 1,
}

#[discriminator]
pub enum VaultAccount {
	VaultConfig = 1,
}

#[account(discriminator = VaultAccount)]
pub struct VaultConfig {
	pub authority: Address,
	pub vault: Address,
}

#[instruction(discriminator = VaultInstruction, variant = Sweep)]
pub struct SweepInstruction {}

#[instruction(discriminator = VaultInstruction, variant = RotateAuthority)]
pub struct RotateAuthorityInstruction {}

#[derive(Accounts, Debug)]
pub struct SweepAccounts<'a> {
	pub authority: &'a AccountView,
	pub config: &'a AccountView,
	pub vault: &'a mut AccountView,
	pub recipient: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct RotateAuthorityAccounts<'a> {
	pub authority: &'a AccountView,
	pub new_authority: &'a AccountView,
	pub config: &'a mut AccountView,
}

impl<'a> ProcessAccountInfos<'a> for SweepAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = SweepInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.config.assert_not_empty()?;
		self.vault.assert_not_empty()?.assert_writable()?;

		let vault_address = {
			let config = self.config.as_account::<VaultConfig>(&ID)?;

			if config.vault != *self.vault.address() {
				return Err(ProgramError::InvalidAccountData);
			}

			config.authority
		};

		self.authority.assert_address(&vault_address)?;

		// BUG: The whole vault balance leaves the program in a single
		// instruction. Nothing on-chain can slow the drain: no pause switch,
		// no withdrawal cap, and no second key that can halt the program. One
		// leaked `authority` key — Raydium's trojan-infected AMM authority in
		// 2022, DEXX's custodial keys in 2024 — is enough to empty every
		// depositor's funds.
		self.vault
			.send_owned(&ID, self.vault.lamports(), self.recipient)?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RotateAuthorityAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = RotateAuthorityInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;
		self.new_authority.assert_signer()?;

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;
		self.authority.assert_address(&config.authority)?;

		// BUG: One signature permanently redirects every privileged path to
		// `new_authority`. A single fooled or compromised signing ceremony
		// (Radiant 2024, Bybit 2025) is enough to take over the program, and
		// there is no path back if the current key is simply lost.
		config.authority = *self.new_authority.address();

		Ok(())
	}
}
