//! SECURE: Contained administrative powers.
//!
//! The same vault, rebuilt so that no single key can drain it:
//!
//! - sweeps are bounded by an on-chain circuit breaker (cap per window) and
//!   must leave a permanent reserve behind
//! - a dedicated guardian key can pause the program but cannot move funds
//! - unpausing needs dual control (guardian + authority)
//! - authority rotation is two-phase (propose, then a delayed accept) and can
//!   be cancelled during the delay
//!
//! A leaked authority key drains at most one window's cap and never empties
//! the vault; a leaked guardian key cannot move funds at all. Rotation cannot
//! take effect within one transaction, so an honest key always has a window to
//! cancel it.

#![allow(missing_docs)]
#![no_std]

#[cfg(all(not(any(target_os = "solana", target_arch = "bpf")), not(test)))]
extern crate std;

use pina::*;

declare_id!("FWVbUoNj1iZGp9caFp1a6TxgrM5KjxmgHVRiSHFYyZuJ");

/// Maximum lamports the circuit breaker lets out of the vault per window.
const WITHDRAWAL_CAP: u64 = 1_000_000;

/// Lamports the vault must always keep, so no window can empty it outright.
const VAULT_RESERVE: u64 = 10_000;

/// Length of one circuit-breaker window, in seconds.
const WINDOW_SECONDS: i64 = 3_600;

/// Delay between proposing an authority and accepting it, in seconds.
///
/// The window lets the guardian pause the program and the current authority
/// cancel a rotation before it takes effect.
const ROTATION_DELAY_SECONDS: i64 = 86_400;

const CLOCK_SYSVAR_ID: Address = address!("SysvarC1ock11111111111111111111111111111111");

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
	ProgramPaused = 0,
	WithdrawalCapExhausted = 1,
	NoPendingAuthority = 2,
	RotationDelayPending = 3,
	InsufficientVaultReserve = 4,
}

#[discriminator]
pub enum VaultInstruction {
	Sweep = 0,
	Pause = 1,
	Unpause = 2,
	ProposeAuthority = 3,
	AcceptAuthority = 4,
	CancelAuthority = 5,
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
	pub rotation_ready_at: i64,
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

#[instruction(discriminator = VaultInstruction, variant = CancelAuthority)]
pub struct CancelAuthorityInstruction {}

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
	pub clock: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct AcceptAuthorityAccounts<'a> {
	pub pending_authority: &'a AccountView,
	pub config: &'a mut AccountView,
	pub clock: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct CancelAuthorityAccounts<'a> {
	pub authority: &'a AccountView,
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

		// The sweep allowance belongs to this configuration's vault. Without
		// this binding any authority could spend its allowance against any
		// program-owned account: `send_owned` proves program ownership, not
		// membership in this configuration.
		if config.vault != *self.vault.address() {
			return Err(ProgramError::InvalidAccountData);
		}

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

		// A vault smaller than the cap would otherwise let one sweep empty it:
		// the cap only binds when the balance exceeds it. Keeping a reserve
		// makes "one instruction cannot empty the vault" true at every size,
		// so a leaked key can never leave the account at zero and closed.
		let balance = self.vault.lamports();
		let available = balance
			.checked_sub(VAULT_RESERVE)
			.ok_or(VaultError::InsufficientVaultReserve)?;
		let amount = available.min(remaining);

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
		self.clock.assert_sysvar(&CLOCK_SYSVAR_ID)?;

		let now = {
			let clock = sysvars::clock::Clock::from_account_view(self.clock)?;
			clock.unix_timestamp
		};

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;
		self.authority.assert_address(&config.authority)?;

		// Phase one of a two-phase rotation: only record the candidate, with a
		// delay before it can take effect so the guardian can pause and the
		// current authority can cancel.
		config.pending_authority = *self.new_authority.address();

		let ready_at = now
			.checked_add(ROTATION_DELAY_SECONDS)
			.ok_or(ProgramError::ArithmeticOverflow)?;

		config.rotation_ready_at.set(ready_at);
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for AcceptAuthorityAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = AcceptAuthorityInstruction::try_from_bytes(data)?;

		self.pending_authority.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;
		self.clock.assert_sysvar(&CLOCK_SYSVAR_ID)?;

		let now = {
			let clock = sysvars::clock::Clock::from_account_view(self.clock)?;
			clock.unix_timestamp
		};

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;

		if config.pending_authority == Address::new_from_array([0; ADDRESS_BYTES]) {
			return Err(VaultError::NoPendingAuthority.into());
		}

		self.pending_authority
			.assert_address(&config.pending_authority)?;

		// The delay is what gives the honest keys a reaction window; without
		// it, a leaked authority key could propose and accept in one
		// transaction and the two-phase flow would protect nothing.
		if now < config.rotation_ready_at.get() {
			return Err(VaultError::RotationDelayPending.into());
		}

		// Phase two: the proposed key proves control by signing.
		config.authority = *self.pending_authority.address();
		config.pending_authority = Address::new_from_array([0; ADDRESS_BYTES]);
		config.rotation_ready_at.set(0);

		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for CancelAuthorityAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = CancelAuthorityInstruction::try_from_bytes(data)?;

		self.authority.assert_signer()?;
		self.config.assert_not_empty()?.assert_writable()?;

		let mut config = self.config.as_account_mut::<VaultConfig>(&ID)?;

		// The current authority, or the guardian once paused, can cancel a
		// rotation during its delay window.
		let canceller = *self.authority.address();
		let permitted =
			canceller == config.authority || (config.paused.get() && canceller == config.guardian);

		if !permitted {
			return Err(ProgramError::MissingRequiredSignature);
		}

		self.authority.assert_address(&canceller)?;

		config.pending_authority = Address::new_from_array([0; ADDRESS_BYTES]);
		config.rotation_ready_at.set(0);

		Ok(())
	}
}
