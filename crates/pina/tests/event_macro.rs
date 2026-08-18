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
	let mut bytes = [0u8; Initialize::SIZE];
	let event = Initialize::initialize(&mut bytes).unwrap();
	event.choice = 10;
	assert_eq!(event.choice, 10);

	let disc = &<Initialize as HasDiscriminator>::VALUE;
	assert_eq!(*disc, Event::Initialize);
}

#[test]
fn test_event_bytes() {
	let mut bytes = [0u8; Initialize::SIZE];
	{
		let event = Initialize::initialize(&mut bytes).unwrap();
		event.choice = 10;
	}
	let from_bytes = Initialize::try_from_bytes(&bytes).unwrap();
	assert_eq!(from_bytes.discriminator, [Event::Initialize as u8]);
	assert_eq!(from_bytes.choice, 10);
}
