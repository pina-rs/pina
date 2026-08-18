//! Anchor `events` parity example ported to pina.
//!
//! Anchor's event transport (`emit!`, `emit_cpi!`) is framework-specific. This
//! parity port focuses on the event type definitions and deterministic
//! serialization/discriminator behavior in pina.

#![allow(clippy::inline_always)]
#![no_std]

#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;

use pina::*;

declare_id!("2dhGsWUzy5YKUsjZdLHLmkNpUDAXkNa9MYWsPc4Ziqzy");

#[discriminator]
pub enum EventsInstruction {
	Initialize = 0,
	TestEvent = 1,
	TestEventCpi = 2,
}

#[instruction(discriminator = EventsInstruction::Initialize)]
pub struct InitializeInstruction {}

#[instruction(discriminator = EventsInstruction::TestEvent)]
pub struct TestEventInstruction {}

#[instruction(discriminator = EventsInstruction::TestEventCpi)]
pub struct TestEventCpiInstruction {}

#[discriminator]
pub enum EventDiscriminator {
	MyEvent = 1,
	MyOtherEvent = 2,
}

#[event(discriminator = EventDiscriminator)]
#[derive(Debug)]
pub struct MyEvent {
	pub data: u64,
	pub label: [u8; 8],
}

#[event(discriminator = EventDiscriminator)]
#[derive(Debug)]
pub struct MyOtherEvent {
	pub data: u64,
	pub label: [u8; 8],
}

#[allow(dead_code)]
const LABEL_HELLO: [u8; 8] = [b'h', b'e', b'l', b'l', b'o', 0, 0, 0];
#[allow(dead_code)]
const LABEL_BYE: [u8; 8] = [b'b', b'y', b'e', 0, 0, 0, 0, 0];
#[allow(dead_code)]
const LABEL_CPI: [u8; 8] = [b'c', b'p', b'i', 0, 0, 0, 0, 0];

#[allow(dead_code)]
pub enum EmittedEvent {
	MyEvent([u8; MyEvent::SIZE]),
	MyOtherEvent([u8; MyOtherEvent::SIZE]),
}

#[allow(dead_code)]
fn build_event(instruction: EventsInstruction) -> EmittedEvent {
	match instruction {
		EventsInstruction::Initialize => {
			let mut bytes = [0u8; MyEvent::SIZE];
			let event = MyEvent::initialize(&mut bytes)
				.unwrap_or_else(|error| panic!("initialize event: {error:?}"));
			event.data.set(5);
			event.label = LABEL_HELLO;
			EmittedEvent::MyEvent(bytes)
		}

		EventsInstruction::TestEvent => {
			let mut bytes = [0u8; MyOtherEvent::SIZE];
			let event = MyOtherEvent::initialize(&mut bytes)
				.unwrap_or_else(|error| panic!("initialize event: {error:?}"));
			event.data.set(6);
			event.label = LABEL_BYE;
			EmittedEvent::MyOtherEvent(bytes)
		}

		EventsInstruction::TestEventCpi => {
			let mut bytes = [0u8; MyOtherEvent::SIZE];
			let event = MyOtherEvent::initialize(&mut bytes)
				.unwrap_or_else(|error| panic!("initialize event: {error:?}"));
			event.data.set(7);
			event.label = LABEL_CPI;
			EmittedEvent::MyOtherEvent(bytes)
		}
	}
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		program_id: &Address,
		_accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: EventsInstruction = parse_instruction(program_id, &ID, data)?;
		let _ = build_event(instruction);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn initialize_event_matches_expected_values() {
		let bytes = match build_event(EventsInstruction::Initialize) {
			EmittedEvent::MyEvent(bytes) => bytes,
			EmittedEvent::MyOtherEvent(_) => panic!("expected my event"),
		};
		let event = MyEvent::try_from_bytes(&bytes).unwrap();

		assert_eq!(event.data.get(), 5);
		assert_eq!(event.label, LABEL_HELLO);
	}

	#[test]
	fn test_event_matches_expected_values() {
		let bytes = match build_event(EventsInstruction::TestEvent) {
			EmittedEvent::MyOtherEvent(bytes) => bytes,
			EmittedEvent::MyEvent(_) => panic!("expected other event"),
		};
		let event = MyOtherEvent::try_from_bytes(&bytes).unwrap();

		assert_eq!(event.data.get(), 6);
		assert_eq!(event.label, LABEL_BYE);
	}

	#[test]
	fn test_event_cpi_matches_expected_values() {
		let bytes = match build_event(EventsInstruction::TestEventCpi) {
			EmittedEvent::MyOtherEvent(bytes) => bytes,
			EmittedEvent::MyEvent(_) => panic!("expected other event"),
		};
		let event = MyOtherEvent::try_from_bytes(&bytes).unwrap();

		assert_eq!(event.data.get(), 7);
		assert_eq!(event.label, LABEL_CPI);
	}

	#[test]
	fn my_event_roundtrip_storage_view() {
		let mut bytes = [0u8; MyEvent::SIZE];
		let event = MyEvent::initialize(&mut bytes)
			.unwrap_or_else(|error| panic!("initialize event: {error:?}"));
		event.data.set(5);
		event.label = LABEL_HELLO;
		let decoded = MyEvent::try_from_bytes(&bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(decoded.label, LABEL_HELLO);
		assert_eq!(decoded.data.get(), 5);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [8u8; 32].into();
		let data = [EventsInstruction::Initialize as u8];
		let result = parse_instruction::<EventsInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
