use steel::*;

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
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
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

// pub fn process_instruction(
//     program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     data: &[u8],
// ) -> ProgramResult {
//     let (ix, data) = parse_instruction::<MyInstruction>(&example_api::ID, program_id, data)?;

//     match ix {
//         MyInstruction::Add => process_add(accounts, data)?,
//         MyInstruction::Initialize => process_initialize(accounts, data)?,
//     }

//     Ok(())
// }

// entrypoint!(process_instruction);

// pub fn process_add(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
//     let [signer_info, counter_info] = accounts else {
//         return Err(ProgramError::NotEnoughAccountKeys);
//     };

//     signer_info.is_signer()?;

//     let counter = counter_info
//         .as_account_mut::<Counter>(&example_api::ID)?
//         .assert_mut(|c| c.value <= 42)?;

//     counter.value += 1;

//     Ok(())
// }

// pub fn process_transfer(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
//     let [signer_info, counter_info, mint_info, sender_info, receiver_info, token_program] = accounts else {
//         return Err(ProgramError::NotEnoughAccountKeys);
//     };

//     signer_info.is_signer()?;

//     counter_info
//         .as_account::<Counter>(&example_api::ID)?
//         .assert(|c| c.value >= 42)?;

//     mint_info.as_mint()?;

//     sender_info
//         .is_writable()?
//         .as_token_account()?
//         .assert(|t| t.owner == *signer_info.key)?
//         .assert(|t| t.mint == *mint_info.key)?;

//     receiver_info
//         .is_writable()?
//         .as_token_account()?
//         .assert(|t| t.mint == *mint_info.key)?;

//     token_program.is_program(&spl_token_2022::ID)?;

//     transfer(
//         signer_info,
//         sender_info,
//         receiver_info,
//         token_program,
//         counter.value,
//     )?;

//     Ok(())
// }
