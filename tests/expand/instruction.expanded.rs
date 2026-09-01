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
pub struct Initialize {
    discriminator: [u8; InstructionDisc::BYTES],
}
#[repr(C)]
pub struct InitializeZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
{
    discriminator: [u8; InstructionDisc::BYTES],
}
impl Copy for InitializeZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
{}
impl Clone for InitializeZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(core::mem::align_of::<InitializeZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: core::mem::align_of::<InitializeZc>() == 1",
    )
};
impl InitializeZc {
    #[inline(always)]
    pub fn discriminator(&self) -> &[u8; InstructionDisc::BYTES] {
        &self.discriminator
    }
}
impl zeropod::ZcValidate for InitializeZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
{
    fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
        <[u8; InstructionDisc::BYTES] as zeropod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        Ok(())
    }
}
impl zeropod::ZeroPodSchema for Initialize
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
{
    const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for Initialize
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
{
    type Zc = InitializeZc;
    const SIZE: usize = core::mem::size_of::<InitializeZc>();
    fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
    }
    fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
    }
    fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
        if data.len() < core::mem::size_of::<InitializeZc>() {
            return Err(zeropod::ZeroPodError::BufferTooSmall);
        }
        let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
        <Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
        Ok(())
    }
}
impl zeropod::ZcField for Initialize
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
{
    type Pod = InitializeZc;
    const POD_SIZE: usize = core::mem::size_of::<InitializeZc>();
}
unsafe impl zeropod::ZcElem for InitializeZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
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
    if !(::core::mem::size_of::<InitializeZc>() == InstructionDisc::BYTES) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<InitializeZc>() == InstructionDisc::BYTES",
        )
    }
};
impl Initialize {
    /// The exact number of bytes required by the zeropod representation.
    pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;
    /// Validate `data` and return zeropod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::ZeroPodFixed>::from_bytes(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage and return its mutable zero-copy view.
    ///
    /// The complete slice is initialized before zeropod validates it. The
    /// returned borrow prevents the caller from observing or changing the raw
    /// bytes while the typed view is live.
    ///
    /// Every accepted field has an audited all-zero representation. The macro
    /// rejects custom types and other layouts whose zero state cannot be
    /// established by Pina's closed schema grammar.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong
    /// length or zeroed storage is not a valid zeropod representation.
    pub fn initialize(
        data: &mut [u8],
    ) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        data.fill(0);
        <Self as pina::HasDiscriminator>::write_discriminator(data);
        <Self as pina::ZeroPodFixed>::from_bytes_mut(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for Initialize {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::Initialize;
}
pub struct FlipBit {
    discriminator: [u8; InstructionDisc::BYTES],
    pub section_index: u8,
    pub array_index: u8,
    pub offset: u8,
    pub value: u8,
}
#[repr(C)]
pub struct FlipBitZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
{
    discriminator: [u8; InstructionDisc::BYTES],
    pub section_index: u8,
    pub array_index: u8,
    pub offset: u8,
    pub value: u8,
}
impl Copy for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
{}
impl Clone for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(core::mem::align_of::<FlipBitZc>() == 1) {
    ::core::panicking::panic("assertion failed: core::mem::align_of::<FlipBitZc>() == 1")
};
impl FlipBitZc {
    #[inline(always)]
    pub fn discriminator(&self) -> &[u8; InstructionDisc::BYTES] {
        &self.discriminator
    }
    #[inline(always)]
    pub fn section_index(&self) -> u8 {
        self.section_index
    }
    #[inline(always)]
    pub fn array_index(&self) -> u8 {
        self.array_index
    }
    #[inline(always)]
    pub fn offset(&self) -> u8 {
        self.offset
    }
    #[inline(always)]
    pub fn value(&self) -> u8 {
        self.value
    }
}
impl zeropod::ZcValidate for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
{
    fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
        <[u8; InstructionDisc::BYTES] as zeropod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <u8 as zeropod::ZcValidate>::validate_ref(&value.section_index)?;
        <u8 as zeropod::ZcValidate>::validate_ref(&value.array_index)?;
        <u8 as zeropod::ZcValidate>::validate_ref(&value.offset)?;
        <u8 as zeropod::ZcValidate>::validate_ref(&value.value)?;
        Ok(())
    }
}
impl zeropod::ZeroPodSchema for FlipBit
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
{
    const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for FlipBit
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
{
    type Zc = FlipBitZc;
    const SIZE: usize = core::mem::size_of::<FlipBitZc>();
    fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
    }
    fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
    }
    fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
        if data.len() < core::mem::size_of::<FlipBitZc>() {
            return Err(zeropod::ZeroPodError::BufferTooSmall);
        }
        let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
        <Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
        Ok(())
    }
}
impl zeropod::ZcField for FlipBit
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
{
    type Pod = FlipBitZc;
    const POD_SIZE: usize = core::mem::size_of::<FlipBitZc>();
}
unsafe impl zeropod::ZcElem for FlipBitZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
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
    if !(::core::mem::size_of::<::core::primitive::u8>()
        == <u8 as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<::core::primitive::u8>() ==\n    <u8 as pina::ZcField>::POD_SIZE",
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
    if !(::core::mem::size_of::<::core::primitive::u8>()
        == <u8 as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<::core::primitive::u8>() ==\n    <u8 as pina::ZcField>::POD_SIZE",
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
    if !(::core::mem::size_of::<::core::primitive::u8>()
        == <u8 as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<::core::primitive::u8>() ==\n    <u8 as pina::ZcField>::POD_SIZE",
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
    if !(::core::mem::size_of::<::core::primitive::u8>()
        == <u8 as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<::core::primitive::u8>() ==\n    <u8 as pina::ZcField>::POD_SIZE",
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
        == InstructionDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<FlipBitZc>() ==\n    InstructionDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>() +\n                ::core::mem::size_of::<::core::primitive::u8>() +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl FlipBit {
    /// The exact number of bytes required by the zeropod representation.
    pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;
    /// Validate `data` and return zeropod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::ZeroPodFixed>::from_bytes(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage and return its mutable zero-copy view.
    ///
    /// The complete slice is initialized before zeropod validates it. The
    /// returned borrow prevents the caller from observing or changing the raw
    /// bytes while the typed view is live.
    ///
    /// Every accepted field has an audited all-zero representation. The macro
    /// rejects custom types and other layouts whose zero state cannot be
    /// established by Pina's closed schema grammar.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong
    /// length or zeroed storage is not a valid zeropod representation.
    pub fn initialize(
        data: &mut [u8],
    ) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        data.fill(0);
        <Self as pina::HasDiscriminator>::write_discriminator(data);
        <Self as pina::ZeroPodFixed>::from_bytes_mut(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for FlipBit {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::FlipBit;
}
pub struct Transfer {
    discriminator: [u8; InstructionDisc::BYTES],
    pub amount: PodU64,
}
#[repr(C)]
pub struct TransferZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
    discriminator: [u8; InstructionDisc::BYTES],
    pub amount: <PodU64 as zeropod::ZcField>::Pod,
}
impl Copy for TransferZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{}
impl Clone for TransferZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(core::mem::align_of::<TransferZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: core::mem::align_of::<TransferZc>() == 1",
    )
};
impl TransferZc {
    #[inline(always)]
    pub fn discriminator(&self) -> &[u8; InstructionDisc::BYTES] {
        &self.discriminator
    }
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as zeropod::ZcField>::Pod {
        &self.amount
    }
}
impl zeropod::ZcValidate for TransferZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
    fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
        <[u8; InstructionDisc::BYTES] as zeropod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<PodU64 as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        Ok(())
    }
}
impl zeropod::ZeroPodSchema for Transfer
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
    const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for Transfer
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
    type Zc = TransferZc;
    const SIZE: usize = core::mem::size_of::<TransferZc>();
    fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
    }
    fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
    }
    fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
        if data.len() < core::mem::size_of::<TransferZc>() {
            return Err(zeropod::ZeroPodError::BufferTooSmall);
        }
        let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
        <Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
        Ok(())
    }
}
impl zeropod::ZcField for Transfer
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
{
    type Pod = TransferZc;
    const POD_SIZE: usize = core::mem::size_of::<TransferZc>();
}
unsafe impl zeropod::ZcElem for TransferZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
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
    if !(::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE",
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
        == InstructionDisc::BYTES + ::core::mem::size_of::<pina::PodU64>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<TransferZc>() ==\n    InstructionDisc::BYTES + ::core::mem::size_of::<pina::PodU64>()",
        )
    }
};
impl Transfer {
    /// The exact number of bytes required by the zeropod representation.
    pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;
    /// Validate `data` and return zeropod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::ZeroPodFixed>::from_bytes(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage and return its mutable zero-copy view.
    ///
    /// The complete slice is initialized before zeropod validates it. The
    /// returned borrow prevents the caller from observing or changing the raw
    /// bytes while the typed view is live.
    ///
    /// Every accepted field has an audited all-zero representation. The macro
    /// rejects custom types and other layouts whose zero state cannot be
    /// established by Pina's closed schema grammar.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong
    /// length or zeroed storage is not a valid zeropod representation.
    pub fn initialize(
        data: &mut [u8],
    ) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        data.fill(0);
        <Self as pina::HasDiscriminator>::write_discriminator(data);
        <Self as pina::ZeroPodFixed>::from_bytes_mut(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for Transfer {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::Transfer;
}
pub struct CustomTransferData {
    discriminator: [u8; InstructionDisc::BYTES],
    pub amount: PodU64,
    pub destination: [u8; 32],
}
#[repr(C)]
pub struct CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
{
    discriminator: [u8; InstructionDisc::BYTES],
    pub amount: <PodU64 as zeropod::ZcField>::Pod,
    pub destination: [u8; 32],
}
impl Copy for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
{}
impl Clone for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(core::mem::align_of::<CustomTransferDataZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: core::mem::align_of::<CustomTransferDataZc>() == 1",
    )
};
impl CustomTransferDataZc {
    #[inline(always)]
    pub fn discriminator(&self) -> &[u8; InstructionDisc::BYTES] {
        &self.discriminator
    }
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as zeropod::ZcField>::Pod {
        &self.amount
    }
    #[inline(always)]
    pub fn destination(&self) -> &[u8; 32] {
        &self.destination
    }
}
impl zeropod::ZcValidate for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
{
    fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
        <[u8; InstructionDisc::BYTES] as zeropod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<PodU64 as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        <[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.destination)?;
        Ok(())
    }
}
impl zeropod::ZeroPodSchema for CustomTransferData
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
{
    const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for CustomTransferData
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
{
    type Zc = CustomTransferDataZc;
    const SIZE: usize = core::mem::size_of::<CustomTransferDataZc>();
    fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
    }
    fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
    }
    fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
        if data.len() < core::mem::size_of::<CustomTransferDataZc>() {
            return Err(zeropod::ZeroPodError::BufferTooSmall);
        }
        let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
        <Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
        Ok(())
    }
}
impl zeropod::ZcField for CustomTransferData
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
{
    type Pod = CustomTransferDataZc;
    const POD_SIZE: usize = core::mem::size_of::<CustomTransferDataZc>();
}
unsafe impl zeropod::ZcElem for CustomTransferDataZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
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
    if !(::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE",
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
    if !(::core::mem::size_of::<[::core::primitive::u8; 32]>()
        == <[u8; 32] as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 32]>() ==\n    <[u8; 32] as pina::ZcField>::POD_SIZE",
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
        == InstructionDisc::BYTES + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<CustomTransferDataZc>() ==\n    InstructionDisc::BYTES + ::core::mem::size_of::<pina::PodU64>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 32]>()",
        )
    }
};
impl CustomTransferData {
    /// The exact number of bytes required by the zeropod representation.
    pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;
    /// Validate `data` and return zeropod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::ZeroPodFixed>::from_bytes(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage and return its mutable zero-copy view.
    ///
    /// The complete slice is initialized before zeropod validates it. The
    /// returned borrow prevents the caller from observing or changing the raw
    /// bytes while the typed view is live.
    ///
    /// Every accepted field has an audited all-zero representation. The macro
    /// rejects custom types and other layouts whose zero state cannot be
    /// established by Pina's closed schema grammar.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong
    /// length or zeroed storage is not a valid zeropod representation.
    pub fn initialize(
        data: &mut [u8],
    ) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        data.fill(0);
        <Self as pina::HasDiscriminator>::write_discriminator(data);
        <Self as pina::ZeroPodFixed>::from_bytes_mut(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for CustomTransferData {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::TransferData;
}
pub struct ComplexInstruction {
    discriminator: [u8; InstructionDisc::BYTES],
    pub seed: [u8; 32],
    pub amount: PodU64,
    pub bump: u8,
    pub flags: [u8; 4],
}
#[repr(C)]
pub struct ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
{
    discriminator: [u8; InstructionDisc::BYTES],
    pub seed: [u8; 32],
    pub amount: <PodU64 as zeropod::ZcField>::Pod,
    pub bump: u8,
    pub flags: [u8; 4],
}
impl Copy for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
{}
impl Clone for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(core::mem::align_of::<ComplexInstructionZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: core::mem::align_of::<ComplexInstructionZc>() == 1",
    )
};
impl ComplexInstructionZc {
    #[inline(always)]
    pub fn discriminator(&self) -> &[u8; InstructionDisc::BYTES] {
        &self.discriminator
    }
    #[inline(always)]
    pub fn seed(&self) -> &[u8; 32] {
        &self.seed
    }
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as zeropod::ZcField>::Pod {
        &self.amount
    }
    #[inline(always)]
    pub fn bump(&self) -> u8 {
        self.bump
    }
    #[inline(always)]
    pub fn flags(&self) -> &[u8; 4] {
        &self.flags
    }
}
impl zeropod::ZcValidate for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
{
    fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
        <[u8; InstructionDisc::BYTES] as zeropod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <[u8; 32] as zeropod::ZcValidate>::validate_ref(&value.seed)?;
        <<PodU64 as zeropod::ZcField>::Pod as zeropod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        <u8 as zeropod::ZcValidate>::validate_ref(&value.bump)?;
        <[u8; 4] as zeropod::ZcValidate>::validate_ref(&value.flags)?;
        Ok(())
    }
}
impl zeropod::ZeroPodSchema for ComplexInstruction
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
{
    const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
}
impl zeropod::ZeroPodFixed for ComplexInstruction
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
{
    type Zc = ComplexInstructionZc;
    const SIZE: usize = core::mem::size_of::<ComplexInstructionZc>();
    fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
    }
    fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
        Self::validate(data)?;
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
    }
    fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
        if data.len() < core::mem::size_of::<ComplexInstructionZc>() {
            return Err(zeropod::ZeroPodError::BufferTooSmall);
        }
        let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
        <Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
        Ok(())
    }
}
impl zeropod::ZcField for ComplexInstruction
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
{
    type Pod = ComplexInstructionZc;
    const POD_SIZE: usize = core::mem::size_of::<ComplexInstructionZc>();
}
unsafe impl zeropod::ZcElem for ComplexInstructionZc
where
    [u8; InstructionDisc::BYTES]: zeropod::ZcValidate,
    [u8; 32]: zeropod::ZcValidate,
    <PodU64 as zeropod::ZcField>::Pod: zeropod::ZcValidate,
    u8: zeropod::ZcValidate,
    [u8; 4]: zeropod::ZcValidate,
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
    if !(::core::mem::size_of::<[::core::primitive::u8; 32]>()
        == <[u8; 32] as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 32]>() ==\n    <[u8; 32] as pina::ZcField>::POD_SIZE",
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
    if !(::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<pina::PodU64>() == <PodU64 as pina::ZcField>::POD_SIZE",
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
    if !(::core::mem::size_of::<::core::primitive::u8>()
        == <u8 as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<::core::primitive::u8>() ==\n    <u8 as pina::ZcField>::POD_SIZE",
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
    if !(::core::mem::size_of::<[::core::primitive::u8; 4]>()
        == <[u8; 4] as pina::ZcField>::POD_SIZE)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<[::core::primitive::u8; 4]>() ==\n    <[u8; 4] as pina::ZcField>::POD_SIZE",
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
        == InstructionDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<[::core::primitive::u8; 4]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<ComplexInstructionZc>() ==\n    InstructionDisc::BYTES +\n                    ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n                ::core::mem::size_of::<pina::PodU64>() +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 4]>()",
        )
    }
};
impl ComplexInstruction {
    /// The exact number of bytes required by the zeropod representation.
    pub const SIZE: usize = <Self as pina::ZeroPodFixed>::SIZE;
    /// Validate `data` and return zeropod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        <Self as pina::ZeroPodFixed>::from_bytes(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
    /// Initialize caller-owned storage and return its mutable zero-copy view.
    ///
    /// The complete slice is initialized before zeropod validates it. The
    /// returned borrow prevents the caller from observing or changing the raw
    /// bytes while the typed view is live.
    ///
    /// Every accepted field has an audited all-zero representation. The macro
    /// rejects custom types and other layouts whose zero state cannot be
    /// established by Pina's closed schema grammar.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong
    /// length or zeroed storage is not a valid zeropod representation.
    pub fn initialize(
        data: &mut [u8],
    ) -> Result<&mut <Self as pina::ZeroPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE {
            return Err(pina::ProgramError::InvalidInstructionData);
        }
        data.fill(0);
        <Self as pina::HasDiscriminator>::write_discriminator(data);
        <Self as pina::ZeroPodFixed>::from_bytes_mut(data)
            .map_err(|_| pina::ProgramError::InvalidInstructionData)
    }
}
impl pina::HasDiscriminator for ComplexInstruction {
    type Type = InstructionDisc;
    const VALUE: Self::Type = InstructionDisc::ComplexInstruction;
}
