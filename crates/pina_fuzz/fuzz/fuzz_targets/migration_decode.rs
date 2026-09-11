//! Fuzz generated migration decoders with arbitrary historical bytes.

#![allow(clippy::all)]
#![no_main]

use libfuzzer_sys::fuzz_target;
use migrations_program::CompactState;
use migrations_program::ManualState;
use migrations_program::State;
use migrations_program::UpdateInstruction;
use migrations_program::ValueChangedEvent;
use pina::HasDiscriminator;
use pina::HasMigrationVersion;
use pina::MigratableAccount;
use pina::MigratableEvent;
use pina::MigratableInstruction;
use pina::MigrationVersion;
use pina::normalize_event_data;
use pina::normalize_instruction_data;

fn fuzz_instruction(data: &[u8]) {
	let mut workspace = vec![0xa5; <UpdateInstruction as MigratableInstruction>::WORKING_SIZE];
	if let Ok(current) = normalize_instruction_data::<UpdateInstruction>(data, &mut workspace) {
		assert_eq!(
			current.as_bytes().len(),
			<UpdateInstruction as MigratableInstruction>::CURRENT_SIZE,
		);
		assert!(UpdateInstruction::matches_discriminator(current.as_bytes()));
		assert_eq!(
			UpdateInstruction::read_migration_version(current.as_bytes()),
			Ok(UpdateInstruction::CURRENT_VERSION),
		);
		assert!(UpdateInstruction::try_from_bytes(current.as_bytes()).is_ok());
	}
}

fn fuzz_event(data: &[u8]) {
	let mut workspace = vec![0xa5; <ValueChangedEvent as MigratableEvent>::WORKING_SIZE];
	if let Ok(current) = normalize_event_data::<ValueChangedEvent>(data, &mut workspace) {
		assert_eq!(
			current.as_bytes().len(),
			<ValueChangedEvent as MigratableEvent>::CURRENT_SIZE,
		);
		assert!(ValueChangedEvent::matches_discriminator(current.as_bytes()));
		assert_eq!(
			ValueChangedEvent::read_migration_version(current.as_bytes()),
			Ok(ValueChangedEvent::CURRENT_VERSION),
		);
		assert!(ValueChangedEvent::try_from_bytes(current.as_bytes()).is_ok());
	}
}

fn fuzz_account<T>(data: &[u8])
where
	T: MigratableAccount<Plan = u32>,
{
	let Ok(plan) = T::plan_migration(data) else {
		return;
	};
	let version = plan.to_version();
	let target_size = plan.target_size();
	let mut destination = vec![0xa5; plan.working_size()];
	assert!(destination.len() >= data.len());
	destination[..data.len()].copy_from_slice(data);
	T::apply_migration(plan.into_payload(), &mut destination);
	destination.truncate(target_size);
	assert!(T::validate_migration_destination(version, &destination).is_ok());
	let version = T::Version::try_from_u32(version)
		.unwrap_or_else(|error| panic!("generated destination version must fit: {error:?}"));
	T::write_migration_version(version, &mut destination)
		.unwrap_or_else(|error| panic!("write generated destination version: {error:?}"));
	assert!(T::validate_migration_destination(version.into_u32(), &destination).is_ok());
}

fn copy_fuzzed_payload(destination: &mut [u8], source: &[u8]) {
	for (index, byte) in destination.iter_mut().enumerate() {
		*byte = source.get(index).copied().unwrap_or_default();
	}
}

fn fuzz_valid_historical_envelopes(data: &[u8]) {
	let mut instruction = [0_u8; 10];
	copy_fuzzed_payload(&mut instruction[2..], data);
	fuzz_instruction(&instruction);

	let mut event = [0_u8; 10];
	event[0] = 4;
	copy_fuzzed_payload(&mut event[2..], data);
	fuzz_event(&event);

	let mut fixed_account = [0_u8; 42];
	fixed_account[0] = 1;
	copy_fuzzed_payload(&mut fixed_account[2..], data);
	fuzz_account::<State>(&fixed_account);

	let manual_account = [2, 0, data.first().copied().unwrap_or_default()];
	fuzz_account::<ManualState>(&manual_account);

	let active_length = data.first().copied().unwrap_or_default() % 5;
	let mut compact_account = [0_u8; 7];
	compact_account[0] = 3;
	compact_account[2] = active_length;
	for (index, byte) in compact_account[3..].iter_mut().enumerate() {
		*byte = data.get(index + 1).copied().unwrap_or_default() & 0x7f;
	}
	fuzz_account::<CompactState>(&compact_account);
}

fuzz_target!(|data: &[u8]| {
	fuzz_instruction(data);
	fuzz_event(data);
	fuzz_account::<State>(data);
	fuzz_account::<ManualState>(data);
	fuzz_account::<CompactState>(data);
	fuzz_valid_historical_envelopes(data);
});
