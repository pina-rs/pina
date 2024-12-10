use bytemuck::Pod;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub trait AccountDeserialize {
	fn try_from_bytes(data: &[u8]) -> Result<&Self, ProgramError>;
	fn try_from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError>;
}

impl<T> AccountDeserialize for T
where
	T: Discriminator + Pod,
{
	fn try_from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
		if Self::discriminator().ne(&data[0]) {
			return Err(solana_program::program_error::ProgramError::InvalidAccountData);
		}
		bytemuck::try_from_bytes::<Self>(&data[8..]).or(Err(
			solana_program::program_error::ProgramError::InvalidAccountData,
		))
	}

	fn try_from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
		if Self::discriminator().ne(&data[0]) {
			return Err(solana_program::program_error::ProgramError::InvalidAccountData);
		}
		bytemuck::try_from_bytes_mut::<Self>(&mut data[8..]).or(Err(
			solana_program::program_error::ProgramError::InvalidAccountData,
		))
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
	T: Discriminator + Pod,
{
	fn try_header_from_bytes(data: &[u8]) -> Result<(&Self, &[u8]), ProgramError> {
		if Self::discriminator().ne(&data[0]) {
			return Err(solana_program::program_error::ProgramError::InvalidAccountData);
		}
		let (prefix, remainder) = data[8..].split_at(std::mem::size_of::<T>());
		Ok((
			bytemuck::try_from_bytes::<Self>(prefix).or(Err(
				solana_program::program_error::ProgramError::InvalidAccountData,
			))?,
			remainder,
		))
	}

	fn try_header_from_bytes_mut(data: &mut [u8]) -> Result<(&mut Self, &mut [u8]), ProgramError> {
		let (prefix, remainder) = data[8..].split_at_mut(std::mem::size_of::<T>());
		Ok((
			bytemuck::try_from_bytes_mut::<Self>(prefix).or(Err(
				solana_program::program_error::ProgramError::InvalidAccountData,
			))?,
			remainder,
		))
	}
}

pub trait AccountValidation {
	fn assert<F>(&self, condition: F) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	fn assert_err<F, E>(&self, condition: F, err: E) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
		E: Into<ProgramError> + std::error::Error;

	fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool;

	fn assert_mut_err<F, E>(&mut self, condition: F, err: E) -> Result<&mut Self, ProgramError>
	where
		F: Fn(&Self) -> bool,
		E: Into<ProgramError> + std::error::Error;

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
	fn assert_type<T: Discriminator>(&self, program_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account is a program.
	fn assert_program(&self, program_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account is a system variable.
	fn assert_sysvar(&self, sysvar_id: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account has the address provided.
	fn assert_address(&self, address: &Pubkey) -> Result<&Self, ProgramError>;
	/// Assert that the account is owned by the address provided.
	fn assert_owner(&self, program_id: &Pubkey) -> Result<&Self, ProgramError>;
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
	/// Assert that the account is an associated token account.
	#[cfg(feature = "spl")]
	fn assert_associated_token_address(
		&self,
		wallet: &Pubkey,
		mint: &Pubkey,
	) -> Result<&Self, ProgramError>;
}

pub trait Discriminator {
	fn discriminator() -> u8;
}

/// Performs:
/// 1. Program owner check
/// 2. Discriminator byte check
/// 3. Checked bytemuck conversion of account data to &T or &mut T.
pub trait AsAccount {
	fn as_account<T>(&self, program_id: &Pubkey) -> Result<&T, ProgramError>
	where
		T: AccountDeserialize + Discriminator + Pod;

	fn as_account_mut<T>(&self, program_id: &Pubkey) -> Result<&mut T, ProgramError>
	where
		T: AccountDeserialize + Discriminator + Pod;
}

#[cfg(feature = "spl")]
pub trait AsSplAccount {
	fn as_token_mint_state<'info>(
		&self,
	) -> Result<
		spl_token_2022::extension::PodStateWithExtensions<'info, spl_token_2022::pod::PodMint>,
		ProgramError,
	>;
	fn as_token_mint(&self) -> Result<spl_token_2022::pod::PodMint, ProgramError>;
	fn as_token_account_state<'info>(
		&self,
	) -> Result<
		spl_token_2022::extension::PodStateWithExtensions<'info, spl_token_2022::pod::PodAccount>,
		ProgramError,
	>;
	fn as_token_account(&self) -> Result<spl_token_2022::pod::PodAccount, ProgramError>;
}

pub trait LamportTransfer<'a, 'info> {
	fn send(&'a self, lamports: u64, to: &'a AccountInfo<'info>);
	fn collect(&'a self, lamports: u64, from: &'a AccountInfo<'info>) -> Result<(), ProgramError>;
}

pub trait CloseAccount<'a, 'info> {
	fn close(&'a self, to: &'a AccountInfo<'info>) -> Result<(), ProgramError>;
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

	impl Discriminator for GenericallySizedTypeHeader {
		fn discriminator() -> u8 {
			0
		}
	}

	#[test]
	fn account_headers() {
		let mut data = [0u8; 32];
		data[8] = 4;
		data[16] = 5;
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

	impl Discriminator for TestType {
		fn discriminator() -> u8 {
			7
		}
	}

	#[test]
	fn account_deserialize() {
		let mut data = [0u8; 24];
		data[0] = 7;
		data[8] = 42;
		data[16] = 43;
		let foo = TestType::try_from_bytes(&data).unwrap();
		assert_eq!(42, foo.field0);
		assert_eq!(43, foo.field1);
	}
}
