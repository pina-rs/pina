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

		bytemuck::try_from_bytes::<Self>(Self::strip_discriminator(data))
			.or(Err(ProgramError::InvalidAccountData))
	}

	fn try_from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
		if !Self::matches_discriminator(data) {
			return Err(ProgramError::InvalidAccountData);
		}

		bytemuck::try_from_bytes_mut::<Self>(Self::strip_discriminator_mut(data))
			.or(Err(ProgramError::InvalidAccountData))
	}
}

/// Account data is sometimes stored via a header and body type,
/// where the former resolves the type of the latter (e.g. merkle trees with a
/// generic size const). This trait parses a header type from the first N bytes
/// of some data, and returns the remaining bytes, which are then available for
/// further processing.
///
/// See module-level tests for example usage.
pub trait AccountHeaderDeserialize {
	fn try_header_from_bytes(data: &[u8]) -> Result<(&Self, &[u8]), ProgramError>;
	fn try_header_from_bytes_mut(data: &mut [u8]) -> Result<(&mut Self, &mut [u8]), ProgramError>;
}

impl<T> AccountHeaderDeserialize for T
where
	T: HasDiscriminator + Pod,
{
	fn try_header_from_bytes(data: &[u8]) -> Result<(&Self, &[u8]), ProgramError> {
		if !Self::matches_discriminator(data) {
			return Err(ProgramError::InvalidAccountData);
		}

		let (prefix, remainder) = Self::strip_discriminator(data).split_at(Self::STRUCT_SPACE);

		Ok((
			bytemuck::try_from_bytes::<Self>(prefix).or(Err(ProgramError::InvalidAccountData))?,
			remainder,
		))
	}

	fn try_header_from_bytes_mut(data: &mut [u8]) -> Result<(&mut Self, &mut [u8]), ProgramError> {
		let (prefix, remainder) =
			Self::strip_discriminator_mut(data).split_at_mut(Self::STRUCT_SPACE);

		Ok((
			bytemuck::try_from_bytes_mut::<Self>(prefix)
				.or(Err(ProgramError::InvalidAccountData))?,
			remainder,
		))
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
			fn write_discriminator(&self, bytes: &mut [u8]) {
				bytes.copy_from_slice(&self.to_le_bytes());
			}

			fn matches_discriminator(&self, bytes: &[u8]) -> bool {
				assert!(bytes.len() >= Self::SPACE);
				self.to_le_bytes().eq(&bytes[..Self::SPACE])
			}
		}
	};
}

primitive_into_discriminator!(u8);
primitive_into_discriminator!(u16);
primitive_into_discriminator!(u32);
primitive_into_discriminator!(u64);

#[macro_export]
macro_rules! enum_into_discriminator {
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
	/// The space of this discriminator.
	const SPACE: usize = size_of::<Self>();

	/// Write the discriminator to the provided bytes.
	fn write_discriminator(&self, bytes: &mut [u8]);

	/// Check if the provided descriptor matches the first `N` bytes of the
	/// provided bytes array.
	fn matches_discriminator(&self, bytes: &[u8]) -> bool;
}

#[repr(u16)]
#[derive(Clone, Copy, Debug)]
enum ZZ {
	A = 0,
	B = 1,
}

enum_into_discriminator!(ZZ, u16);

pub trait HasDiscriminator: Sized {
	/// The underling type of the discriminator.
	type Type: IntoDiscriminator;
	/// The vaue of the discriminator.
	const VALUE: Self::Type;
	/// The number of bytes used by the discriminator.
	const DISCRIMINATOR_SPACE: usize = Self::Type::SPACE;
	/// The space used by the struct which implements this discriminator. This
	/// does **NOT** include the space required for the [`IntoDiscriminator`].
	/// That is stored in [`HasDiscriminator::DISCRIMINATOR_SPACE`]
	const STRUCT_SPACE: usize = size_of::<Self>();
	/// Get the total bytes needed to store this struct including it's
	/// discriminator.
	const SPACE: usize = Self::DISCRIMINATOR_SPACE + Self::STRUCT_SPACE;

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

	/// Strip the discriminator from the returnd slice.
	#[inline(always)]
	fn strip_discriminator<'a>(bytes: &'a [u8]) -> &'a [u8] {
		&bytes[Self::DISCRIMINATOR_SPACE..]
	}

	/// Strip the discriminator from the returned mutable bytes array slice.
	#[inline(always)]
	fn strip_discriminator_mut<'a>(bytes: &'a mut [u8]) -> &'a mut [u8] {
		&mut bytes[Self::DISCRIMINATOR_SPACE..]
	}
}

struct A {}

impl HasDiscriminator for A {
	type Type = u8;

	const VALUE: Self::Type = 0;
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

	use std::format;

	use bytemuck::Pod;
	use bytemuck::Zeroable;

	use super::*;

	#[repr(C)]
	#[derive(Copy, Clone)]
	struct GenericallySizedType<const N: usize> {
		field: [u32; N],
	}

	unsafe impl<const N: usize> Zeroable for GenericallySizedType<N> {}
	unsafe impl<const N: usize> Pod for GenericallySizedType<N> {}

	#[repr(C)]
	#[derive(Copy, Clone, Zeroable, Pod)]
	struct GenericallySizedTypeHeader {
		field_len: u64,
	}

	impl HasDiscriminator for GenericallySizedTypeHeader {
		type Type = u8;

		const VALUE: Self::Type = 0;
	}

	#[test]
	fn account_headers() {
		let mut data = [0u8; 25];
		data[1] = 4;
		data[9] = 5;
		let (_foo_header, foo) = GenericallySizedTypeHeader::try_header_from_bytes(&data)
			.map(|(header, remainder)| {
				let foo = match header.field_len {
					4 => bytemuck::try_from_bytes::<GenericallySizedType<4>>(remainder).unwrap(),
					x => panic!("{}", format!("unknown field len, {x}")),
				};
				(header, foo)
			})
			.unwrap();
		assert_eq!(5, foo.field[0]);
	}

	#[repr(C)]
	#[derive(Copy, Clone, Zeroable, Pod)]
	struct TestType {
		field0: u64,
		field1: u64,
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
		assert_eq!(42, foo.field0);
		assert_eq!(43, foo.field1);
	}
}
