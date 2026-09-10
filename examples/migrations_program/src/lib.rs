#![cfg_attr(not(feature = "fuzzing"), no_std)]

use pina::*;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

const MAX_INLINE_MIGRATION_LAMPORTS: u64 = 20_000;

#[discriminator]
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

#[account(discriminator = MigrationAccount::State, migrations)]
pub struct State {
	pub authority: Address,
	pub value: u64,
	pub enabled: bool,
	pub revision: u8,
}

#[account(discriminator = MigrationAccount::ManualState, compact, migrations)]
pub struct ManualState {
	pub code: String<5>,
}

#[account(discriminator = MigrationAccount::CompactState, compact, migrations)]
pub struct CompactState {
	pub name: String<4>,
	pub tags: Vec<u16, 2>,
}

#[instruction(discriminator = MigrationInstruction::Update, migrations)]
pub struct UpdateInstruction {
	pub value: u64,
	pub memo: u16,
}

#[event(discriminator = MigrationEvent::ValueChanged, migrations)]
pub struct ValueChangedEvent {
	pub value: u64,
	pub memo: u16,
}

#[instruction(discriminator = MigrationInstruction::Relay)]
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
		UpdateInstruction::with_current_instruction_data(data, |current| {
			let instruction = UpdateInstruction::try_from_bytes(current)?;
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
				max_lamports: MAX_INLINE_MIGRATION_LAMPORTS,
			}
			.invoke::<State>()?;

			// Mixed-version sets: each migratable account advances
			// independently inside the same instruction.
			if let Some(manual_state) = self.manual_state {
				MigrateAccount {
					account: manual_state,
					payer,
					program_id: &ID,
					max_lamports: MAX_INLINE_MIGRATION_LAMPORTS,
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
					max_lamports: MAX_INLINE_MIGRATION_LAMPORTS,
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
		})
	}
}

