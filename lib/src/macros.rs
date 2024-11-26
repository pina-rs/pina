#[macro_export]
macro_rules! impl_to_bytes {
	($struct_name:ident) => {
		impl $struct_name {
			pub fn to_bytes(&self) -> &[u8] {
				bytemuck::bytes_of(self)
			}
		}
	};
}

#[macro_export]
macro_rules! impl_space {
	($struct_name:ident) => {
		impl $struct_name {
			pub const fn space() -> usize {
				8 + std::mem::size_of::<$struct_name>()
			}
		}
	};
}

#[macro_export]
macro_rules! impl_from_bytes {
	($struct_name:ident) => {
		impl $struct_name {
			pub fn from_bytes(data: &[u8]) -> &Self {
				bytemuck::from_bytes::<Self>(data)
			}
		}
	};
}

#[macro_export]
macro_rules! impl_instruction_from_bytes {
	($struct_name:ident) => {
		impl $struct_name {
			pub fn try_from_bytes(data: &[u8]) -> Result<&Self, $crate::ProgramError> {
				bytemuck::try_from_bytes::<Self>(data)
					.or(Err($crate::ProgramError::InvalidInstructionData))
			}
		}
	};
}

#[macro_export]
macro_rules! account {
	($discriminator_name:ident, $struct_name:ident) => {
		$crate::impl_to_bytes!($struct_name);
		$crate::impl_space!($struct_name);

		impl $crate::Discriminator for $struct_name {
			fn discriminator() -> u8 {
				$discriminator_name::$struct_name.into()
			}
		}

		impl $crate::AccountValidation for $struct_name {
			fn assert<F>(&self, condition: F) -> Result<&Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				if !condition(self) {
					return Err($crate::ProgramError::InvalidAccountData);
				}
				Ok(self)
			}

			fn assert_err<F, E>(&self, condition: F, err: E) -> Result<&Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
				E: Into<$crate::ProgramError> + std::error::Error,
			{
				if !condition(self) {
					#[cfg(feature = "logs")]
					$crate::msg!("Account validation error: {}", err);

					return Err(err.into());
				}

				Ok(self)
			}

			fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				match $crate::assert(
					condition(self),
					$crate::ProgramError::InvalidAccountData,
					msg,
				) {
					Err(err) => Err(err.into()),
					Ok(()) => Ok(self),
				}
			}

			fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				if !condition(self) {
					return Err($crate::ProgramError::InvalidAccountData);
				}

				Ok(self)
			}

			fn assert_mut_err<F, E>(
				&mut self,
				condition: F,
				err: E,
			) -> Result<&mut Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
				E: Into<$crate::ProgramError> + std::error::Error,
			{
				if !condition(self) {
					#[cfg(feature = "logs")]
					$crate::msg!("Account validation error: {}", err);

					return Err(err.into());
				}
				Ok(self)
			}

			fn assert_mut_msg<F>(
				&mut self,
				condition: F,
				msg: &str,
			) -> Result<&mut Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				match $crate::assert(
					condition(self),
					$crate::ProgramError::InvalidAccountData,
					msg,
				) {
					Err(err) => Err(err.into()),
					Ok(()) => Ok(self),
				}
			}
		}
	};
}

#[macro_export]
macro_rules! error {
	($struct_name:ident) => {
		impl From<$struct_name> for $crate::ProgramError {
			fn from(e: $struct_name) -> Self {
				$crate::ProgramError::Custom(e as u32)
			}
		}
	};
}

#[macro_export]
macro_rules! event {
	($struct_name:ident) => {
		$crate::impl_to_bytes!($struct_name);
		$crate::impl_from_bytes!($struct_name);

		impl $crate::Loggable for $struct_name {
			fn log(&self) {
				solana_program::log::sol_log_data(&[self.to_bytes()]);
			}

			fn log_return(&self) {
				solana_program::program::set_return_data(self.to_bytes());
			}
		}
	};
}

#[macro_export]
macro_rules! instruction {
	($discriminator_name:ident, $struct_name:ident) => {
		$crate::impl_instruction_from_bytes!($struct_name);

		impl $crate::Discriminator for $struct_name {
			fn discriminator() -> u8 {
				$discriminator_name::$struct_name as u8
			}
		}

		impl $struct_name {
			pub fn to_bytes(&self) -> Vec<u8> {
				[
					[$discriminator_name::$struct_name as u8].to_vec(),
					bytemuck::bytes_of(self).to_vec(),
				]
				.concat()
			}
		}
	};
}
