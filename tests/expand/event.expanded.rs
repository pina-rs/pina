use pina::*;
#[repr(u8)]
#[non_exhaustive]
pub enum EventDisc {
    TransferEvent = 1,
    InitializeEvent = 2,
    EmptyEvent = 3,
    AuditEvent = 4,
}
#[automatically_derived]
impl ::core::fmt::Debug for EventDisc {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(
            f,
            match self {
                EventDisc::TransferEvent => "TransferEvent",
                EventDisc::InitializeEvent => "InitializeEvent",
                EventDisc::EmptyEvent => "EmptyEvent",
                EventDisc::AuditEvent => "AuditEvent",
            },
        )
    }
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for EventDisc {}
#[automatically_derived]
impl ::core::clone::Clone for EventDisc {
    #[inline]
    fn clone(&self) -> EventDisc {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for EventDisc {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for EventDisc {}
#[automatically_derived]
impl ::core::cmp::PartialEq for EventDisc {
    #[inline]
    fn eq(&self, other: &EventDisc) -> bool {
        let __self_discr = ::core::intrinsics::discriminant_value(self);
        let __arg1_discr = ::core::intrinsics::discriminant_value(other);
        __self_discr == __arg1_discr
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for EventDisc {
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
impl ::core::convert::From<EventDisc> for u8 {
    #[inline]
    fn from(enum_value: EventDisc) -> Self {
        enum_value as Self
    }
}
impl ::core::convert::TryFrom<u8> for EventDisc {
    type Error = ::pina::ProgramError;
    #[inline]
    fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
        #![allow(non_upper_case_globals)]
        const __TRANSFER_EVENT: u8 = 1;
        const __INITIALIZE_EVENT: u8 = 2;
        const __EMPTY_EVENT: u8 = 3;
        const __AUDIT_EVENT: u8 = 4;
        #[deny(unreachable_patterns)]
        match number {
            __TRANSFER_EVENT => ::core::result::Result::Ok(Self::TransferEvent),
            __INITIALIZE_EVENT => ::core::result::Result::Ok(Self::InitializeEvent),
            __EMPTY_EVENT => ::core::result::Result::Ok(Self::EmptyEvent),
            __AUDIT_EVENT => ::core::result::Result::Ok(Self::AuditEvent),
            #[allow(unreachable_patterns)]
            _ => {
                ::core::result::Result::Err(
                    ::pina::PinaProgramError::InvalidDiscriminator.into(),
                )
            }
        }
    }
}
const _: () = if !(::core::mem::size_of::<EventDisc>() == ::core::mem::size_of::<u8>()) {
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "The size of the enum `EventDisc` must match the size of its primitive representation\n\t\t\t\t`u8`.",
            ),
        );
    }
};
impl ::pina::IntoDiscriminator for EventDisc {
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
pub struct TransferEvent {
    #[pinapod(skip_accessor)]
    discriminator: [u8; EventDisc::BYTES],
    pub from: [u8; 32],
    pub to: [u8; 32],
    pub amount: PodU64,
}
#[repr(C)]
pub struct TransferEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub from: <[u8; 32] as pina::pinapod::ZcField>::Pod,
    pub to: <[u8; 32] as pina::pinapod::ZcField>::Pod,
    pub amount: <PodU64 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for TransferEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for TransferEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<TransferEventZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<TransferEventZc>() == 1",
    )
};
impl TransferEventZc {
    #[inline(always)]
    pub fn from(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.from
    }
    #[inline(always)]
    pub fn to(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.to
    }
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.amount
    }
}
impl pina::pinapod::ZcValidate for TransferEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.from,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.to,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for TransferEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for TransferEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = TransferEventZc;
}
unsafe impl pina::pinapod::ZcField for TransferEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = TransferEventZc;
}
unsafe impl pina::pinapod::ZcElem for TransferEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<TransferEventZc>();
};
const _: () = {
    if !(::core::mem::align_of::<TransferEventZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<TransferEventZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<TransferEventZc>()
        == EventDisc::BYTES + 0usize
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<pina::PodU64>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<TransferEventZc>() ==\n    EventDisc::BYTES + 0usize +\n                ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n            ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n        ::core::mem::size_of::<pina::PodU64>()",
        )
    }
};
impl TransferEvent {
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
impl pina::HasDiscriminator for TransferEvent {
    type Type = EventDisc;
    const VALUE: Self::Type = EventDisc::TransferEvent;
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct InitEvent {
    #[pinapod(skip_accessor)]
    discriminator: [u8; EventDisc::BYTES],
    pub choice: u8,
}
#[repr(C)]
pub struct InitEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub choice: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for InitEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for InitEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<InitEventZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<InitEventZc>() == 1",
    )
};
impl InitEventZc {
    #[inline(always)]
    pub fn choice(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.choice
    }
}
impl pina::pinapod::ZcValidate for InitEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.choice,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for InitEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for InitEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = InitEventZc;
}
unsafe impl pina::pinapod::ZcField for InitEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = InitEventZc;
}
unsafe impl pina::pinapod::ZcElem for InitEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<InitEventZc>();
};
const _: () = {
    if !(::core::mem::align_of::<InitEventZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<InitEventZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<InitEventZc>()
        == EventDisc::BYTES + 0usize + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<InitEventZc>() ==\n    EventDisc::BYTES + 0usize +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl InitEvent {
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
impl pina::HasDiscriminator for InitEvent {
    type Type = EventDisc;
    const VALUE: Self::Type = EventDisc::InitializeEvent;
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct EmptyEvent {
    #[pinapod(skip_accessor)]
    discriminator: [u8; EventDisc::BYTES],
}
#[repr(C)]
pub struct EmptyEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for EmptyEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for EmptyEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<EmptyEventZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<EmptyEventZc>() == 1",
    )
};
impl pina::pinapod::ZcValidate for EmptyEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for EmptyEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for EmptyEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = EmptyEventZc;
}
unsafe impl pina::pinapod::ZcField for EmptyEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = EmptyEventZc;
}
unsafe impl pina::pinapod::ZcElem for EmptyEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<EmptyEventZc>();
};
const _: () = {
    if !(::core::mem::align_of::<EmptyEventZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<EmptyEventZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<EmptyEventZc>() == EventDisc::BYTES + 0usize) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<EmptyEventZc>() == EventDisc::BYTES + 0usize",
        )
    }
};
impl EmptyEvent {
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
impl pina::HasDiscriminator for EmptyEvent {
    type Type = EventDisc;
    const VALUE: Self::Type = EventDisc::EmptyEvent;
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct AuditEvent {
    #[pinapod(skip_accessor)]
    discriminator: [u8; EventDisc::BYTES],
    pub action: u8,
    pub timestamp: PodU64,
}
#[automatically_derived]
impl ::core::fmt::Debug for AuditEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field3_finish(
            f,
            "AuditEvent",
            "discriminator",
            &self.discriminator,
            "action",
            &self.action,
            "timestamp",
            &&self.timestamp,
        )
    }
}
#[repr(C)]
pub struct AuditEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub action: <u8 as pina::pinapod::ZcField>::Pod,
    pub timestamp: <PodU64 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for AuditEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for AuditEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<AuditEventZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<AuditEventZc>() == 1",
    )
};
impl AuditEventZc {
    #[inline(always)]
    pub fn action(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.action
    }
    #[inline(always)]
    pub fn timestamp(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.timestamp
    }
}
impl pina::pinapod::ZcValidate for AuditEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.action,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.timestamp,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for AuditEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for AuditEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = AuditEventZc;
}
unsafe impl pina::pinapod::ZcField for AuditEvent
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = AuditEventZc;
}
unsafe impl pina::pinapod::ZcElem for AuditEventZc
where
    [u8; EventDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; EventDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
    assert_storage::<AuditEventZc>();
};
const _: () = {
    if !(::core::mem::align_of::<AuditEventZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<AuditEventZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<AuditEventZc>()
        == EventDisc::BYTES + 0usize + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<pina::PodU64>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<AuditEventZc>() ==\n    EventDisc::BYTES + 0usize +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<pina::PodU64>()",
        )
    }
};
impl AuditEvent {
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
impl pina::HasDiscriminator for AuditEvent {
    type Type = EventDisc;
    const VALUE: Self::Type = EventDisc::AuditEvent;
}
