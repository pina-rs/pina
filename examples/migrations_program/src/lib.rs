#![allow(missing_docs)]
#![cfg_attr(not(feature = "fuzzing"), no_std)]

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

// Rent-exemption head room for on-demand growth: roughly 6,960 lamports per
// grown byte (3,480 per byte-year at the two-year exemption threshold). The
// budget must fund a whole stale ladder, not one step. `ManualState` grows
// three bytes across its v0→v1→v2 history (u8 → u16, then the compact
// `String<5>`), so the full reserved `Migrate` ladder needs exactly
// 3 × 6,960 = 20,880 lamports. The previous 20,000 cap funded the first step
// and then failed the cumulative check on the second — after the first effect
// had already landed, which aborts the instruction as a panic — stranding
// every v0 `ManualState` behind permanent `ProgramFailedToComplete` failures
// on both the reserved route and the inline `Update` path (security sweep
// finding N1). 24,000 covers the ladder with margin while keeping the
// two-`State` sweep (2 × 13,920 = 27,840) above the cap, which the surfpool
// suite pins as the shared-budget rejection.
// `pina migrations create` prints the same estimate when a transition grows.
const MAX_INLINE_MIGRATION_LAMPORTS: u64 = 24_000;

/// Instruction discriminator.
///
/// The reserved `Migrate` instruction's ladder is derived from the manifest:
/// one optional slot per enveloped account contract, in identity-sorted
/// order — `[payer, systemProgram, state, manualState, compactState]` — the
/// same order generated clients compose. An explicit `migrations(State, ...)`
/// list remains available when a program needs several same-contract slots in
/// one sweep under a shared lamport budget.
#[discriminator(
	entrypoint,
	// The explicit list keeps the batching demo alive: two `State` accounts
	// share one sweep under one lamport budget. Derived ladders (the default)
	// expose one slot per contract instead.
	migrations(State, ManualState, CompactState, State),
	migrations_max_lamports = MAX_INLINE_MIGRATION_LAMPORTS,
	// This program was measured with `#[inline]`; the unconditional hint adds
	// 240 bytes to the deployed binary without changing its dispatch cost.
	inline = "hint"
)]
pub enum MigrationInstruction {
	Update = 0,
	Relay = 1,
}

#[discriminator]
pub enum MigrationAccount {
	State = 1,
	ManualState = 2,
	CompactState = 3,
}

#[discriminator]
pub enum MigrationEvent {
	ValueChanged = 4,
}

#[account(discriminator = MigrationAccount::State)]
pub struct State {
	pub authority: Address,
	pub value: u64,
	pub enabled: bool,
	pub revision: u8,
}

#[account(discriminator = MigrationAccount::ManualState, compact)]
pub struct ManualState {
	pub code: String<5>,
}

#[account(discriminator = MigrationAccount::CompactState, compact)]
pub struct CompactState {
	pub name: String<4>,
	pub tags: Vec<u16, 2>,
}

// Opted into instruction migrations: the payload carries a version envelope,
// and the generated entrypoint converts an older client's request before the
// handler runs. Other instructions under the `auto` policy are only recorded.
#[instruction(discriminator = MigrationInstruction::Update, migrations)]
pub struct UpdateInstruction {
	pub value: u64,
	pub memo: u16,
}

#[event(discriminator = MigrationEvent::ValueChanged)]
pub struct ValueChangedEvent {
	pub value: u64,
	pub memo: u16,
}

#[instruction(discriminator = MigrationInstruction::Relay, migrations = false)]
pub struct RelayInstruction {
	pub value: u64,
}

pub struct MigrationProgram;

impl CpiProgramId for MigrationProgram {
	const ID: Address = ID;
}

#[derive(Accounts)]
pub struct UpdateAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a AccountView,
	pub referrer: Option<&'a AccountView>,
	pub state: Option<&'a mut AccountView>,
	#[pina(validate(signer))]
	pub migration_payer: Option<&'a mut AccountView>,
	pub system_program: Option<&'a AccountView>,
	pub manual_state: Option<&'a mut AccountView>,
	pub compact_state: Option<&'a mut AccountView>,
}

