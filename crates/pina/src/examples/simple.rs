use pina::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum MyAccount {
	Counter = 0,
	Profile = 1,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Counter {
	pub value: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Profile {
	pub data: [u16; 1024],
	pub id: u64,
	pub key: Pubkey,
}

account!(MyAccount, Counter);
account!(MyAccount, Profile);

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum MyInstruction {
	Add = 0,
	Initialize = 1,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Add {
	pub value: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Initialize {}

instruction!(MyInstruction, Add);
instruction!(MyInstruction, Initialize);

#[repr(u32)]
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
pub enum MyError {
	#[error("You did something wrong")]
	Dummy = 0,
}

error!(MyError);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MyEvent {
	pub value: u64,
}

event!(MyEvent);

pub fn process_instruction(
	program_id: &Pubkey,
	accounts: &[AccountInfo],
	data: &[u8],
) -> ProgramResult {
	let (ix, _) = parse_instruction::<MyInstruction>(&system_program::ID, program_id, data)?;

	match ix {
		MyInstruction::Add => {
			let [counter_info] = accounts else {
				return Err(ProgramError::NotEnoughAccountKeys);
			};

			let counter = counter_info.as_account_mut::<Counter>(&program_id)?;
			counter.assert_err(|c| c.value <= 42, MyError::Dummy)?;
			counter.assert_mut_err(|c| c.value <= 42, MyError::Dummy)?;
			counter.value += 1;
		}
		MyInstruction::Initialize => {
			//
		}
	}

	Ok(())
}

entrypoint!(process_instruction);
