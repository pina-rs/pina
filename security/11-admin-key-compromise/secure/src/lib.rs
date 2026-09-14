//! SECURE: Contained administrative powers.
//!
//! The same vault, rebuilt so that no single key can drain it:
//!
//! - sweeps are bounded by an on-chain circuit breaker (cap per window)
//! - a dedicated guardian key can pause the program but cannot move funds
//! - unpausing needs dual control (guardian + authority)
//! - authority rotation is two-phase (propose, then accept)
//!
//! A leaked authority key drains at most one window's cap before the guardian
//! pauses the program; a leaked guardian key cannot move funds at all.

#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("FWVbUoNj1iZGp9caFp1a6TxgrM5KjxmgHVRiSHFYyZuJ");

/// Maximum lamports the circuit breaker lets out of the vault per window.
const WITHDRAWAL_CAP: u64 = 1_000_000;

/// Length of one circuit-breaker window, in seconds.
const WINDOW_SECONDS: i64 = 3_600;

const CLOCK_SYSVAR_ID: Address = address!("SysvarC1ock11111111111111111111111111111111");

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
	ProgramPaused = 0,
	WithdrawalCapExhausted = 1,
	NoPendingAuthority = 2,
}

#[discriminator]
pub enum VaultInstruction {
	Sweep = 0,
	Pause = 1,
	Unpause = 2,
	ProposeAuthority = 3,
	AcceptAuthority = 4,
}

#[discriminator]
pub enum VaultAccount {
	VaultConfig = 1,
}

#[account(discriminator = VaultAccount)]
pub struct VaultConfig {
	pub authority: Address,
	pub pending_authority: Address,
	pub guardian: Address,
	pub vault: Address,
	pub paused: bool,
	pub window_start: i64,
	pub withdrawn_in_window: u64,
}

#[instruction(discriminator = VaultInstruction, variant = Sweep)]
pub struct SweepInstruction {}

#[instruction(discriminator = VaultInstruction, variant = Pause)]
pub struct PauseInstruction {}

#[instruction(discriminator = VaultInstruction, variant = Unpause)]
pub struct UnpauseInstruction {}

#[instruction(discriminator = VaultInstruction, variant = ProposeAuthority)]
pub struct ProposeAuthorityInstruction {}

#[instruction(discriminator = VaultInstruction, variant = AcceptAuthority)]
pub struct AcceptAuthorityInstruction {}

#[derive(Accounts, Debug)]
pub struct SweepAccounts<'a> {
	pub authority: &'a AccountView,
	pub config: &'a mut AccountView,
	pub vault: &'a mut AccountView,
	pub clock: &'a AccountView,
	pub recipient: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct PauseAccounts<'a> {
	pub guardian: &'a AccountView,
	pub config: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct UnpauseAccounts<'a> {
	pub guardian: &'a AccountView,
	pub authority: &'a AccountView,
	pub config: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct ProposeAuthorityAccounts<'a> {
	pub authority: &'a AccountView,
	pub new_authority: &'a AccountView,
	pub config: &'a mut AccountView,
}

#[derive(Accounts, Debug)]
pub struct AcceptAuthorityAccounts<'a> {
	pub pending_authority: &'a AccountView,
	pub config: &'a mut AccountView,
}

/// Halts privileged instructions while the guardian has paused the program.
fn assert_live(paused: bool) -> Result<(), VaultError> {
	if paused {
		return Err(VaultError::ProgramPaused);
	}

	Ok(())
}

/// Halts sweeps once the circuit breaker has spent the window's allowance.
fn assert_within_cap(remaining: u64) -> Result<(), VaultError> {
	if remaining == 0 {
		return Err(VaultError::WithdrawalCapExhausted);
	}

	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for SweepAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = SweepInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;
		self.vault.assert_not_empty()?.assert_writable()?;
		self.clock.assert_sysvar(&CLOCK_SYSVAR_ID)?;

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;
		self.authority.assert_address(&config.authority)?;

		assert_live(config.paused.get())?;

		// Start a fresh circuit-breaker window when the current one elapsed.
		let now = {
			let clock = sysvars::clock::Clock::from_account_view(self.clock)?;
			clock.unix_timestamp
		};

		if now - config.window_start.get() >= WINDOW_SECONDS {
			config.window_start.set(now);
			config.withdrawn_in_window.set(0);
		}

		// Never more than the window's remaining allowance leaves the vault,
		// even when the signing key is in attacker hands.
		let remaining = WITHDRAWAL_CAP
			.checked_sub(config.withdrawn_in_window.get())
			.ok_or(VaultError::WithdrawalCapExhausted)?;

		assert_within_cap(remaining)?;

		let amount = self.vault.lamports().min(remaining);

		if amount == 0 {
			return Err(VaultError::WithdrawalCapExhausted.into());
		}

		let withdrawn = config
			.withdrawn_in_window
			.get()
			.checked_add(amount)
			.ok_or(VaultError::WithdrawalCapExhausted)?;

		config.withdrawn_in_window.set(withdrawn);

		self.vault.send_owned(&ID, amount, self.recipient)?;

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for PauseAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = PauseInstruction::try_from_bytes(data)?;

		self.guardian.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;
		self.guardian.assert_address(&config.guardian)?;

		// The guardian's only power is defensive: pause everything. The
		// guardian key cannot sweep, rotate authority, or unpause alone, so
		// leaking it cannot move funds.
		config.paused.set(true);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for UnpauseAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = UnpauseInstruction::try_from_bytes(data)?;

		self.guardian.assert_signer()?;
		self.authority.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;
		self.guardian.assert_address(&config.guardian)?;
		self.authority.assert_address(&config.authority)?;

		// Dual control: neither a leaked authority key nor a leaked guardian
		// key can resume the program on its own.
		config.paused.set(false);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for ProposeAuthorityAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = ProposeAuthorityInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;
		self.authority.assert_address(&config.authority)?;

		// Phase one of a two-phase rotation: only record the candidate. The
		// rotation takes effect when the candidate itself accepts, so one
		// fooled signing ceremony cannot hand the program to an attacker.
		config.pending_authority = *self.new_authority.address();

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for AcceptAuthorityAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = AcceptAuthorityInstruction::try_from_bytes(data)?;

		self.pending_authority.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;

		if config.pending_authority == Address::new_from_array([0; ADDRESS_BYTES]) {
			return Err(VaultError::NoPendingAuthority.into());
		}

		self.pending_authority
			.assert_address(&config.pending_authority)?;

		// Phase two: the proposed key proves control by signing.
		config.authority = *self.pending_authority.address();
		config.pending_authority = Address::new_from_array([0; ADDRESS_BYTES]);

		Ok(())
	}
}
