use pina::*;

#[discriminator]
pub enum Kind {
	State = 7,
	Action = 8,
	Notice = 9,
}

#[account(discriminator = Kind)]
pub struct State {
	pub owner: Address,
	pub bytes: [u8; 16],
	pub u8_value: u8,
	pub u16_value: u16,
	pub u32_value: u32,
	pub u64_value: u64,
	pub u128_value: u128,
	pub i8_value: i8,
	pub i16_value: i16,
	pub i32_value: i32,
	pub i64_value: i64,
	pub i128_value: i128,
	pub enabled: bool,
	pub pod_u16: PodU16,
	pub pod_u32: PodU32,
	pub pod_u64: PodU64,
	pub pod_u128: PodU128,
	pub pod_i16: PodI16,
	pub pod_i32: PodI32,
	pub pod_i64: PodI64,
	pub pod_i128: PodI128,
	pub pod_bool: PodBool,
	pub maybe_u8: Option<u8>,
	pub maybe_u16: Option<u16>,
	pub maybe_u32: Option<u32>,
	pub maybe_u64: Option<u64>,
	pub maybe_u128: Option<u128>,
	pub maybe_i8: Option<i8>,
	pub maybe_i16: Option<i16>,
	pub maybe_i32: Option<i32>,
	pub maybe_i64: Option<i64>,
	pub maybe_i128: Option<i128>,
	pub maybe_enabled: Option<bool>,
}

#[instruction(discriminator = Kind)]
pub struct Action {
	pub amount: u128,
	pub maybe_delta: Option<i64>,
}

#[event(discriminator = Kind)]
pub struct Notice {
	pub code: u16,
}

fn main() {
	let mut bytes = [0u8; State::SIZE];
	let _ = State::initialize(&mut bytes);
}
