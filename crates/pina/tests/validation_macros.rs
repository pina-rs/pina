#![cfg(feature = "validation")]

use pina::*;

#[discriminator(crate = ::pina, primitive = u8, final)]
enum ValidationKind {
	Instruction = 1,
	Event = 2,
	Account = 3,
}

#[instruction(
	crate = ::pina,
	discriminator = ValidationKind,
	variant = Instruction,
	validate(with = validate_instruction)
)]
struct ValidatedInstruction {
	#[pina(validate(min = 1, max = 10, error = ProgramError::Custom(41)))]
	amount: u16,
	#[pina(validate(min_len = 2, max_len = 5))]
	memo: String<8>,
	#[pina(validate(exact_len = 2))]
	tags: Vec<u8, 4>,
}

fn validate_instruction(value: &ValidatedInstructionZc) -> ProgramResult {
	if value.amount() == value.memo().len() as u16 {
		return Err(ProgramError::Custom(42));
	}

	Ok(())
}

#[event(crate = ::pina, discriminator = ValidationKind, variant = Event)]
struct ValidatedEvent {
	#[pina(validate(max = 3))]
	level: u8,
	#[pina(validate(min = -3, max = 3))]
	score: PodI16,
}

#[account(
	crate = ::pina,
	discriminator = ValidationKind,
	variant = Account
)]
struct ValidatedAccount {
	#[pina(validate(min = 1, max = 5))]
	version: u8,
}

#[test]
fn instruction_validation_runs_explicitly_and_at_decode_boundaries() {
	let mut bytes = [0u8; ValidatedInstruction::SIZE];
	ValidatedInstruction::initialize(&mut bytes, |value| {
		value.amount.set(6);
		value.memo.try_set("pina")?;
		value.tags.try_set([1, 2])?;
		Ok(())
	})
	.unwrap();

	let value = ValidatedInstruction::try_from_bytes(&bytes).unwrap();
	value.validate().unwrap();

	bytes[1..3].copy_from_slice(&11u16.to_le_bytes());
	assert_eq!(
		ValidatedInstruction::try_from_bytes(&bytes).err().unwrap(),
		ProgramError::Custom(41)
	);

	let exact_length_error = ValidatedInstruction::initialize(&mut bytes, |value| {
		value.amount.set(6);
		value.memo.try_set("pina")?;
		value.tags.try_set([1])?;
		Ok(())
	})
	.err()
	.unwrap();
	assert_eq!(exact_length_error, ProgramError::InvalidInstructionData);
}

#[test]
fn instruction_custom_hook_runs_after_field_rules() {
	let mut bytes = [0u8; ValidatedInstruction::SIZE];
	let error = ValidatedInstruction::initialize(&mut bytes, |value| {
		value.amount.set(4);
		value.memo.try_set("pina")?;
		value.tags.try_set([1, 2])?;
		Ok(())
	})
	.err()
	.unwrap();

	assert_eq!(error, ProgramError::Custom(42));
}

#[test]
fn event_and_account_helpers_validate_automatically() {
	let mut event_bytes = [0u8; ValidatedEvent::SIZE];
	let event_error = ValidatedEvent::initialize(&mut event_bytes, |value| {
		value.level = 4;
		value.score.set(-2);
		Ok(())
	})
	.err()
	.unwrap();
	assert_eq!(event_error, ProgramError::InvalidInstructionData);

	let mut account_bytes = [0u8; ValidatedAccount::SIZE];
	let account_error = ValidatedAccount::initialize(&mut account_bytes, |value| {
		value.version = 0;
		Ok(())
	})
	.err()
	.unwrap();
	assert_eq!(account_error, ProgramError::InvalidAccountData);
}
