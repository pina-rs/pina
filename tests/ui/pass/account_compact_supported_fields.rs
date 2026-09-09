use pina::*;

#[discriminator]
pub enum Kind {
	State = 1,
}

#[account(discriminator = Kind, compact)]
pub struct State {
	pub revision: Option<u64>,
	pub name: String<32>,
	pub values: Vec<u64, 8>,
	pub note: Option<String<64>>,
	pub maybe_values: Option<Vec<u16, 4>>,
	pub labels: Vec<String<16>, 4>,
}

struct WrappedPatch<'a> {
	patch: StatePatch<'a>,
}

impl PinaCompactPatch<State> for WrappedPatch<'_> {
	fn as_pina_patch(&self) -> &StatePatch<'_> {
		&self.patch
	}
}

fn main() {
	fn accepts_builder_patch<P: PinaCompactPatch<State>>(_patch: P) {}

	accepts_builder_patch(StatePatch::new());
	accepts_builder_patch(WrappedPatch {
		patch: StatePatch::new(),
	});
}
