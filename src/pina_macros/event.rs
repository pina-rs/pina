//! `#[event]` contract tests through the public macro.
//!
//! The macro generates a zeropod-backed event payload with a discriminator
//! field, `SIZE`, `initialize`/`try_from_bytes`, and a `HasDiscriminator`
//! impl linking back to the discriminator enum.

/// Local alias to disambiguate from any pina Vec type.
use std::vec;

use pina::*;

#[discriminator]
#[derive(Debug)]
pub enum EventDisc {
	TransferEvent = 1,
	InitializeEvent = 2,
	EmptyEvent = 3,
	AuditEvent = 4,
}

#[event(crate = pina, discriminator = EventDisc)]
pub struct TransferEvent {
	pub from: [u8; 32],
	pub to: [u8; 32],
	pub amount: PodU64,
}

#[event(crate = pina, discriminator = EventDisc, variant = InitializeEvent)]
pub struct InitEvent {
	pub choice: u8,
}

#[event(crate = pina, discriminator = EventDisc)]
pub struct EmptyEvent {}

#[event(crate = pina, discriminator = EventDisc)]
#[derive(Debug)]
pub struct AuditEvent {
	pub action: u8,
	pub timestamp: PodU64,
}

/// A basic event round-trips through initialize and try_from_bytes.
#[test]
fn basic_roundtrip() {
	let mut bytes: std::vec::Vec<u8> = std::vec![0; TransferEvent::SIZE];
	{
		let view = TransferEvent::initialize(&mut bytes).unwrap();
		view.amount.set(500);
	}
	let parsed = TransferEvent::try_from_bytes(&bytes).unwrap();
	assert_eq!(parsed.amount.get(), 500);
}

/// A custom variant name resolves in the `HasDiscriminator` impl.
#[test]
fn with_variant() {
	let disc = &<InitEvent as HasDiscriminator>::VALUE;
	assert_eq!(*disc, EventDisc::InitializeEvent);
}

/// A path variant resolves identically.
#[test]
fn with_path_variant() {
	let mut bytes: std::vec::Vec<u8> = std::vec![0; InitEvent::SIZE];
	{
		let view = InitEvent::initialize(&mut bytes).unwrap();
		view.choice = 7;
	}
	let parsed = InitEvent::try_from_bytes(&bytes).unwrap();
	assert_eq!(parsed.choice, 7);
}

/// An empty event has a one-byte discriminator payload.
#[test]
fn minimal() {
	assert_eq!(EmptyEvent::SIZE, 1);
	assert!(EmptyEvent::try_from_bytes(&[3]).is_ok());
	assert!(EmptyEvent::try_from_bytes(&[]).is_err());
}

/// Existing derives are preserved alongside the generated impls.
#[test]
fn with_existing_derive() {
	let size = AuditEvent::SIZE;
	let mut bytes = vec![0_u8; size];
	AuditEvent::initialize(&mut bytes).unwrap();
	assert!(AuditEvent::try_from_bytes(&bytes).is_ok());
}
