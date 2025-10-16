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