#[derive(Accounts)]
pub struct RelayAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a AccountView,
	pub referrer: &'a AccountView,
	pub state: &'a mut AccountView,
	#[pina(validate(signer))]
	pub migration_payer: &'a mut AccountView,
	pub system_program: &'a AccountView,
	pub migration_program: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		// The generated entrypoint already converted a historical request, so
		// `data` always holds the current layout.
		let instruction = UpdateInstruction::try_from_bytes(data)?;
		let _ = self.referrer;

		if self.state.is_none()
			&& self.migration_payer.is_none()
			&& self.system_program.is_none()
			&& self.manual_state.is_none()
			&& self.compact_state.is_none()
		{
			let _ = instruction.memo.get();
			return Ok(());
		}
		let (Some(state), payer, Some(system_program)) =
			(self.state, self.migration_payer, self.system_program)
		else {
			return Err(ProgramError::NotEnoughAccountKeys);
		};
		system_program.assert_address(&system::ID)?;
		let payer = payer.map(|account| &*account);
		MigrateAccount {
			account: state,
			payer,
			program_id: &ID,
			max_lamports: Some(MAX_INLINE_MIGRATION_LAMPORTS),
		}
		.invoke::<State>()?;

		// Mixed-version sets: each migratable account advances
		// independently inside the same instruction.
		if let Some(manual_state) = self.manual_state {
			MigrateAccount {
				account: manual_state,
				payer,
				program_id: &ID,
				max_lamports: Some(MAX_INLINE_MIGRATION_LAMPORTS),
			}
			.invoke::<ManualState>()?;
			manual_state.with_compact_account::<ManualState, _>(&ID, |manual| {
				if manual.code().is_empty() {
					return Err(ProgramError::InvalidAccountData);
				}
				Ok(())
			})?;
		}
		if let Some(compact_state) = self.compact_state {
			MigrateAccount {
				account: compact_state,
				payer,
				program_id: &ID,
				max_lamports: Some(MAX_INLINE_MIGRATION_LAMPORTS),
			}
			.invoke::<CompactState>()?;
		}

		let mut state = state.as_account_mut::<State>(&ID)?;
		if state.authority != *self.authority.address() {
			return Err(ProgramError::InvalidAccountData);
		}
		state.value.set(instruction.value.get());
		state.enabled = true.into();
		state.revision = state.revision.wrapping_add(1);

		let _ = instruction.memo.get();
		Ok(())
	}
}

impl<'a> ProcessAccountInfos<'a> for RelayAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let instruction = RelayInstruction::try_from_bytes(data)?;
		// The deployed self-CPI validates this account in `UpdateAccounts`. Keep
		// the well-known default visible to source-based IDL generation without
		// repeating that check on-chain.
		#[cfg(not(feature = "bpf-entrypoint"))]
		self.system_program.assert_address(&system::ID)?;
		let program = Program::<MigrationProgram>::try_new(self.migration_program)?;

		let mut historical = [0_u8; 10];
		historical[0] = MigrationInstruction::Update as u8;
		historical[1] = 0;
		historical[2..].copy_from_slice(instruction.value.as_ref());

		CpiContext::new(
			program,
			[
				CpiHandle::readonly_signer(self.authority),
				CpiHandle::readonly(self.referrer),
				CpiHandle::writable(self.state)?,
				CpiHandle::writable_signer(self.migration_payer)?,
				CpiHandle::readonly(self.system_program),
			],
		)
		.invoke(&historical)?;

		// The writable CPI may have resized and rewritten this account. Construct a
		// fresh typed guard instead of retaining any pre-CPI view.
		let state = self.state.as_account::<State>(&ID)?;
		if state.value != instruction.value || state.enabled.is_false() {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(())
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	dispatch_entrypoint!(MigrationInstruction);
}
