use bytemuck::Pod;

use crate::AccountInfo;
use crate::ProgramError;
use crate::Pubkey;

pub trait AccountDeserialize {
	fn try_from_bytes(data: &[u8]) -> Result<&Self, ProgramError>;
	fn try_from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError>;
}

impl<T> AccountDeserialize for T
where
	T: HasDiscriminator + Pod,
{
	fn try_from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
		if !Self::matches_discriminator(data) {
			return Err(ProgramError::InvalidAccountData);
		}

		bytemuck::try_from_bytes::<Self>(data).or(Err(ProgramError::InvalidAccountData))
	}

	fn try_from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
		if !Self::matches_discriminator(data) {
			return Err(ProgramError::InvalidAccountData);
		}

		bytemuck::try_from_bytes_mut::<Self>(data).or(Err(ProgramError::InvalidAccountData))
	}
}

pub trait AccountValidation {
	fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	fn assert_mut_msg<F>(&mut self, condition: F, msg: &str) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool;
}

pub trait AccountInfoValidation {
	/// Assert that the account is a signer.
	fn assert_signer(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is writable.
	fn assert_writable(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is executable.
	fn assert_executable(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is empty.
	fn assert_empty(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is not empty.
	fn assert_not_empty(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is of the type provided.
	fn assert_type<T: HasDiscriminator>(&self, program_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account is a program.
	fn assert_program(&self, program_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account is a system variable.
	fn assert_sysvar(&self, sysvar_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account has the address provided.
	fn assert_address(&self, address: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account is owned by the address provided.
	fn assert_owner(&self, program_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account is owned by one of the spl token programs.
	#[cfg(feature = "token")]
	fn assert_spl_owner(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account has the seeds provided and uses the canonical
	/// bump.
	fn assert_seeds(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account has the seeds and bump provided.
	fn assert_seeds_with_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Pubkey,
	) -> Result<&Self, ProgramError>;
	/// Assert that the account uses the canonical bump for the seeds provided.
	/// Returns the bump.
	fn assert_canonical_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Pubkey,
	) -> Result<u8, ProgramError>;
	/// Assert that the account is an associated token (key) / p-token account.
	#[cfg(feature = "token")]
	fn assert_associated_token_address(
		&self,
		wallet: &Pubkey,
		mint: &Pubkey,
	) -> Result<&Self, ProgramError>;
	/// Assert that the account is an associated token 2022 account.
	#[cfg(feature = "token")]
	fn assert_associated_token_2022_address(
		&self,
		wallet: &Pubkey,
		mint: &Pubkey,
	) -> Result<&Self, ProgramError>;
}

macro_rules! primitive_into_discriminator {
	($type:ty) => {
		impl IntoDiscriminator for $type {
			fn discriminator_from_bytes(bytes: &[u8]) -> Result<Self, $crate::ProgramError> {
				let sliced_bytes = &bytes[..Self::BYTES];
				let mut discriminator_bytes = [0u8; Self::BYTES];
				discriminator_bytes.copy_from_slice(sliced_bytes);

				Ok(<$type>::from_le_bytes(discriminator_bytes))
			}

			fn write_discriminator(&self, bytes: &mut [u8]) {
				assert!(bytes.len() >= Self::BYTES);
				bytes.copy_from_slice(&self.to_le_bytes());
			}

			fn matches_discriminator(&self, bytes: &[u8]) -> bool {
				assert!(bytes.len() >= Self::BYTES);
				self.to_le_bytes().eq(&bytes[..Self::BYTES])
			}
		}
	};
}

primitive_into_discriminator!(u8);
primitive_into_discriminator!(u16);
primitive_into_discriminator!(u32);
primitive_into_discriminator!(u64);

/// Wrap an enum to automatically make it into a discriminator.
///
/// ```
/// use pina::*;
///
/// #[repr(u64)]
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// pub enum MyEnum {
/// 	First = 0,
/// 	Second = 1,
/// }
///
/// impl TryFrom<u64> for MyEnum {
/// 	type Error = ::pina::ProgramError;
///
/// 	#[inline]
/// 	fn try_from(number: u64) -> ::core::result::Result<Self, ::pina::ProgramError> {
/// 		#![allow(non_upper_case_globals)]
/// 		const ___FIRST: u64 = 0;
/// 		const ___SECOND: u64 = 1;
/// 		#[deny(unreachable_patterns)]
/// 		match number {
/// 			___FIRST => ::core::result::Result::Ok(Self::First),
/// 			___SECOND => ::core::result::Result::Ok(Self::Second),
/// 			#[allow(unreachable_patterns)]
/// 			_ => ::core::result::Result::Err(::pina::PinaError::InvalidDiscriminator.into()),
/// 		}
/// 	}
/// }
///
/// into_discriminator!(MyEnum, u64);
/// ```
#[macro_export]
macro_rules! into_discriminator {
	($enum:path, $type:ty) => {
		// This block is evaluated at compile time.
		// If the sizes don't match, the code will fail to compile.
		const _: () = assert!(
			::core::mem::size_of::<$enum>() == ::core::mem::size_of::<$type>(),
			concat!(
				"The size of the enum `",
				stringify!($enum),
				"` must match the size of its primitive representation
				`",
				stringify!($type),
				"`."
			),
		);

		impl $crate::IntoDiscriminator for $enum {
			fn discriminator_from_bytes(
				bytes: &[u8],
			) -> ::core::result::Result<Self, $crate::ProgramError> {
				<$type as $crate::IntoDiscriminator>::discriminator_from_bytes(bytes)
					.and_then(|primitive| Self::try_from(primitive))
			}

			fn write_discriminator(&self, bytes: &mut [u8]) {
				(*self as $type).write_discriminator(bytes);
			}

			fn matches_discriminator(&self, bytes: &[u8]) -> bool {
				(*self as $type).matches_discriminator(bytes)
			}
		}
	};
}

pub trait IntoDiscriminator: Sized {
	/// The number of bytes required to store this discriminator.
	const BYTES: usize = size_of::<Self>();

	/// From a data slice check the first `SPACE` bytes.
	fn discriminator_from_bytes(bytes: &[u8]) -> Result<Self, ProgramError>;

	/// Write the discriminator to the provided bytes.
	fn write_discriminator(&self, bytes: &mut [u8]);

	/// Check if the provided descriptor matches the first `N` bytes of the
	/// provided bytes array.
	fn matches_discriminator(&self, bytes: &[u8]) -> bool;
}

/// This is the max number bytes that a discriminator can take to prevent
/// alignment issues. Since the larges alignment size us u64. 8bytes is needed
/// to ensure the alignement does not error.
pub const MAX_DISCRIMINATOR_SPACE: usize = 8;

pub trait HasDiscriminator: Sized {
	/// The underling type of the discriminator.
	type Type: IntoDiscriminator;
	/// The vaue of the discriminator.
	const VALUE: Self::Type;

	/// Write the discriminator bytes to the provided mutable bytes array.
	#[inline(always)]
	fn write_discriminator(bytes: &mut [u8]) {
		Self::VALUE.write_discriminator(bytes);
	}

	/// Check whether the discriminator matches the provided bytes array.
	#[inline(always)]
	fn matches_discriminator(bytes: &[u8]) -> bool {
		Self::VALUE.matches_discriminator(bytes)
	}
}

/// Performs:
/// 1. Program owner check
/// 2. Discriminator byte check
/// 3. Checked bytemuck conversion of account data to &T or &mut T.
pub trait AsAccount {
	fn as_account<T>(&self, program_id: &Pubkey) -> Result<&T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod;

	#[allow(clippy::mut_from_ref)]
	fn as_account_mut<T>(&self, program_id: &Pubkey) -> Result<&mut T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod;
}

#[cfg(feature = "token")]
pub trait AsTokenAccount {
	fn as_token_mint(&self) -> Result<&crate::token::state::Mint, ProgramError>;
	fn as_token_account(&self) -> Result<&crate::token::state::TokenAccount, ProgramError>;
	fn as_associated_token_account(
		&self,
		owner: &Pubkey,
		mint: &Pubkey,
	) -> Result<&crate::token::state::TokenAccount, ProgramError>;
	fn as_token_2022_mint(&self) -> Result<&crate::token_2022::state::Mint, ProgramError>;
	fn as_token_2022_account(
		&self,
	) -> Result<&crate::token_2022::state::TokenAccount, ProgramError>;
	fn as_associated_token_2022_account(
		&self,
		owner: &Pubkey,
		mint: &Pubkey,
	) -> Result<&crate::token_2022::state::TokenAccount, ProgramError>;
}

pub trait LamportTransfer<'a> {
	fn send(&'a self, lamports: u64, to: &'a AccountInfo);
	fn collect(&'a self, lamports: u64, from: &'a AccountInfo) -> Result<(), ProgramError>;
}

pub trait Loggable {
	fn log(&self);
	fn log_return(&self);
}

pub trait ProgramOwner {
	fn owner() -> Pubkey;
}

#[cfg(test)]
mod tests {
	#![allow(unsafe_code)]
	extern crate std;

	use bytemuck::Pod;
	use bytemuck::Zeroable;

	use super::*;
	use crate::PodU64;

	#[repr(C)]
	#[derive(Copy, Clone, Zeroable, Pod)]
	struct TestType {
		discriminator: [u8; 1],
		field0: PodU64,
		field1: PodU64,
	}

	impl HasDiscriminator for TestType {
		type Type = u8;

		const VALUE: u8 = 7;
	}

	#[test]
	fn account_deserialize() {
		let mut data = [0u8; 17];
		data[0] = 7;
		data[1] = 42;
		data[9] = 43;
		let foo = TestType::try_from_bytes(&data).unwrap();
		assert_eq!(42u64, foo.field0.into());
		assert_eq!(43u64, foo.field1.into());
	}
}
