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
				oo
				$crate::DISCRIMINATOR_SIZE + size_of::<$struct_name>()
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
			#[track_caller]
			fn assert<F>(&self, condition: F) -> Result<&Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				if condition(self) {
					return Ok(self);
				}

				let caller = core::panic::Location::caller();
				$crate::msg!("Account is invalid: {}", caller);

				Err($crate::ProgramError::InvalidAccountData)
			}

			#[track_caller]
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

			#[track_caller]
			fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, $crate::ProgramError>
			where
				F: Fn(&Self) -> bool,
			{
				if !condition(self) {
					return Ok(self);
				}

				$crate::log!("Account is invalid: {}");
				$crate::log_caller();

				Err($crate::ProgramError::InvalidAccountData)
			}

			#[track_caller]
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
macro_rules! event {
	($struct_name:ident) => {
		$crate::impl_to_bytes!($struct_name);
		$crate::impl_from_bytes!($struct_name);

		impl $crate::Loggable for $struct_name {
			fn log(&self) {
				$crate::pinocchio::log::sol_log_data(&[self.to_bytes()]);
			}

			fn log_return(&self) {
				$crate::pinocchio::cpi::set_return_data(self.to_bytes());
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
