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
