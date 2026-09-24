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
impl ::core::cmp::Eq for MyError {}
#[allow(deprecated)]
const _: () = if !((MyError::Invalid as u32) < pina::RESERVED_ERROR_CODE_START) {
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `MyError::Invalid` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
#[allow(deprecated)]
const _: () = if !((MyError::Duplicate as u32) < pina::RESERVED_ERROR_CODE_START) {
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `MyError::Duplicate` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
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
#[allow(deprecated)]
const _: () = if !((FinalError::Unauthorized as u32) < pina::RESERVED_ERROR_CODE_START) {
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `FinalError::Unauthorized` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
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
#[allow(deprecated)]
const _: () = if !((DetailedError::InsufficientFunds as u32)
    < pina::RESERVED_ERROR_CODE_START)
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `DetailedError::InsufficientFunds` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
#[allow(deprecated)]
const _: () = if !((DetailedError::AlreadyInitialized as u32)
    < pina::RESERVED_ERROR_CODE_START)
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `DetailedError::AlreadyInitialized` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
#[allow(deprecated)]
const _: () = if !((DetailedError::InvalidAuthority as u32)
    < pina::RESERVED_ERROR_CODE_START)
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `DetailedError::InvalidAuthority` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
#[allow(deprecated)]
const _: () = if !((DetailedError::InvalidMint as u32) < pina::RESERVED_ERROR_CODE_START)
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `DetailedError::InvalidMint` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
#[allow(deprecated)]
const _: () = if !((DetailedError::Overflow as u32) < pina::RESERVED_ERROR_CODE_START) {
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `DetailedError::Overflow` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
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
#[allow(deprecated)]
const _: () = if !((DefaultCrateError::Something as u32)
    < ::pina::RESERVED_ERROR_CODE_START)
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "error discriminant for `DefaultCrateError::Something` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina\'s framework errors; use a value below 0xFFFF_0000",
            ),
        );
    }
};
impl ::core::convert::From<DefaultCrateError> for ::pina::ProgramError {
    fn from(e: DefaultCrateError) -> Self {
        ::pina::ProgramError::Custom(e as u32)
    }
}
