use pina::*;
#[repr(u8)]
#[non_exhaustive]
pub enum InstructionDisc {
    Initialize = 0,
    FlipBit = 1,
    Transfer = 2,
    TransferData = 3,
    ComplexInstruction = 4,
}
#[automatically_derived]
impl ::core::fmt::Debug for InstructionDisc {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(
            f,
            match self {
                InstructionDisc::Initialize => "Initialize",
                InstructionDisc::FlipBit => "FlipBit",
                InstructionDisc::Transfer => "Transfer",
                InstructionDisc::TransferData => "TransferData",
                InstructionDisc::ComplexInstruction => "ComplexInstruction",
            },
        )
    }
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for InstructionDisc {}
#[automatically_derived]
impl ::core::clone::Clone for InstructionDisc {
    #[inline]
    fn clone(&self) -> InstructionDisc {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for InstructionDisc {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for InstructionDisc {}
#[automatically_derived]
impl ::core::cmp::PartialEq for InstructionDisc {
    #[inline]
    fn eq(&self, other: &InstructionDisc) -> bool {
        let __self_discr = ::core::intrinsics::discriminant_value(self);
        let __arg1_discr = ::core::intrinsics::discriminant_value(other);
        __self_discr == __arg1_discr
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for InstructionDisc {
    #[inline]
    #[doc(hidden)]
    #[coverage(off)]
    fn assert_receiver_is_total_eq(&self) {}
}
const _: () = {
    if !(::core::mem::size_of::<u8>() <= ::pina::MAX_DISCRIMINATOR_SPACE) {
        {
            ::core::panicking::panic_fmt(
                format_args!(
                    "A discriminator with primitive `u8` (1 bytes) exceeds `MAX_DISCRIMINATOR_SPACE` and cannot be safely used for zero-copy layouts. Supported primitives: `u8`, `u16`, `u32`, `u64`.",
                ),
            );
        }
    }
};
impl ::core::convert::From<InstructionDisc> for u8 {
    #[inline]
    fn from(enum_value: InstructionDisc) -> Self {
        enum_value as Self
    }
}
impl ::core::convert::TryFrom<u8> for InstructionDisc {
    type Error = ::pina::ProgramError;
    #[inline]
    fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
        #![allow(non_upper_case_globals)]
        const __INITIALIZE: u8 = 0;
        const __FLIP_BIT: u8 = 1;
        const __TRANSFER: u8 = 2;
        const __TRANSFER_DATA: u8 = 3;
        const __COMPLEX_INSTRUCTION: u8 = 4;
        #[deny(unreachable_patterns)]
        match number {
            __INITIALIZE => ::core::result::Result::Ok(Self::Initialize),
            __FLIP_BIT => ::core::result::Result::Ok(Self::FlipBit),
            __TRANSFER => ::core::result::Result::Ok(Self::Transfer),
            __TRANSFER_DATA => ::core::result::Result::Ok(Self::TransferData),
            __COMPLEX_INSTRUCTION => ::core::result::Result::Ok(Self::ComplexInstruction),
            #[allow(unreachable_patterns)]
            _ => {
                ::core::result::Result::Err(
                    ::pina::PinaProgramError::InvalidDiscriminator.into(),
                )
            }
        }
    }
}
const _: () = if !(::core::mem::size_of::<InstructionDisc>()
    == ::core::mem::size_of::<u8>())
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "The size of the enum `InstructionDisc` must match the size of its primitive representation\n\t\t\t\t`u8`.",
            ),
        );
    }
};
impl ::pina::IntoDiscriminator for InstructionDisc {
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
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct Initialize {
    #[pinapod(skip_accessor)]
    discriminator: [u8; InstructionDisc::BYTES],
}
#[repr(C)]
pub struct InitializeZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for InitializeZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for InitializeZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<InitializeZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<InitializeZc>() == 1",
    )
};
impl pina::pinapod::ZcValidate for InitializeZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for Initialize
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for Initialize
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = InitializeZc;
}
unsafe impl pina::pinapod::ZcField for Initialize
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = InitializeZc;
}
unsafe impl pina::pinapod::ZcElem for InitializeZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<InitializeZc>();
};
const _: () = {
    if !(::core::mem::align_of::<InitializeZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<InitializeZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<InitializeZc>() == InstructionDisc::BYTES + 0usize) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<InitializeZc>() == InstructionDisc::BYTES + 0usize",
        )
    }
};
impl Initialize {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, `PinaPod` zeros the complete slice again.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or the completed representation is invalid.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        <Self as pina::PinaPodFixed>::initialize(
                data,
                |value| {
                    <Self as pina::HasDiscriminator>::write_discriminator(
                        &mut value.discriminator,
                    );
                    initialize(value)
                },
            )
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for Initialize {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::Initialize;
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct FlipBit {
    #[pinapod(skip_accessor)]
    discriminator: [u8; InstructionDisc::BYTES],
    pub section_index: u8,
    pub array_index: u8,
    pub offset: u8,
    pub value: u8,
}
#[repr(C)]
pub struct FlipBitZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub section_index: <u8 as pina::pinapod::ZcField>::Pod,
    pub array_index: <u8 as pina::pinapod::ZcField>::Pod,
    pub offset: <u8 as pina::pinapod::ZcField>::Pod,
    pub value: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<FlipBitZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<FlipBitZc>() == 1",
    )
};
impl FlipBitZc {
    #[inline(always)]
    pub fn section_index(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.section_index
    }
    #[inline(always)]
    pub fn array_index(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.array_index
    }
    #[inline(always)]
    pub fn offset(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.offset
    }
    #[inline(always)]
    pub fn value(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.value
    }
}
impl pina::pinapod::ZcValidate for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.section_index,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.array_index,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.offset,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.value,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for FlipBit
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for FlipBit
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = FlipBitZc;
}
unsafe impl pina::pinapod::ZcField for FlipBit
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = FlipBitZc;
}
unsafe impl pina::pinapod::ZcElem for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
const _: fn(u8) -> ::core::primitive::u8 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = ::core::primitive::u8>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<u8>();
    assert_storage::<::core::primitive::u8>();
};
const _: () = {
    if !(::core::mem::align_of::<::core::primitive::u8>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<::core::primitive::u8>() == 1",
        )
    }
};
const _: fn(u8) -> ::core::primitive::u8 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = ::core::primitive::u8>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<u8>();
    assert_storage::<::core::primitive::u8>();
};
const _: () = {
    if !(::core::mem::align_of::<::core::primitive::u8>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<::core::primitive::u8>() == 1",
        )
    }
};
const _: fn(u8) -> ::core::primitive::u8 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = ::core::primitive::u8>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<u8>();
    assert_storage::<::core::primitive::u8>();
};
const _: () = {
    if !(::core::mem::align_of::<::core::primitive::u8>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<::core::primitive::u8>() == 1",
        )
    }
};
const _: fn(u8) -> ::core::primitive::u8 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = ::core::primitive::u8>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<u8>();
    assert_storage::<::core::primitive::u8>();
};
const _: () = {
    if !(::core::mem::align_of::<::core::primitive::u8>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<::core::primitive::u8>() == 1",
        )
    }
};
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<FlipBitZc>();
};
const _: () = {
    if !(::core::mem::align_of::<FlipBitZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<FlipBitZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<FlipBitZc>()
        == InstructionDisc::BYTES + 0usize
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<FlipBitZc>() ==\n    InstructionDisc::BYTES + 0usize +\n                    ::core::mem::size_of::<::core::primitive::u8>() +\n                ::core::mem::size_of::<::core::primitive::u8>() +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl FlipBit {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, `PinaPod` zeros the complete slice again.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or the completed representation is invalid.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        <Self as pina::PinaPodFixed>::initialize(
                data,
                |value| {
                    <Self as pina::HasDiscriminator>::write_discriminator(
                        &mut value.discriminator,
                    );
                    initialize(value)
                },
            )
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for FlipBit {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::FlipBit;
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct Transfer {
    #[pinapod(skip_accessor)]
    discriminator: [u8; InstructionDisc::BYTES],
    pub amount: PodU64,
}
#[repr(C)]
pub struct TransferZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub amount: <PodU64 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for TransferZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for TransferZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<TransferZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<TransferZc>() == 1",
    )
};
impl TransferZc {
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.amount
    }
}
impl pina::pinapod::ZcValidate for TransferZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for Transfer
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for Transfer
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = TransferZc;
}
unsafe impl pina::pinapod::ZcField for Transfer
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = TransferZc;
}
unsafe impl pina::pinapod::ZcElem for TransferZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
const _: fn(PodU64) -> pina::PodU64 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::PodU64>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<PodU64>();
    assert_storage::<pina::PodU64>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::PodU64>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::PodU64>() == 1",
        )
    }
};
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<TransferZc>();
};
const _: () = {
    if !(::core::mem::align_of::<TransferZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<TransferZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<TransferZc>()
        == InstructionDisc::BYTES + 0usize + ::core::mem::size_of::<pina::PodU64>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<TransferZc>() ==\n    InstructionDisc::BYTES + 0usize + ::core::mem::size_of::<pina::PodU64>()",
        )
    }
};
impl Transfer {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, `PinaPod` zeros the complete slice again.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or the completed representation is invalid.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        <Self as pina::PinaPodFixed>::initialize(
                data,
                |value| {
                    <Self as pina::HasDiscriminator>::write_discriminator(
                        &mut value.discriminator,
                    );
                    initialize(value)
                },
            )
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for Transfer {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::Transfer;
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct CustomTransferData {
    #[pinapod(skip_accessor)]
    discriminator: [u8; InstructionDisc::BYTES],
    pub amount: PodU64,
    pub destination: [u8; 32],
}
#[repr(C)]
pub struct CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub amount: <PodU64 as pina::pinapod::ZcField>::Pod,
    pub destination: <[u8; 32] as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<CustomTransferDataZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<CustomTransferDataZc>() == 1",
    )
};
impl CustomTransferDataZc {
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.amount
    }
    #[inline(always)]
    pub fn destination(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.destination
    }
}
impl pina::pinapod::ZcValidate for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.destination,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for CustomTransferData
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for CustomTransferData
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = CustomTransferDataZc;
}
unsafe impl pina::pinapod::ZcField for CustomTransferData
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = CustomTransferDataZc;
}
unsafe impl pina::pinapod::ZcElem for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
const _: fn(PodU64) -> pina::PodU64 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::PodU64>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<PodU64>();
    assert_storage::<pina::PodU64>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::PodU64>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::PodU64>() == 1",
        )
    }
};
const _: fn([u8; 32]) -> [::core::primitive::u8; 32] = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 32]>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<[u8; 32]>();
    assert_storage::<[::core::primitive::u8; 32]>();
};
const _: () = {
    if !(::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1",
        )
    }
};
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<CustomTransferDataZc>();
};
const _: () = {
    if !(::core::mem::align_of::<CustomTransferDataZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<CustomTransferDataZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<CustomTransferDataZc>()
        == InstructionDisc::BYTES + 0usize + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<CustomTransferDataZc>() ==\n    InstructionDisc::BYTES + 0usize + ::core::mem::size_of::<pina::PodU64>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 32]>()",
        )
    }
};
impl CustomTransferData {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, `PinaPod` zeros the complete slice again.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or the completed representation is invalid.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        <Self as pina::PinaPodFixed>::initialize(
                data,
                |value| {
                    <Self as pina::HasDiscriminator>::write_discriminator(
                        &mut value.discriminator,
                    );
                    initialize(value)
                },
            )
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for CustomTransferData {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::TransferData;
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct ComplexInstruction {
    #[pinapod(skip_accessor)]
    discriminator: [u8; InstructionDisc::BYTES],
    pub seed: [u8; 32],
    pub amount: PodU64,
    pub bump: u8,
    pub flags: [u8; 4],
}
#[repr(C)]
pub struct ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub seed: <[u8; 32] as pina::pinapod::ZcField>::Pod,
    pub amount: <PodU64 as pina::pinapod::ZcField>::Pod,
    pub bump: <u8 as pina::pinapod::ZcField>::Pod,
    pub flags: <[u8; 4] as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<ComplexInstructionZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<ComplexInstructionZc>() == 1",
    )
};
impl ComplexInstructionZc {
    #[inline(always)]
    pub fn seed(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.seed
    }
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.amount
    }
    #[inline(always)]
    pub fn bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.bump
    }
    #[inline(always)]
    pub fn flags(&self) -> &<[u8; 4] as pina::pinapod::ZcField>::Pod {
        &self.flags
    }
}
impl pina::pinapod::ZcValidate for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.seed,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.bump,
        )?;
        <<[u8; 4] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.flags,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for ComplexInstruction
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for ComplexInstruction
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = ComplexInstructionZc;
}
unsafe impl pina::pinapod::ZcField for ComplexInstruction
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = ComplexInstructionZc;
}
unsafe impl pina::pinapod::ZcElem for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; InstructionDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
const _: fn([u8; 32]) -> [::core::primitive::u8; 32] = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 32]>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<[u8; 32]>();
    assert_storage::<[::core::primitive::u8; 32]>();
};
const _: () = {
    if !(::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 32]>() == 1",
        )
    }
};
const _: fn(PodU64) -> pina::PodU64 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::PodU64>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<PodU64>();
    assert_storage::<pina::PodU64>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::PodU64>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::PodU64>() == 1",
        )
    }
};
const _: fn(u8) -> ::core::primitive::u8 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = ::core::primitive::u8>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<u8>();
    assert_storage::<::core::primitive::u8>();
};
const _: () = {
    if !(::core::mem::align_of::<::core::primitive::u8>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<::core::primitive::u8>() == 1",
        )
    }
};
const _: fn([u8; 4]) -> [::core::primitive::u8; 4] = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 4]>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<[u8; 4]>();
    assert_storage::<[::core::primitive::u8; 4]>();
};
const _: () = {
    if !(::core::mem::align_of::<[::core::primitive::u8; 4]>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 4]>() == 1",
        )
    }
};
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<ComplexInstructionZc>();
};
const _: () = {
    if !(::core::mem::align_of::<ComplexInstructionZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<ComplexInstructionZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<ComplexInstructionZc>()
        == InstructionDisc::BYTES + 0usize
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<[::core::primitive::u8; 4]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<ComplexInstructionZc>() ==\n    InstructionDisc::BYTES + 0usize +\n                    ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n                ::core::mem::size_of::<pina::PodU64>() +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 4]>()",
        )
    }
};
impl ComplexInstruction {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, `PinaPod` zeros the complete slice again.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or the completed representation is invalid.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        <Self as pina::PinaPodFixed>::initialize(
                data,
                |value| {
                    <Self as pina::HasDiscriminator>::write_discriminator(
                        &mut value.discriminator,
                    );
                    initialize(value)
                },
            )
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for ComplexInstruction {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::ComplexInstruction;
}
