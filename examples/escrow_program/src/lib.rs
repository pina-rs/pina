#![no_std]

use pina::*;

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use pina::*;

	nostd_entrypoint!(process_instruction);

	#[inline(always)]
	pub fn process_instruction(
		_program_id: &Pubkey,
		accounts: &[AccountInfo],
		instruction_data: &[u8],
	) -> ProgramResult {
		// Validate program ID
		// if _program_id != &crate::ID {
		Err(ProgramError::IncorrectProgramId)
		// }

		// let (discriminator, data) = instruction_data
		// 	.split_first()
		// 	.ok_or(ProgramError::InvalidInstructionData)?;

		// match Instruction::try_from(discriminator)? {
		// 	Instruction::MakeOffer => {
		// 		log!("Instruction: MakeOffer");
		// 		MakeOffer::try_from((accounts, data))?.handler()
		// 	}
		// 	Instruction::TakeOffer => {
		// 		log!("Instruction: TakeOffer");
		// 		TakeOffer::try_from((accounts, data))?.handler()
		// 	}
		// }
	}
}

#[error]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowError {
	OfferKeyMismatch = 0,
	TokenAccountMismatch = 1,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq, Eq)]
pub struct Offer {
	pub maker: Pubkey,
	pub mint_a: Pubkey,
	pub mint_b: Pubkey,
	pub amount: [u8; 8],
	pub receiver: [u8; 8],
	pub seed: [u8; 8],
	pub bump: u8,
}

#[discriminator(crate = ::pina, primitive = u8, final)]
pub enum MyAccount {
	ConfigState = 0,
	GameState = 1,
	SectionState = 2,
}

#[repr(C)]
#[derive(
	Debug,
	::core::clone::Clone,
	::core::marker::Copy,
	::core::cmp::PartialEq,
	::core::cmp::Eq,
	::pina::Pod,
	::pina::Zeroable,
	::pina::TypedBuilder,
)]
#[builder(builder_method(vis = "", name = __builder))]
pub struct ConfigState {
	/// The automatically applied discriminator which is inserted as bytes.
	discriminator: [u8; MyAccount::BYTES],
	/// The version of the state.
	pub version: u8,
	/// The authority which can update this config.
	pub authority: Pubkey,
	/// Store the bump to save compute units.
	pub bump: u8,
	/// The treasury account bump where fees are sent and where the minted
	/// tokens are transferred.
	pub treasury_bump: u8,
	/// The mint account bump.
	pub mint_bit_bump: u8,
	/// The mint account bump for KIBIBIT.
	pub mint_kibibit_bump: u8,
	/// The mint account bump for MEBIBIT.
	pub mint_mebibit_bump: u8,
	/// The mint account bump for GIBIBIT.
	pub mint_gibibit_bump: u8,
	/// There will be a maximum of 8 games.
	pub game_index: u8,
}

// This type is generated to match the `TypedBuilder` type with the
// discriminator already set.
type ConfigStateBuilderType = ConfigStateBuilder<(
	([u8; MyAccount::BYTES],), /* `discriminator`: automatically applied in the builder method
	                            * below. */
	(), // `version`
	(), // `authority`
	(), // `bump`
	(), // `treasury_bump`
	(), // `mint_bit_bump`
	(), // `mint_kibibit_bump`
	(), // `mint_mebibit_bump`
	(), // `mint_gibibit_bump`
	(), // `game_index`
)>;

impl ConfigState {
	pub fn to_bytes(&self) -> &[u8] {
		::pina::bytemuck::bytes_of(self)
	}

	pub fn builder() -> ConfigStateBuilderType {
		let mut bytes = [0u8; MyAccount::BYTES];
		<Self as ::pina::HasDiscriminator>::VALUE.write_discriminator(&mut bytes);

		Self::__builder().discriminator(bytes)
	}
}

impl ::pina::HasDiscriminator for ConfigState {
	type Type = MyAccount;

	const VALUE: Self::Type = MyAccount::ConfigState;
}

impl ::pina::AccountValidation for ConfigState {
	#[track_caller]
	fn assert<F>(&self, condition: F) -> Result<&Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if condition(self) {
			return Ok(self);
		}

		::pina::log!("Account is invalid");
		::pina::log_caller();

		Err(::pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match ::pina::assert(
			condition(self),
			::pina::ProgramError::InvalidAccountData,
			msg,
		) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}

	#[track_caller]
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		if !condition(self) {
			return Ok(self);
		}

		::pina::log!("Account is invalid");
		::pina::log_caller();

		Err(::pina::ProgramError::InvalidAccountData)
	}

	#[track_caller]
	fn assert_mut_msg<F>(
		&mut self,
		condition: F,
		msg: &str,
	) -> Result<&mut Self, ::pina::ProgramError>
	where
		F: Fn(&Self) -> bool,
	{
		match ::pina::assert(
			condition(self),
			::pina::ProgramError::InvalidAccountData,
			msg,
		) {
			Err(err) => Err(err),
			Ok(()) => Ok(self),
		}
	}
}
