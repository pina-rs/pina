use pina::*;

#[discriminator(primitive = u8)]
#[derive(Debug)]
pub enum Event {
	Initialize = 0,
	Abandon = 1,
}

#[event(crate = pina, discriminator = Event)]
#[derive(Debug)]
pub struct Initialize {
	pub choice: u8,
}

#[test]
fn test_event_compiles() {
	let event = Initialize::builder().choice(10).build();
	assert_eq!(event.choice, 10);

	let disc = &<Initialize as HasDiscriminator>::VALUE;
	assert_eq!(*disc, Event::Initialize);
}

#[test]
fn test_event_bytes() {
	let event = Initialize::builder().choice(10).build();
	let bytes = event.to_bytes();
	let from_bytes = Initialize::try_from_bytes(bytes).unwrap();
	assert_eq!(event.discriminator, from_bytes.discriminator);
	assert_eq!(event.choice, from_bytes.choice);
}
