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

#[instruction(discriminator = EventsInstruction, variant = Initialize)]
pub struct InitializeInstruction {}

#[instruction(discriminator = EventsInstruction, variant = TestEvent)]
pub struct TestEventInstruction {}

#[instruction(discriminator = EventsInstruction, variant = TestEventCpi)]
pub struct TestEventCpiInstruction {}

#[discriminator]
pub enum EventDiscriminator {
	MyEvent = 1,
	MyOtherEvent = 2,
}

#[event(discriminator = EventDiscriminator)]
#[derive(Debug)]
pub struct MyEvent {
	pub data: PodU64,
	pub label: [u8; 8],
}

#[event(discriminator = EventDiscriminator)]
#[derive(Debug)]
pub struct MyOtherEvent {
	pub data: PodU64,
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
	MyEvent(MyEvent),
	MyOtherEvent(MyOtherEvent),
}

#[allow(dead_code)]
fn build_event(instruction: EventsInstruction) -> EmittedEvent {
	match instruction {
		EventsInstruction::Initialize => {
			EmittedEvent::MyEvent(
				MyEvent::builder()
					.data(PodU64::from_primitive(5))
					.label(LABEL_HELLO)
					.build(),
			)
		}
		EventsInstruction::TestEvent => {
			EmittedEvent::MyOtherEvent(
				MyOtherEvent::builder()
					.data(PodU64::from_primitive(6))
					.label(LABEL_BYE)
					.build(),
			)
		}
		EventsInstruction::TestEventCpi => {
			EmittedEvent::MyOtherEvent(
				MyOtherEvent::builder()
					.data(PodU64::from_primitive(7))
					.label(LABEL_CPI)
					.build(),
			)
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
		_accounts: &[AccountView],
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
		let event = match build_event(EventsInstruction::Initialize) {
			EmittedEvent::MyEvent(event) => event,
			EmittedEvent::MyOtherEvent(_) => panic!("expected my event"),
		};

		assert_eq!(u64::from(event.data), 5);
		assert_eq!(event.label, LABEL_HELLO);
	}

	#[test]
	fn test_event_matches_expected_values() {
		let event = match build_event(EventsInstruction::TestEvent) {
			EmittedEvent::MyOtherEvent(event) => event,
			EmittedEvent::MyEvent(_) => panic!("expected other event"),
		};

		assert_eq!(u64::from(event.data), 6);
		assert_eq!(event.label, LABEL_BYE);
	}

	#[test]
	fn test_event_cpi_matches_expected_values() {
		let event = match build_event(EventsInstruction::TestEventCpi) {
			EmittedEvent::MyOtherEvent(event) => event,
			EmittedEvent::MyEvent(_) => panic!("expected other event"),
		};

		assert_eq!(u64::from(event.data), 7);
		assert_eq!(event.label, LABEL_CPI);
	}

	#[test]
	fn my_event_roundtrip_serialization() {
		let event = MyEvent::builder()
			.data(PodU64::from_primitive(5))
			.label(LABEL_HELLO)
			.build();
		let bytes = event.to_bytes();
		let decoded = MyEvent::try_from_bytes(bytes).unwrap_or_else(|e| panic!("decode: {e:?}"));

		assert_eq!(decoded.label, LABEL_HELLO);
		assert_eq!(u64::from(decoded.data), 5);
	}

	#[test]
	fn parse_instruction_rejects_program_id_mismatch() {
		let wrong_program_id: Address = [8u8; 32].into();
		let data = [EventsInstruction::Initialize as u8];
		let result = parse_instruction::<EventsInstruction>(&wrong_program_id, &ID, &data);
		assert!(matches!(result, Err(ProgramError::IncorrectProgramId)));
	}
}