impl<'a> ProcessAccountInfos<'a> for RelayAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let instruction = RelayInstruction::try_from_bytes(data)?;
		self.system_program.assert_address(&system::ID)?;
		let program = Program::<MigrationProgram>::try_new(self.migration_program)?;

		let mut historical = [0_u8; 10];
		historical[0] = MigrationInstruction::Update as u8;
		historical[1] = 0;
		historical[2..].copy_from_slice(&instruction.value.get().to_le_bytes());

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
		if state.value.get() != instruction.value.get() || !bool::from(state.enabled) {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(())
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: MigrationInstruction = parse_instruction(program_id, &ID, data)?;
		match instruction {
			MigrationInstruction::Update => {
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}
			MigrationInstruction::Relay => {
				RelayAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn generated_instruction_migration_accepts_version_zero_exactly() {
		let mut old = [0_u8; 10];
		old[0] = MigrationInstruction::Update as u8;
		old[1] = 0;
		old[2..].copy_from_slice(&42_u64.to_le_bytes());
		let mut workspace = [0xaa; 12];

		let current = normalize_instruction_data::<UpdateInstruction>(&old, &mut workspace)
			.unwrap_or_else(|error| panic!("normalize instruction: {error:?}"));
		assert!(current.was_migrated());
		// The historical v0 request walks both adjacent instruction
		// transitions and lands on the current v2 envelope.
		assert_eq!(current.as_bytes()[0..2], [0, 2]);
		assert_eq!(&current.as_bytes()[2..10], &42_u64.to_le_bytes());
		assert_eq!(&current.as_bytes()[10..12], &[0, 0]);
	}

	#[test]
	fn generated_instruction_migration_rejects_trailing_bytes() {
		let mut old = [0_u8; 11];
		old[0] = MigrationInstruction::Update as u8;
		old[1] = 0;
		let mut workspace = [0xaa; 12];

		assert_eq!(
			normalize_instruction_data::<UpdateInstruction>(&old, &mut workspace),
			Err(ProgramError::InvalidInstructionData),
		);
	}

	#[test]
	fn generated_event_projection_preserves_provenance_and_defaults_new_fields() {
		let mut old = [0_u8; 10];
		old[0] = MigrationEvent::ValueChanged as u8;
		old[1] = 0;
		old[2..].copy_from_slice(&42_u64.to_le_bytes());
		let mut workspace = [0xaa; 12];

		let current = normalize_event_data::<ValueChangedEvent>(&old, &mut workspace)
			.unwrap_or_else(|error| panic!("normalize event: {error:?}"));
		assert!(current.was_migrated());
		assert_eq!(current.source_version(), 0);
		assert_eq!(current.as_bytes()[0..2], [4, 1]);
		assert_eq!(&current.as_bytes()[2..10], &42_u64.to_le_bytes());
		assert_eq!(&current.as_bytes()[10..12], &[0, 0]);

		ValueChangedEvent::with_current_event_data(&old, |bytes, source_version| {
			let event = ValueChangedEvent::try_from_bytes(bytes)?;
			assert_eq!(source_version, 0);
			assert_eq!(event.value.get(), 42);
			assert_eq!(event.memo.get(), 0);
			Ok(())
		})
		.unwrap_or_else(|error| panic!("project event: {error:?}"));
	}

	#[test]
	fn generated_event_projection_rejects_malleable_historical_bytes() {
		let mut trailing = [0_u8; 11];
		trailing[0] = MigrationEvent::ValueChanged as u8;
		trailing[1] = 0;
		let future = [MigrationEvent::ValueChanged as u8, 2, 0, 0];

		for rejected in [&trailing[..], &future[..], &[5, 0, 0][..]] {
			let mut workspace = [0xaa; 12];
			assert!(normalize_event_data::<ValueChangedEvent>(rejected, &mut workspace).is_err());
		}
	}

	#[test]
	fn generated_account_migration_walks_every_adjacent_step() {
		let authority = Address::new_from_array([9; 32]);
		let mut old = [0_u8; 42];
		old[0] = MigrationAccount::State as u8;
		old[1] = 0;
		old[2..34].copy_from_slice(authority.as_ref());
		old[34..42].copy_from_slice(&42_u64.to_le_bytes());

		// Step one: v0 -> v1 appends the enabled byte.
		let plan = State::plan_migration(&old)
			.unwrap_or_else(|error| panic!("plan account migration: {error:?}"));
		assert_eq!((plan.from_version(), plan.to_version()), (0, 1));
		assert_eq!(plan.target_size(), 43);
		let mut destination = [0xaa; 43];
		destination[..old.len()].copy_from_slice(&old);
		State::apply_migration(plan.into_payload(), &mut destination);
		State::validate_migration_destination(1, &destination)
			.unwrap_or_else(|error| panic!("validate destination: {error:?}"));
		destination[1] = 1;

		// Step two: v1 -> v2 appends the revision byte, planned from the
		// representation produced by step one.
		let plan = State::plan_migration(&destination)
			.unwrap_or_else(|error| panic!("replan account migration: {error:?}"));
		assert_eq!((plan.from_version(), plan.to_version()), (1, 2));
		assert_eq!(plan.target_size(), 44);
		let mut current = [0xaa; 44];
		current[..destination.len()].copy_from_slice(&destination);
		State::apply_migration(plan.into_payload(), &mut current);
		State::validate_migration_destination(2, &current)
			.unwrap_or_else(|error| panic!("validate current destination: {error:?}"));
		State::write_current_migration_version(&mut current)
			.unwrap_or_else(|error| panic!("write version: {error:?}"));

		assert_eq!(current[0..2], [1, 2]);
		assert_eq!(&current[2..34], authority.as_ref());
		assert_eq!(&current[34..42], &42_u64.to_le_bytes());
		assert_eq!(current[42], 0);
		assert_eq!(current[43], 0);
	}

	#[test]
	fn manual_account_migration_widens_then_converts_to_compact() {
		let old = [MigrationAccount::ManualState as u8, 0, u8::MAX];

		// Step one: v0 -> v1 widens the amount to u16.
		let plan = ManualState::plan_migration(&old)
			.unwrap_or_else(|error| panic!("plan manual account migration: {error:?}"));
		assert_eq!(plan.target_size(), 4);
		let mut destination = [0xaa; 4];
		destination[..old.len()].copy_from_slice(&old);
		ManualState::apply_migration(plan.into_payload(), &mut destination);
		ManualState::validate_migration_destination(1, &destination)
			.unwrap_or_else(|error| panic!("validate manual destination: {error:?}"));
		destination[1] = 1;
		assert_eq!(u16::from_le_bytes([destination[2], destination[3]]), 255);

		// Step two: v1 -> v2 converts the fixed amount into a compact
		// decimal code through the manual fixed-to-compact transition.
		let plan = ManualState::plan_migration(&destination)
			.unwrap_or_else(|error| panic!("replan manual migration: {error:?}"));
		assert_eq!((plan.from_version(), plan.to_version()), (1, 2));
		assert_eq!(plan.target_size(), 6);
		let mut current = [0xaa; 6];
		current[..destination.len()].copy_from_slice(&destination);
		ManualState::apply_migration(plan.into_payload(), &mut current);
		ManualState::validate_migration_destination(2, &current)
			.unwrap_or_else(|error| panic!("validate compact destination: {error:?}"));
		ManualState::write_current_migration_version(&mut current)
			.unwrap_or_else(|error| panic!("write manual version: {error:?}"));

		assert_eq!(current[..2], [MigrationAccount::ManualState as u8, 2]);
		assert_eq!(current[2], 3);
		assert_eq!(&current[3..6], b"255");
	}

	#[test]
	fn compact_account_migration_preserves_active_data_and_discards_spare_bytes() {
		let old = [
			MigrationAccount::CompactState as u8,
			0,
			3,
			b'A',
			b'd',
			b'a',
			0xee,
		];
		let plan = CompactState::plan_migration(&old)
			.unwrap_or_else(|error| panic!("plan compact account migration: {error:?}"));
		assert_eq!(plan.target_size(), 8);
		assert_eq!(plan.working_size(), 8);

		let mut destination = [0xaa; 8];
		destination[..old.len()].copy_from_slice(&old);
		CompactState::apply_migration(plan.into_payload(), &mut destination);
		CompactState::validate_migration_destination(1, &destination)
			.unwrap_or_else(|error| panic!("validate compact destination: {error:?}"));
		CompactState::write_current_migration_version(&mut destination)
			.unwrap_or_else(|error| panic!("write compact version: {error:?}"));
		let state = CompactState::try_from_bytes(&destination)
			.unwrap_or_else(|error| panic!("read compact destination: {error:?}"));

		assert_eq!(state.name(), "Ada");
		assert!(state.tags().is_empty());
		assert!(!destination.contains(&0xee));
	}

	#[test]
	fn compact_account_planner_rejects_forged_historical_lengths() {
		let forged = [MigrationAccount::CompactState as u8, 0, 5, b'A', b'd', b'a'];
		let wrong_discriminator = [0, 0, 0, b'A', b'd', b'a'];

		assert_eq!(
			CompactState::plan_migration(&forged),
			Err(ProgramError::InvalidAccountData),
		);
		assert_eq!(
			CompactState::plan_migration(&wrong_discriminator),
			Err(ProgramError::InvalidAccountData),
		);
		let mut wrong_fixed = [0_u8; 42];
		wrong_fixed[1] = 0;
		assert_eq!(
			State::plan_migration(&wrong_fixed),
			Err(ProgramError::InvalidAccountData),
		);
	}

	#[test]
	fn generic_read_paths_reject_stale_version_envelopes() {
		// A stale account whose physical size already matches the current
		// layout (the shape produced by every same-size migration: reorders,
		// semantic changes, field-type splits). The bytes carry a stale
		// version envelope but otherwise validate as the current layout.
		let mut stale = [0_u8; State::SIZE];
		stale[0] = MigrationAccount::State as u8;
		stale[1] = 1;
		stale[2..34].copy_from_slice(&[7; 32]);
		stale[34..42].copy_from_slice(&42_u64.to_le_bytes());
		stale[42] = 1;
		stale[43] = 0;

		// The generated inherent reader already refuses the stale envelope.
		assert_eq!(
			State::try_from_bytes(&stale).map(|_| ()),
			Err(PinaProgramError::MigrationRequired.into()),
		);

		// `AccountView::as_account` and `as_account_mut` resolve
		// `T::try_from_bytes` through the `PinaAccount` trait bound, so this
		// generic shim reproduces their method resolution exactly.
		fn read_through_trait<T: PinaAccount>(data: &[u8]) -> Result<(), ProgramError> {
			T::validate_account_data(data)?;
			T::try_from_bytes(data).map(|_| ())
		}

		assert_eq!(
			read_through_trait::<State>(&stale),
			Err(PinaProgramError::MigrationRequired.into()),
		);
	}
}
