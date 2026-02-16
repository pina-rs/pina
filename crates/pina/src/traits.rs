use bytemuck::Pod;
use pinocchio::ProgramResult;

use crate::AccountView;
use crate::Address;
use crate::ProgramError;

/// Zero-copy deserialization for on-chain account data.
///
/// Validates the discriminator and reinterprets the byte slice as `&Self`
/// (or `&mut Self`) without copying. The blanket implementation covers all
/// types that implement both [`HasDiscriminator`] and [`Pod`].
///
/// **Note:** This trait is used by `#[account]` types and by
/// [`AsAccount::as_account`]. The `#[instruction]` macro generates its own
/// `try_from_bytes` that does **not** check the discriminator — instruction
/// data is already validated by [`parse_instruction`](crate::parse_instruction)
/// at the entrypoint level, so a second discriminator check would be redundant.
pub trait AccountDeserialize {
	/// Validate the discriminator and reinterpret `data` as `&Self`.
	fn try_from_bytes(data: &[u8]) -> Result<&Self, ProgramError>;
	/// Validate the discriminator and reinterpret `data` as `&mut Self`.
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

/// Validation trait for deserialized account data (e.g. `EscrowState`).
///
/// Allows chaining arbitrary boolean assertions on the typed account, returning
/// `Ok(&Self)` when the condition holds and `Err(InvalidAccountData)`
/// otherwise.
pub trait AccountValidation {
	/// Assert an immutable condition on the account data.
	fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	/// Assert an immutable condition with a custom log message on failure.
	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	/// Assert a condition on a mutable reference to the account data.
	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	/// Assert a condition on a mutable reference with a custom log message.
	fn assert_mut_msg<F>(&mut self, condition: F, msg: &str) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool;
}

/// Validation trait for raw `AccountView` references.
///
/// Methods return `Result<&Self, ProgramError>` to enable chaining:
/// ```ignore
/// account.assert_signer()?.assert_writable()?.assert_owner(&program_id)?;
/// ```
pub trait AccountInfoValidation {
	/// Assert that the account is a signer.
	fn assert_signer(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is writable.
	fn assert_writable(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is executable.
	fn assert_executable(&self) -> Result<&Self, ProgramError>;
	/// Assert that the data held by the account is of the specified length.
	fn assert_data_len(&self, len: usize) -> Result<&Self, ProgramError>;
	/// Assert that the account is empty.
	fn assert_empty(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is not empty.
	fn assert_not_empty(&self) -> Result<&Self, ProgramError>;
	/// Assert that the account is of the type provided.
	fn assert_type<T: HasDiscriminator>(&self, program_id: &Address)
	-> Result<&Self, ProgramError>;
	/// Assert that the account is a program.
	fn assert_program(&self, program_id: &Address) -> Result<&Self, ProgramError>;
	/// Assert that the account is a system variable.
	fn assert_sysvar(&self, sysvar_id: &Address) -> Result<&Self, ProgramError>;
	/// Assert that the account has the address provided.
	fn assert_address(&self, address: &Address) -> Result<&Self, ProgramError>;
	/// Assert that the account has any of the address provided.
	fn assert_addresses(&self, addresses: &[Address]) -> Result<&Self, ProgramError>;
	/// Assert that the account is owned by the address provided.
	fn assert_owner(&self, owner: &Address) -> Result<&Self, ProgramError>;
	/// Assert that the account is owned by one of the owner (program) ids
	/// provided.
	fn assert_owners(&self, owners: &[Address]) -> Result<&Self, ProgramError>;
	/// Assert that the account has the seeds provided and uses the canonical
	/// bump.
	fn assert_seeds(&self, seeds: &[&[u8]], program_id: &Address) -> Result<&Self, ProgramError>;
	/// Assert that the account has the seeds and bump provided.
	fn assert_seeds_with_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Address,
	) -> Result<&Self, ProgramError>;
	/// Assert that the account uses the canonical bump for the seeds provided.
	/// Returns the bump.
	fn assert_canonical_bump(
		&self,
		seeds: &[&[u8]],
		program_id: &Address,
	) -> Result<u8, ProgramError>;
	/// Assert that the account is an associated token with the provided token
	/// account.
	#[cfg(feature = "token")]
	fn assert_associated_token_address(
		&self,
		wallet: &Address,
		mint: &Address,
		token_program: &Address,
	) -> Result<&Self, ProgramError>;
}

macro_rules! primitive_into_discriminator {
	($type:ty) => {
		impl IntoDiscriminator for $type {
			fn discriminator_from_bytes(bytes: &[u8]) -> Result<Self, $crate::ProgramError> {
				// SECURITY: panics if `bytes.len() < Self::BYTES`. Callers must
				// ensure the slice is at least `BYTES` long.
				let sliced_bytes = &bytes[..Self::BYTES];
				let mut discriminator_bytes = [0u8; Self::BYTES];
				discriminator_bytes.copy_from_slice(sliced_bytes);

				Ok(<$type>::from_le_bytes(discriminator_bytes))
			}

			fn write_discriminator(&self, bytes: &mut [u8]) {
				assert!(bytes.len() >= Self::BYTES);
				bytes[..Self::BYTES].copy_from_slice(&self.to_le_bytes());
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
/// 			_ => {
/// 				::core::result::Result::Err(
/// 					::pina::PinaProgramError::InvalidDiscriminator.into(),
/// 				)
/// 			}
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

/// Low-level discriminator codec.
///
/// Implemented for the primitive types (`u8`, `u16`, `u32`, `u64`) and for
/// user-defined discriminator enums via the [`into_discriminator!`] macro or
/// the `#[discriminator]` attribute macro.
pub trait IntoDiscriminator: Sized {
	/// The number of bytes required to store this discriminator.
	const BYTES: usize = size_of::<Self>();

	/// Read a discriminator from the first `BYTES` of the data slice.
	fn discriminator_from_bytes(bytes: &[u8]) -> Result<Self, ProgramError>;

	/// Write the discriminator to the provided bytes.
	fn write_discriminator(&self, bytes: &mut [u8]);

	/// Check if this discriminator matches the first `BYTES` of the provided
	/// byte array.
	fn matches_discriminator(&self, bytes: &[u8]) -> bool;
}

/// The maximum number of bytes that a discriminator can occupy, chosen to
/// prevent alignment issues. Since the largest alignment size is `u64`
/// (8 bytes), this constant ensures the discriminator never causes alignment
/// errors.
pub const MAX_DISCRIMINATOR_SPACE: usize = 8;

/// Associates a concrete type (account / instruction / event struct) with its
/// discriminator enum variant.
pub trait HasDiscriminator: Sized {
	/// The underlying type of the discriminator.
	type Type: IntoDiscriminator;
	/// The value of the discriminator for this type.
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

/// Deserializes raw `AccountView` data into a typed account reference.
///
/// Performs:
/// 1. Program owner check
/// 2. Discriminator byte check
/// 3. Checked bytemuck conversion of account data to `&T` or `&mut T`.
pub trait AsAccount {
	/// Validate ownership and deserialize the account data into an immutable
	/// reference of type `T`. Returns `InvalidAccountData` if the
	/// discriminator doesn't match or the data is the wrong size.
	fn as_account<T>(&self, program_id: &Address) -> Result<&T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod;

	/// Validate ownership and deserialize the account data into a mutable
	/// reference of type `T`. The Solana runtime guarantees exclusive access
	/// when the mutable borrow succeeds.
	#[allow(clippy::mut_from_ref)]
	fn as_account_mut<T>(&self, program_id: &Address) -> Result<&mut T, ProgramError>
	where
		T: AccountDeserialize + HasDiscriminator + Pod;
}

/// Convenience methods for interpreting `AccountView` as SPL token account
/// types.
///
/// Callers should verify account ownership before trusting the returned
/// reference, since these methods perform layout casts without owner checks.
#[cfg(feature = "token")]
pub trait AsTokenAccount {
	/// Interpret the account data as an SPL Token mint.
	fn as_token_mint(&self) -> Result<&crate::token::state::Mint, ProgramError>;
	/// Interpret the account data as an SPL Token account.
	fn as_token_account(&self) -> Result<&crate::token::state::TokenAccount, ProgramError>;
	/// Interpret the account data as a Token-2022 mint.
	fn as_token_2022_mint(&self) -> Result<&crate::token_2022::state::Mint, ProgramError>;
	/// Interpret the account data as a Token-2022 token account.
	fn as_token_2022_account(
		&self,
	) -> Result<&crate::token_2022::state::TokenAccount, ProgramError>;
	/// Interpret the account data as an associated token account, verifying
	/// the address matches the derived ATA for the given wallet, mint, and
	/// token program.
	fn as_associated_token_account(
		&self,
		owner: &Address,
		mint: &Address,
		token_program: &Address,
	) -> Result<&crate::token::state::TokenAccount, ProgramError>;
}

/// Direct lamport transfer between accounts.
///
/// `send` directly manipulates lamport balances (no CPI). This only works
/// when the sender is owned by the executing program. `collect` uses a system
/// program CPI transfer and works with any signer account.
pub trait LamportTransfer<'a> {
	/// Debit `lamports` from this account and credit them to `to` by directly
	/// mutating both accounts' lamport balances. The sender must be owned by
	/// the executing program.
	fn send(&'a self, lamports: u64, to: &'a AccountView) -> ProgramResult;
	/// Transfer `lamports` from the `from` account to this account via a
	/// system program CPI. The `from` account must be a signer.
	fn collect(&'a self, lamports: u64, from: &'a AccountView) -> ProgramResult;
}

/// Close an account and reclaim its rent lamports.
pub trait CloseAccountWithRecipient<'a> {
	/// Close the account, transfer all remaining lamports to the recipient,
	/// and zero the account data.
	fn close_with_recipient(&'a self, recipient: &'a AccountView) -> ProgramResult;
}

/// Destructures a slice of `AccountView` into a named accounts struct.
///
/// Automatically derived by `#[derive(Accounts)]`.
pub trait TryFromAccountInfos<'a>: Sized {
	fn try_from_account_infos(accounts: &'a [AccountView]) -> Result<Self, ProgramError>;
}

/// Instruction processor.
///
/// Implementors validate accounts and execute the instruction logic.
pub trait ProcessAccountInfos<'a>: TryFromAccountInfos<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult;
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
	#[derive(Copy, Clone, Debug, Zeroable, Pod)]
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

	#[test]
	fn account_deserialize_wrong_discriminator() {
		let mut data = [0u8; 17];
		data[0] = 99; // wrong discriminator — TestType expects 7
		let result = TestType::try_from_bytes(&data);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
	}

	#[test]
	fn account_deserialize_undersized_data() {
		// Only 5 bytes — far too small for TestType (17 bytes).
		let data = [7u8, 0, 0, 0, 0];
		let result = TestType::try_from_bytes(&data);
		assert!(result.is_err());
	}

	#[test]
	fn account_deserialize_oversized_data() {
		// 20 bytes — more than size_of::<TestType>() (17).
		let mut data = [0u8; 20];
		data[0] = 7;
		let result = TestType::try_from_bytes(&data);
		// bytemuck::try_from_bytes rejects slices that aren't exactly the right
		// size.
		assert!(result.is_err());
	}

	#[test]
	fn account_deserialize_mut_roundtrip() {
		let mut data = [0u8; 17];
		data[0] = 7;
		let foo = TestType::try_from_bytes_mut(&mut data).unwrap();
		foo.field0 = PodU64::from_primitive(100);
		assert_eq!(100u64, u64::from(foo.field0));
		// Verify the underlying bytes changed.
		assert_eq!(data[1], 100);
	}

	#[test]
	fn discriminator_from_bytes_u8() {
		let data = [42u8, 0, 0, 0];
		let d = u8::discriminator_from_bytes(&data).unwrap();
		assert_eq!(d, 42);
	}

	#[test]
	fn discriminator_from_bytes_u16() {
		// 0x0102 in little-endian is [2, 1]
		let data = [2u8, 1];
		let d = u16::discriminator_from_bytes(&data).unwrap();
		assert_eq!(d, 0x0102);
	}

	#[test]
	fn discriminator_write_and_match_u32() {
		let val: u32 = 0xDEAD_BEEF;
		let mut bytes = [0u8; 4];
		val.write_discriminator(&mut bytes);
		assert!(val.matches_discriminator(&bytes));

		let other: u32 = 0x0000_0001;
		assert!(!other.matches_discriminator(&bytes));
	}

	#[test]
	fn has_discriminator_matches_and_writes() {
		let mut bytes = [0u8; 1];
		TestType::write_discriminator(&mut bytes);
		assert_eq!(bytes[0], 7);
		assert!(TestType::matches_discriminator(&bytes));

		bytes[0] = 0;
		assert!(!TestType::matches_discriminator(&bytes));
	}
}
