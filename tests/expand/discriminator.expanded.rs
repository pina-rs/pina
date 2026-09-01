use pina::*;
#[repr(u8)]
#[non_exhaustive]
pub enum MyDiscriminator {
	First = 0,
	Second = 1,
	Third = 2,
}
#[automatically_derived]
impl ::core::fmt::Debug for MyDiscriminator {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				MyDiscriminator::First => "First",
				MyDiscriminator::Second => "Second",
				MyDiscriminator::Third => "Third",
			},
		)
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for MyDiscriminator {}
#[automatically_derived]
impl ::core::clone::Clone for MyDiscriminator {
	#[inline]
	fn clone(&self) -> MyDiscriminator {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for MyDiscriminator {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for MyDiscriminator {}
#[automatically_derived]
impl ::core::cmp::PartialEq for MyDiscriminator {
	#[inline]
	fn eq(&self, other: &MyDiscriminator) -> bool {
		let __self_discr = ::core::intrinsics::discriminant_value(self);
		let __arg1_discr = ::core::intrinsics::discriminant_value(other);
		__self_discr == __arg1_discr
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for MyDiscriminator {
	#[inline]
	#[doc(hidden)]
	#[coverage(off)]
	fn assert_receiver_is_total_eq(&self) {}
}
const _: () = {
	if !(::core::mem::size_of::<u8>() <= ::pina::MAX_DISCRIMINATOR_SPACE) {
		{
			::core::panicking::panic_fmt(format_args!(
				"A discriminator with primitive `u8` (1 bytes) exceeds `MAX_DISCRIMINATOR_SPACE` \
				 and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, \
				 `u16`, `u32`, `u64`.",
			));
		}
	}
};
impl ::core::convert::From<MyDiscriminator> for u8 {
	#[inline]
	fn from(enum_value: MyDiscriminator) -> Self {
		enum_value as Self
	}
}
impl ::core::convert::TryFrom<u8> for MyDiscriminator {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __FIRST: u8 = 0;
		const __SECOND: u8 = 1;
		const __THIRD: u8 = 2;
		#[deny(unreachable_patterns)]
		match number {
			__FIRST => ::core::result::Result::Ok(Self::First),
			__SECOND => ::core::result::Result::Ok(Self::Second),
			__THIRD => ::core::result::Result::Ok(Self::Third),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}
const _: () = if !(::core::mem::size_of::<MyDiscriminator>() == ::core::mem::size_of::<u8>()) {
	{
		::core::panicking::panic_fmt(format_args!(
			"The size of the enum `MyDiscriminator` must match the size of its primitive \
			 representation\n\t\t\t\t`u8`.",
		));
	}
};
impl ::pina::IntoDiscriminator for MyDiscriminator {
	fn discriminator_from_bytes(
		bytes: &[u8],
	) -> ::core::result::Result<Self, ::pina::ProgramError> {
		<u8 as ::pina::IntoDiscriminator>::discriminator_from_bytes(bytes)
			.and_then(|primitive| Self::try_from(primitive))
	}

	fn write_discriminator(&self, bytes: &mut [u8]) {
		(*self as u8).write_discriminator(bytes);
	}

	fn matches_discriminator(&self, bytes: &[u8]) -> bool {
		(*self as u8).matches_discriminator(bytes)
	}
}
#[repr(u16)]
#[non_exhaustive]
pub enum U16Discriminator {
	First = 0,
	Second = 1,
}
#[automatically_derived]
impl ::core::fmt::Debug for U16Discriminator {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				U16Discriminator::First => "First",
				U16Discriminator::Second => "Second",
			},
		)
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for U16Discriminator {}
#[automatically_derived]
impl ::core::clone::Clone for U16Discriminator {
	#[inline]
	fn clone(&self) -> U16Discriminator {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for U16Discriminator {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for U16Discriminator {}
#[automatically_derived]
impl ::core::cmp::PartialEq for U16Discriminator {
	#[inline]
	fn eq(&self, other: &U16Discriminator) -> bool {
		let __self_discr = ::core::intrinsics::discriminant_value(self);
		let __arg1_discr = ::core::intrinsics::discriminant_value(other);
		__self_discr == __arg1_discr
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for U16Discriminator {
	#[inline]
	#[doc(hidden)]
	#[coverage(off)]
	fn assert_receiver_is_total_eq(&self) {}
}
const _: () = {
	if !(::core::mem::size_of::<u16>() <= ::pina::MAX_DISCRIMINATOR_SPACE) {
		{
			::core::panicking::panic_fmt(format_args!(
				"A discriminator with primitive `u16` (2 bytes) exceeds `MAX_DISCRIMINATOR_SPACE` \
				 and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, \
				 `u16`, `u32`, `u64`.",
			));
		}
	}
};
impl ::core::convert::From<U16Discriminator> for u16 {
	#[inline]
	fn from(enum_value: U16Discriminator) -> Self {
		enum_value as Self
	}
}
impl ::core::convert::TryFrom<u16> for U16Discriminator {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u16) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __FIRST: u16 = 0;
		const __SECOND: u16 = 1;
		#[deny(unreachable_patterns)]
		match number {
			__FIRST => ::core::result::Result::Ok(Self::First),
			__SECOND => ::core::result::Result::Ok(Self::Second),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}
const _: () = if !(::core::mem::size_of::<U16Discriminator>() == ::core::mem::size_of::<u16>()) {
	{
		::core::panicking::panic_fmt(format_args!(
			"The size of the enum `U16Discriminator` must match the size of its primitive \
			 representation\n\t\t\t\t`u16`.",
		));
	}
};
impl ::pina::IntoDiscriminator for U16Discriminator {
	fn discriminator_from_bytes(
		bytes: &[u8],
	) -> ::core::result::Result<Self, ::pina::ProgramError> {
		<u16 as ::pina::IntoDiscriminator>::discriminator_from_bytes(bytes)
			.and_then(|primitive| Self::try_from(primitive))
	}

	fn write_discriminator(&self, bytes: &mut [u8]) {
		(*self as u16).write_discriminator(bytes);
	}

	fn matches_discriminator(&self, bytes: &[u8]) -> bool {
		(*self as u16).matches_discriminator(bytes)
	}
}
#[repr(u32)]
#[non_exhaustive]
pub enum U32Discriminator {
	First = 0,
	Second = 1,
}
#[automatically_derived]
impl ::core::fmt::Debug for U32Discriminator {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				U32Discriminator::First => "First",
				U32Discriminator::Second => "Second",
			},
		)
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for U32Discriminator {}
#[automatically_derived]
impl ::core::clone::Clone for U32Discriminator {
	#[inline]
	fn clone(&self) -> U32Discriminator {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for U32Discriminator {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for U32Discriminator {}
#[automatically_derived]
impl ::core::cmp::PartialEq for U32Discriminator {
	#[inline]
	fn eq(&self, other: &U32Discriminator) -> bool {
		let __self_discr = ::core::intrinsics::discriminant_value(self);
		let __arg1_discr = ::core::intrinsics::discriminant_value(other);
		__self_discr == __arg1_discr
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for U32Discriminator {
	#[inline]
	#[doc(hidden)]
	#[coverage(off)]
	fn assert_receiver_is_total_eq(&self) {}
}
const _: () = {
	if !(::core::mem::size_of::<u32>() <= ::pina::MAX_DISCRIMINATOR_SPACE) {
		{
			::core::panicking::panic_fmt(format_args!(
				"A discriminator with primitive `u32` (4 bytes) exceeds `MAX_DISCRIMINATOR_SPACE` \
				 and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, \
				 `u16`, `u32`, `u64`.",
			));
		}
	}
};
impl ::core::convert::From<U32Discriminator> for u32 {
	#[inline]
	fn from(enum_value: U32Discriminator) -> Self {
		enum_value as Self
	}
}
impl ::core::convert::TryFrom<u32> for U32Discriminator {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u32) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __FIRST: u32 = 0;
		const __SECOND: u32 = 1;
		#[deny(unreachable_patterns)]
		match number {
			__FIRST => ::core::result::Result::Ok(Self::First),
			__SECOND => ::core::result::Result::Ok(Self::Second),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}
const _: () = if !(::core::mem::size_of::<U32Discriminator>() == ::core::mem::size_of::<u32>()) {
	{
		::core::panicking::panic_fmt(format_args!(
			"The size of the enum `U32Discriminator` must match the size of its primitive \
			 representation\n\t\t\t\t`u32`.",
		));
	}
};
impl ::pina::IntoDiscriminator for U32Discriminator {
	fn discriminator_from_bytes(
		bytes: &[u8],
	) -> ::core::result::Result<Self, ::pina::ProgramError> {
		<u32 as ::pina::IntoDiscriminator>::discriminator_from_bytes(bytes)
			.and_then(|primitive| Self::try_from(primitive))
	}

	fn write_discriminator(&self, bytes: &mut [u8]) {
		(*self as u32).write_discriminator(bytes);
	}

	fn matches_discriminator(&self, bytes: &[u8]) -> bool {
		(*self as u32).matches_discriminator(bytes)
	}
}
#[repr(u64)]
#[non_exhaustive]
pub enum U64Discriminator {
	First = 0,
	Second = 1,
}
#[automatically_derived]
impl ::core::fmt::Debug for U64Discriminator {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(
			f,
			match self {
				U64Discriminator::First => "First",
				U64Discriminator::Second => "Second",
			},
		)
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for U64Discriminator {}
#[automatically_derived]
impl ::core::clone::Clone for U64Discriminator {
	#[inline]
	fn clone(&self) -> U64Discriminator {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for U64Discriminator {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for U64Discriminator {}
#[automatically_derived]
impl ::core::cmp::PartialEq for U64Discriminator {
	#[inline]
	fn eq(&self, other: &U64Discriminator) -> bool {
		let __self_discr = ::core::intrinsics::discriminant_value(self);
		let __arg1_discr = ::core::intrinsics::discriminant_value(other);
		__self_discr == __arg1_discr
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for U64Discriminator {
	#[inline]
	#[doc(hidden)]
	#[coverage(off)]
	fn assert_receiver_is_total_eq(&self) {}
}
const _: () = {
	if !(::core::mem::size_of::<u64>() <= ::pina::MAX_DISCRIMINATOR_SPACE) {
		{
			::core::panicking::panic_fmt(format_args!(
				"A discriminator with primitive `u64` (8 bytes) exceeds `MAX_DISCRIMINATOR_SPACE` \
				 and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, \
				 `u16`, `u32`, `u64`.",
			));
		}
	}
};
impl ::core::convert::From<U64Discriminator> for u64 {
	#[inline]
	fn from(enum_value: U64Discriminator) -> Self {
		enum_value as Self
	}
}
impl ::core::convert::TryFrom<u64> for U64Discriminator {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u64) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __FIRST: u64 = 0;
		const __SECOND: u64 = 1;
		#[deny(unreachable_patterns)]
		match number {
			__FIRST => ::core::result::Result::Ok(Self::First),
			__SECOND => ::core::result::Result::Ok(Self::Second),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}
const _: () = if !(::core::mem::size_of::<U64Discriminator>() == ::core::mem::size_of::<u64>()) {
	{
		::core::panicking::panic_fmt(format_args!(
			"The size of the enum `U64Discriminator` must match the size of its primitive \
			 representation\n\t\t\t\t`u64`.",
		));
	}
};
impl ::pina::IntoDiscriminator for U64Discriminator {
	fn discriminator_from_bytes(
		bytes: &[u8],
	) -> ::core::result::Result<Self, ::pina::ProgramError> {
		<u64 as ::pina::IntoDiscriminator>::discriminator_from_bytes(bytes)
			.and_then(|primitive| Self::try_from(primitive))
	}

	fn write_discriminator(&self, bytes: &mut [u8]) {
		(*self as u64).write_discriminator(bytes);
	}

	fn matches_discriminator(&self, bytes: &[u8]) -> bool {
		(*self as u64).matches_discriminator(bytes)
	}
}
#[repr(u8)]
pub enum FinalDiscriminator {
	Only = 0,
}
#[automatically_derived]
impl ::core::fmt::Debug for FinalDiscriminator {
	#[inline]
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
		::core::fmt::Formatter::write_str(f, "Only")
	}
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for FinalDiscriminator {}
#[automatically_derived]
impl ::core::clone::Clone for FinalDiscriminator {
	#[inline]
	fn clone(&self) -> FinalDiscriminator {
		*self
	}
}
#[automatically_derived]
impl ::core::marker::Copy for FinalDiscriminator {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for FinalDiscriminator {}
#[automatically_derived]
impl ::core::cmp::PartialEq for FinalDiscriminator {
	#[inline]
	fn eq(&self, other: &FinalDiscriminator) -> bool {
		true
	}
}
#[automatically_derived]
impl ::core::cmp::Eq for FinalDiscriminator {
	#[inline]
	#[doc(hidden)]
	#[coverage(off)]
	fn assert_receiver_is_total_eq(&self) {}
}
const _: () = {
	if !(::core::mem::size_of::<u8>() <= ::pina::MAX_DISCRIMINATOR_SPACE) {
		{
			::core::panicking::panic_fmt(format_args!(
				"A discriminator with primitive `u8` (1 bytes) exceeds `MAX_DISCRIMINATOR_SPACE` \
				 and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, \
				 `u16`, `u32`, `u64`.",
			));
		}
	}
};
impl ::core::convert::From<FinalDiscriminator> for u8 {
	#[inline]
	fn from(enum_value: FinalDiscriminator) -> Self {
		enum_value as Self
	}
}
impl ::core::convert::TryFrom<u8> for FinalDiscriminator {
	type Error = ::pina::ProgramError;

	#[inline]
	fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
		#![allow(non_upper_case_globals)]
		const __ONLY: u8 = 0;
		#[deny(unreachable_patterns)]
		match number {
			__ONLY => ::core::result::Result::Ok(Self::Only),
			#[allow(unreachable_patterns)]
			_ => ::core::result::Result::Err(::pina::PinaProgramError::InvalidDiscriminator.into()),
		}
	}
}
const _: () = if !(::core::mem::size_of::<FinalDiscriminator>() == ::core::mem::size_of::<u8>()) {
	{
		::core::panicking::panic_fmt(format_args!(
			"The size of the enum `FinalDiscriminator` must match the size of its primitive \
			 representation\n\t\t\t\t`u8`.",
		));
	}
};
impl ::pina::IntoDiscriminator for FinalDiscriminator {
	fn discriminator_from_bytes(
		bytes: &[u8],
	) -> ::core::result::Result<Self, ::pina::ProgramError> {
		<u8 as ::pina::IntoDiscriminator>::discriminator_from_bytes(bytes)
			.and_then(|primitive| Self::try_from(primitive))
	}

	fn write_discriminator(&self, bytes: &mut [u8]) {
		(*self as u8).write_discriminator(bytes);
	}

	fn matches_discriminator(&self, bytes: &[u8]) -> bool {
		(*self as u8).matches_discriminator(bytes)
	}
}
