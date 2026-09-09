use pina::*;

#[discriminator]
pub enum Kind {
	State = 1,
}

#[account(discriminator = Kind, compact)]
pub struct State {
	pub values: Vec<u64, 4>,
}

struct UncheckedPatch;

impl PinaPodPatch<State> for UncheckedPatch {
	fn updated_len(&self, data: &[u8]) -> Result<usize, PinaPodError> {
		Ok(data.len())
	}

	fn update(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
		data.fill(u8::MAX);
		Ok(data.len())
	}

	fn initialize(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
		data.fill(u8::MAX);
		Ok(data.len())
	}
}

fn require_builder_patch<P: PinaCompactPatch<State>>(_patch: P) {}

fn main() {
	require_builder_patch(UncheckedPatch);
}
