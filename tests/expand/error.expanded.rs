use pina::*;
#[repr(u32)]
#[non_exhaustive]
pub enum MyError {
	Invalid = 0,
	Duplicate = 1,
}
#[automatically_derived]
impl ::core::fmt::Debug for MyError {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				MyError::Invalid => "Invalid",
				MyError::Duplicate => "Duplicate",
			},
		)
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for MyError {}
#[automatically_derived]
impl ::core::clone::Clone for MyError {
	#[inline]
	fn clone(&self) -> MyError {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for MyError {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for MyError {}
#[automatically_derived]
impl ::core::cmp::PartialEq for MyError {
	#[inline]
	fn eq(&self, other: &MyError) -> bool {
		let __self_discr = ::core::intrinsics::discriminant_value(self);
		let __arg1_discr = ::core::intrinsics::discriminant_value(other);
		__self_discr == __arg1_discr
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for MyError {
	#[inline]
	#[doc(hidden)]
	#[coverage(off)]
	fn assert_receiver_is_total_eq(&self) {}
}
impl ::core::convert::From<MyError> for pina::ProgramError {
	fn from(e: MyError) -> Self {
		pina::ProgramError::Custom(e as u32)
	}
}
#[repr(u32)]
pub enum FinalError {
	Unauthorized = 0,
}
#[automatically_derived]
impl ::core::fmt::Debug for FinalError {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(f, "Unauthorized")
	}
}
impl ::core::convert::From<FinalError> for pina::ProgramError {
	fn from(e: FinalError) -> Self {
		pina::ProgramError::Custom(e as u32)
	}
}
#[repr(u32)]
#[non_exhaustive]
pub enum DetailedError {
	/// Not enough funds to complete the transaction.
	InsufficientFunds = 0,
	/// The account has already been initialized.
	AlreadyInitialized = 1,
	/// The provided authority does not match.
	InvalidAuthority = 2,
	/// The mint does not match.
	InvalidMint = 3,
	/// Arithmetic overflow occurred.
	Overflow = 4,
}
#[automatically_derived]
impl ::core::fmt::Debug for DetailedError {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				DetailedError::InsufficientFunds => "InsufficientFunds",
				DetailedError::AlreadyInitialized => "AlreadyInitialized",
				DetailedError::InvalidAuthority => "InvalidAuthority",
				DetailedError::InvalidMint => "InvalidMint",
				DetailedError::Overflow => "Overflow",
			},
		)
	}
}
impl ::core::convert::From<DetailedError> for pina::ProgramError {
	fn from(e: DetailedError) -> Self {
		pina::ProgramError::Custom(e as u32)
	}
}
#[repr(u32)]
#[non_exhaustive]
pub enum DefaultCrateError {
	Something = 0,
}
#[automatically_derived]
impl ::core::fmt::Debug for DefaultCrateError {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(f, "Something")
	}
}
impl ::core::convert::From<DefaultCrateError> for ::pina::ProgramError {
	fn from(e: DefaultCrateError) -> Self {
		::pina::ProgramError::Custom(e as u32)
	}
}
