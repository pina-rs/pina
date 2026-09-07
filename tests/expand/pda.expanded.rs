use pina::*;
const SEED_COUNTER: &[u8] = b"counter";
#[repr(u8)]
#[non_exhaustive]
pub enum PdaDisc {
    CounterState = 1,
    AllSeedState = 2,
    TodoState = 3,
}
#[automatically_derived]
impl ::core::fmt::Debug for PdaDisc {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(
            f,
            match self {
                PdaDisc::CounterState => "CounterState",
                PdaDisc::AllSeedState => "AllSeedState",
                PdaDisc::TodoState => "TodoState",
            },
        )
    }
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for PdaDisc {}
#[automatically_derived]
impl ::core::clone::Clone for PdaDisc {
    #[inline]
    fn clone(&self) -> PdaDisc {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for PdaDisc {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for PdaDisc {}
#[automatically_derived]
impl ::core::cmp::PartialEq for PdaDisc {
    #[inline]
    fn eq(&self, other: &PdaDisc) -> bool {
        let __self_discr = ::core::intrinsics::discriminant_value(self);
        let __arg1_discr = ::core::intrinsics::discriminant_value(other);
        __self_discr == __arg1_discr
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for PdaDisc {
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
impl ::core::convert::From<PdaDisc> for u8 {
    #[inline]
    fn from(enum_value: PdaDisc) -> Self {
        enum_value as Self
    }
}
impl ::core::convert::TryFrom<u8> for PdaDisc {
    type Error = ::pina::ProgramError;
    #[inline]
    fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
        #![allow(non_upper_case_globals)]
        const __COUNTER_STATE: u8 = 1;
        const __ALL_SEED_STATE: u8 = 2;
        const __TODO_STATE: u8 = 3;
        #[deny(unreachable_patterns)]
        match number {
            __COUNTER_STATE => ::core::result::Result::Ok(Self::CounterState),
            __ALL_SEED_STATE => ::core::result::Result::Ok(Self::AllSeedState),
            __TODO_STATE => ::core::result::Result::Ok(Self::TodoState),
            #[allow(unreachable_patterns)]
            _ => {
                ::core::result::Result::Err(
                    ::pina::PinaProgramError::InvalidDiscriminator.into(),
                )
            }
        }
    }
}
const _: () = if !(::core::mem::size_of::<PdaDisc>() == ::core::mem::size_of::<u8>()) {
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "The size of the enum `PdaDisc` must match the size of its primitive representation\n\t\t\t\t`u8`.",
            ),
        );
    }
};
impl ::pina::IntoDiscriminator for PdaDisc {
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
pub struct CounterState {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; PdaDisc::BYTES],
    pub authority: Address,
    pub bump: u8,
}
#[repr(C)]
pub struct CounterStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub authority: <Address as pina::pinapod::ZcField>::Pod,
    pub bump: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for CounterStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for CounterStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<CounterStateZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<CounterStateZc>() == 1",
    )
};
impl CounterStateZc {
    #[inline(always)]
    pub fn authority(&self) -> &<Address as pina::pinapod::ZcField>::Pod {
        &self.authority
    }
    #[inline(always)]
    pub fn bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.bump
    }
}
impl pina::pinapod::ZcValidate for CounterStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<Address as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.authority,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.bump,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for CounterState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for CounterState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = CounterStateZc;
}
unsafe impl pina::pinapod::ZcField for CounterState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = CounterStateZc;
}
unsafe impl pina::pinapod::ZcElem for CounterStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
///The PDA seeds for `CounterState`.
pub struct CounterStateSeeds<'a> {
    ///The `authority` seed.
    pub authority: &'a Address,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl<'a> ::core::clone::TrivialClone for CounterStateSeeds<'a> {}
#[automatically_derived]
impl<'a> ::core::clone::Clone for CounterStateSeeds<'a> {
    #[inline]
    fn clone(&self) -> CounterStateSeeds<'a> {
        let _: ::core::clone::AssertParamIsClone<&'a Address>;
        *self
    }
}
#[automatically_derived]
impl<'a> ::core::marker::Copy for CounterStateSeeds<'a> {}
///The PDA seeds for `CounterState`, including the bump seed.
pub struct CounterStateSeedsWithBump<'a> {
    inner: CounterStateSeeds<'a>,
    _bump: [u8; 1],
}
impl CounterState {
    /// Build the PDA seeds for this account.
    pub fn seeds<'a>(authority: &'a Address) -> CounterStateSeeds<'a> {
        CounterStateSeeds {
            authority: authority,
        }
    }
    /// Find the canonical PDA for this account and its bump seed.
    pub fn try_find_pda(
        authority: &Address,
        program_id: &Address,
    ) -> ::core::option::Option<(pina::Address, u8)> {
        let seeds = Self::seeds(authority);
        pina::try_find_program_address(&seeds.as_slices(), program_id)
    }
    /// Find the canonical PDA for this account and its bump seed.
    ///
    /// # Panics
    ///
    /// Panics if no valid PDA exists for the given seeds.
    pub fn find_pda(authority: &Address, program_id: &Address) -> (pina::Address, u8) {
        Self::try_find_pda(authority, program_id)
            .unwrap_or_else(|| {
                ::core::panicking::panic_fmt(
                    format_args!("could not find program address from seeds"),
                );
            })
    }
    ///Assert that `account` is the PDA for the given seeds, using the stored `bump` field.
    pub fn assert_seeds(
        account: &pina::AccountView,
        authority: &Address,
        program_id: &pina::Address,
    ) -> ::core::result::Result<(), pina::ProgramError> {
        let bump = pina::AsAccount::as_account::<Self>(account, program_id)?.bump;
        let seeds = Self::seeds(authority).with_bump(bump);
        <&pina::AccountView as pina::AccountInfoValidation>::assert_seeds_with_bump(
                account,
                &seeds.as_slices(),
                program_id,
            )
            .map(|_| ())
    }
}
impl<'a> CounterStateSeeds<'a> {
    /// The seeds as byte slices, without the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 2usize] {
        [SEED_COUNTER, self.authority.as_ref()]
    }
    /// Append the bump seed to the seeds.
    pub fn with_bump(&self, bump: u8) -> CounterStateSeedsWithBump<'a> {
        CounterStateSeedsWithBump {
            inner: *self,
            _bump: [bump],
        }
    }
}
impl<'a> CounterStateSeedsWithBump<'a> {
    /// The seeds as byte slices, including the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 3usize] {
        [SEED_COUNTER, self.inner.authority.as_ref(), &self._bump]
    }
    /// The seeds as Pinocchio CPI seed values, including the bump seed.
    pub fn as_seed_array(&self) -> [pina::Seed<'_>; 3usize] {
        self.as_slices().map(pina::Seed::from)
    }
    /// The seeds as an owned PDA signer helper.
    pub fn to_signer(&self) -> pina::PdaSigner<'_, 3usize> {
        pina::PdaSigner::from_seed_array(self.as_seed_array())
    }
}
const _: fn(Address) -> pina::Address = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::Address>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<Address>();
    assert_storage::<pina::Address>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::Address>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::Address>() == 1",
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
    assert_storage::<CounterStateZc>();
};
const _: () = {
    if !(::core::mem::align_of::<CounterStateZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<CounterStateZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<CounterStateZc>()
        == PdaDisc::BYTES + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<CounterStateZc>() ==\n    PdaDisc::BYTES + ::core::mem::size_of::<pina::Address>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl CounterState {
    /// The exact number of bytes required by the PinaPod representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return PinaPod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// PinaPod zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, PinaPod zeros the complete slice again.
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for CounterState {
    type Type = PdaDisc;
    const VALUE: Self::Type = PdaDisc::CounterState;
}
impl pina::AccountValidation for CounterStateZc {
    #[track_caller]
    fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        if condition(self) {
            return Ok(self);
        }
        pina::log_caller();
        Err(pina::ProgramError::InvalidAccountData)
    }
    #[track_caller]
    fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        match pina::assert(
            condition(self),
            pina::ProgramError::InvalidAccountData,
            msg,
        ) {
            Err(err) => Err(err),
            Ok(()) => Ok(self),
        }
    }
    #[track_caller]
    fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        if condition(self) {
            return Ok(self);
        }
        pina::log_caller();
        Err(pina::ProgramError::InvalidAccountData)
    }
    #[track_caller]
    fn assert_mut_msg<F>(
        &mut self,
        condition: F,
        msg: &str,
    ) -> Result<&mut Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        match pina::assert(
            condition(self),
            pina::ProgramError::InvalidAccountData,
            msg,
        ) {
            Err(err) => Err(err),
            Ok(()) => Ok(self),
        }
    }
}
impl pina::PinaAccount for CounterState {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct AllSeedState {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; PdaDisc::BYTES],
    pub authority: Address,
    pub amount: PodU64,
    pub side: u8,
    pub tag: [u8; 8],
    pub width: PodU16,
    pub height: PodU32,
    pub bump: u8,
}
#[repr(C)]
pub struct AllSeedStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub authority: <Address as pina::pinapod::ZcField>::Pod,
    pub amount: <PodU64 as pina::pinapod::ZcField>::Pod,
    pub side: <u8 as pina::pinapod::ZcField>::Pod,
    pub tag: <[u8; 8] as pina::pinapod::ZcField>::Pod,
    pub width: <PodU16 as pina::pinapod::ZcField>::Pod,
    pub height: <PodU32 as pina::pinapod::ZcField>::Pod,
    pub bump: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for AllSeedStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for AllSeedStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<AllSeedStateZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<AllSeedStateZc>() == 1",
    )
};
impl AllSeedStateZc {
    #[inline(always)]
    pub fn authority(&self) -> &<Address as pina::pinapod::ZcField>::Pod {
        &self.authority
    }
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.amount
    }
    #[inline(always)]
    pub fn side(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.side
    }
    #[inline(always)]
    pub fn tag(&self) -> &<[u8; 8] as pina::pinapod::ZcField>::Pod {
        &self.tag
    }
    #[inline(always)]
    pub fn width(&self) -> &<PodU16 as pina::pinapod::ZcField>::Pod {
        &self.width
    }
    #[inline(always)]
    pub fn height(&self) -> &<PodU32 as pina::pinapod::ZcField>::Pod {
        &self.height
    }
    #[inline(always)]
    pub fn bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.bump
    }
}
impl pina::pinapod::ZcValidate for AllSeedStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<Address as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.authority,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.side,
        )?;
        <<[u8; 8] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.tag,
        )?;
        <<PodU16 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.width,
        )?;
        <<PodU32 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.height,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.bump,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for AllSeedState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for AllSeedState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = AllSeedStateZc;
}
unsafe impl pina::pinapod::ZcField for AllSeedState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = AllSeedStateZc;
}
unsafe impl pina::pinapod::ZcElem for AllSeedStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 8]: pina::pinapod::ZcField,
    <[u8; 8] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU16: pina::pinapod::ZcField,
    <PodU16 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU32: pina::pinapod::ZcField,
    <PodU32 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
///The PDA seeds for `AllSeedState`.
pub struct AllSeedStateSeeds<'a> {
    ///The `authority` seed.
    ///The `amount` seed.
    ///The `side` seed.
    ///The `tag` seed.
    ///The `width` seed.
    ///The `height` seed.
    pub authority: &'a Address,
    pub amount: [u8; 8],
    pub side: [u8; 1],
    pub tag: [u8; 8usize],
    pub width: [u8; 2],
    pub height: [u8; 4],
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl<'a> ::core::clone::TrivialClone for AllSeedStateSeeds<'a> {}
#[automatically_derived]
impl<'a> ::core::clone::Clone for AllSeedStateSeeds<'a> {
    #[inline]
    fn clone(&self) -> AllSeedStateSeeds<'a> {
        let _: ::core::clone::AssertParamIsClone<&'a Address>;
        let _: ::core::clone::AssertParamIsClone<[u8; 8]>;
        let _: ::core::clone::AssertParamIsClone<[u8; 1]>;
        let _: ::core::clone::AssertParamIsClone<[u8; 8usize]>;
        let _: ::core::clone::AssertParamIsClone<[u8; 2]>;
        let _: ::core::clone::AssertParamIsClone<[u8; 4]>;
        *self
    }
}
#[automatically_derived]
impl<'a> ::core::marker::Copy for AllSeedStateSeeds<'a> {}
///The PDA seeds for `AllSeedState`, including the bump seed.
pub struct AllSeedStateSeedsWithBump<'a> {
    inner: AllSeedStateSeeds<'a>,
    _bump: [u8; 1],
}
impl AllSeedState {
    /// Build the PDA seeds for this account.
    pub fn seeds<'a>(
        authority: &'a Address,
        amount: u64,
        side: u8,
        tag: [u8; 8usize],
        width: u16,
        height: u32,
    ) -> AllSeedStateSeeds<'a> {
        AllSeedStateSeeds {
            authority: authority,
            amount: amount.to_le_bytes(),
            side: [side],
            tag: tag,
            width: width.to_le_bytes(),
            height: height.to_le_bytes(),
        }
    }
    /// Find the canonical PDA for this account and its bump seed.
    pub fn try_find_pda(
        authority: &Address,
        amount: u64,
        side: u8,
        tag: [u8; 8usize],
        width: u16,
        height: u32,
        program_id: &Address,
    ) -> ::core::option::Option<(::pina::Address, u8)> {
        let seeds = Self::seeds(authority, amount, side, tag, width, height);
        ::pina::try_find_program_address(&seeds.as_slices(), program_id)
    }
    /// Find the canonical PDA for this account and its bump seed.
    ///
    /// # Panics
    ///
    /// Panics if no valid PDA exists for the given seeds.
    pub fn find_pda(
        authority: &Address,
        amount: u64,
        side: u8,
        tag: [u8; 8usize],
        width: u16,
        height: u32,
        program_id: &Address,
    ) -> (::pina::Address, u8) {
        Self::try_find_pda(authority, amount, side, tag, width, height, program_id)
            .unwrap_or_else(|| {
                ::core::panicking::panic_fmt(
                    format_args!("could not find program address from seeds"),
                );
            })
    }
    ///Assert that `account` is the PDA for the given seeds, using the stored `bump` field.
    pub fn assert_seeds(
        account: &::pina::AccountView,
        authority: &Address,
        amount: u64,
        side: u8,
        tag: [u8; 8usize],
        width: u16,
        height: u32,
        program_id: &::pina::Address,
    ) -> ::core::result::Result<(), ::pina::ProgramError> {
        let bump = ::pina::AsAccount::as_account::<Self>(account, program_id)?.bump;
        let seeds = Self::seeds(authority, amount, side, tag, width, height)
            .with_bump(bump);
        <&::pina::AccountView as ::pina::AccountInfoValidation>::assert_seeds_with_bump(
                account,
                &seeds.as_slices(),
                program_id,
            )
            .map(|_| ())
    }
}
impl<'a> AllSeedStateSeeds<'a> {
    /// The seeds as byte slices, without the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 7usize] {
        [
            b"test",
            self.authority.as_ref(),
            &self.amount,
            &self.side,
            &self.tag,
            &self.width,
            &self.height,
        ]
    }
    /// Append the bump seed to the seeds.
    pub fn with_bump(&self, bump: u8) -> AllSeedStateSeedsWithBump<'a> {
        AllSeedStateSeedsWithBump {
            inner: *self,
            _bump: [bump],
        }
    }
}
impl<'a> AllSeedStateSeedsWithBump<'a> {
    /// The seeds as byte slices, including the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 8usize] {
        [
            b"test",
            self.inner.authority.as_ref(),
            &self.inner.amount,
            &self.inner.side,
            &self.inner.tag,
            &self.inner.width,
            &self.inner.height,
            &self._bump,
        ]
    }
    /// The seeds as Pinocchio CPI seed values, including the bump seed.
    pub fn as_seed_array(&self) -> [::pina::Seed<'_>; 8usize] {
        self.as_slices().map(::pina::Seed::from)
    }
    /// The seeds as an owned PDA signer helper.
    pub fn to_signer(&self) -> ::pina::PdaSigner<'_, 8usize> {
        ::pina::PdaSigner::from_seed_array(self.as_seed_array())
    }
}
const _: fn(Address) -> pina::Address = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::Address>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<Address>();
    assert_storage::<pina::Address>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::Address>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::Address>() == 1",
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
const _: fn([u8; 8]) -> [::core::primitive::u8; 8] = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 8]>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<[u8; 8]>();
    assert_storage::<[::core::primitive::u8; 8]>();
};
const _: () = {
    if !(::core::mem::align_of::<[::core::primitive::u8; 8]>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 8]>() == 1",
        )
    }
};
const _: fn(PodU16) -> pina::PodU16 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::PodU16>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<PodU16>();
    assert_storage::<pina::PodU16>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::PodU16>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::PodU16>() == 1",
        )
    }
};
const _: fn(PodU32) -> pina::PodU32 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::PodU32>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<PodU32>();
    assert_storage::<pina::PodU32>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::PodU32>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::PodU32>() == 1",
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
    assert_storage::<AllSeedStateZc>();
};
const _: () = {
    if !(::core::mem::align_of::<AllSeedStateZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<AllSeedStateZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<AllSeedStateZc>()
        == PdaDisc::BYTES + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<[::core::primitive::u8; 8]>()
            + ::core::mem::size_of::<pina::PodU16>()
            + ::core::mem::size_of::<pina::PodU32>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<AllSeedStateZc>() ==\n    PdaDisc::BYTES + ::core::mem::size_of::<pina::Address>() +\n                            ::core::mem::size_of::<pina::PodU64>() +\n                        ::core::mem::size_of::<::core::primitive::u8>() +\n                    ::core::mem::size_of::<[::core::primitive::u8; 8]>() +\n                ::core::mem::size_of::<pina::PodU16>() +\n            ::core::mem::size_of::<pina::PodU32>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl AllSeedState {
    /// The exact number of bytes required by the PinaPod representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return PinaPod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// PinaPod zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, PinaPod zeros the complete slice again.
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for AllSeedState {
    type Type = PdaDisc;
    const VALUE: Self::Type = PdaDisc::AllSeedState;
}
impl pina::AccountValidation for AllSeedStateZc {
    #[track_caller]
    fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        if condition(self) {
            return Ok(self);
        }
        pina::log_caller();
        Err(pina::ProgramError::InvalidAccountData)
    }
    #[track_caller]
    fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        match pina::assert(
            condition(self),
            pina::ProgramError::InvalidAccountData,
            msg,
        ) {
            Err(err) => Err(err),
            Ok(()) => Ok(self),
        }
    }
    #[track_caller]
    fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        if condition(self) {
            return Ok(self);
        }
        pina::log_caller();
        Err(pina::ProgramError::InvalidAccountData)
    }
    #[track_caller]
    fn assert_mut_msg<F>(
        &mut self,
        condition: F,
        msg: &str,
    ) -> Result<&mut Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        match pina::assert(
            condition(self),
            pina::ProgramError::InvalidAccountData,
            msg,
        ) {
            Err(err) => Err(err),
            Ok(()) => Ok(self),
        }
    }
}
impl pina::PinaAccount for AllSeedState {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
pub struct VaultState {
    pub user: Address,
}
///The PDA seeds for `VaultState`.
pub struct VaultStateSeeds<'a> {
    ///The `user` seed.
    pub user: &'a Address,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl<'a> ::core::clone::TrivialClone for VaultStateSeeds<'a> {}
#[automatically_derived]
impl<'a> ::core::clone::Clone for VaultStateSeeds<'a> {
    #[inline]
    fn clone(&self) -> VaultStateSeeds<'a> {
        let _: ::core::clone::AssertParamIsClone<&'a Address>;
        *self
    }
}
#[automatically_derived]
impl<'a> ::core::marker::Copy for VaultStateSeeds<'a> {}
///The PDA seeds for `VaultState`, including the bump seed.
pub struct VaultStateSeedsWithBump<'a> {
    inner: VaultStateSeeds<'a>,
    _bump: [u8; 1],
}
impl VaultState {
    /// Build the PDA seeds for this account.
    pub fn seeds<'a>(user: &'a Address) -> VaultStateSeeds<'a> {
        VaultStateSeeds { user: user }
    }
    /// Find the canonical PDA for this account and its bump seed.
    pub fn try_find_pda(
        user: &Address,
        program_id: &Address,
    ) -> ::core::option::Option<(pina::Address, u8)> {
        let seeds = Self::seeds(user);
        pina::try_find_program_address(&seeds.as_slices(), program_id)
    }
    /// Find the canonical PDA for this account and its bump seed.
    ///
    /// # Panics
    ///
    /// Panics if no valid PDA exists for the given seeds.
    pub fn find_pda(user: &Address, program_id: &Address) -> (pina::Address, u8) {
        Self::try_find_pda(user, program_id)
            .unwrap_or_else(|| {
                ::core::panicking::panic_fmt(
                    format_args!("could not find program address from seeds"),
                );
            })
    }
}
impl<'a> VaultStateSeeds<'a> {
    /// The seeds as byte slices, without the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 2usize] {
        [b"vault", self.user.as_ref()]
    }
    /// Append the bump seed to the seeds.
    pub fn with_bump(&self, bump: u8) -> VaultStateSeedsWithBump<'a> {
        VaultStateSeedsWithBump {
            inner: *self,
            _bump: [bump],
        }
    }
}
impl<'a> VaultStateSeedsWithBump<'a> {
    /// The seeds as byte slices, including the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 3usize] {
        [b"vault", self.inner.user.as_ref(), &self._bump]
    }
    /// The seeds as Pinocchio CPI seed values, including the bump seed.
    pub fn as_seed_array(&self) -> [pina::Seed<'_>; 3usize] {
        self.as_slices().map(pina::Seed::from)
    }
    /// The seeds as an owned PDA signer helper.
    pub fn to_signer(&self) -> pina::PdaSigner<'_, 3usize> {
        pina::PdaSigner::from_seed_array(self.as_seed_array())
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct TodoState {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; PdaDisc::BYTES],
    pub owner: Address,
    pub bump: u8,
}
#[repr(C)]
pub struct TodoStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub owner: <Address as pina::pinapod::ZcField>::Pod,
    pub bump: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for TodoStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for TodoStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<TodoStateZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<TodoStateZc>() == 1",
    )
};
impl TodoStateZc {
    #[inline(always)]
    pub fn owner(&self) -> &<Address as pina::pinapod::ZcField>::Pod {
        &self.owner
    }
    #[inline(always)]
    pub fn bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.bump
    }
}
impl pina::pinapod::ZcValidate for TodoStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<Address as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.owner,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.bump,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for TodoState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for TodoState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = TodoStateZc;
}
unsafe impl pina::pinapod::ZcField for TodoState
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = TodoStateZc;
}
unsafe impl pina::pinapod::ZcElem for TodoStateZc
where
    [u8; PdaDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; PdaDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    Address: pina::pinapod::ZcField,
    <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
///The PDA seeds for `TodoState`.
pub struct TodoStateSeeds<'a> {
    ///The `owner` seed.
    pub owner: &'a Address,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl<'a> ::core::clone::TrivialClone for TodoStateSeeds<'a> {}
#[automatically_derived]
impl<'a> ::core::clone::Clone for TodoStateSeeds<'a> {
    #[inline]
    fn clone(&self) -> TodoStateSeeds<'a> {
        let _: ::core::clone::AssertParamIsClone<&'a Address>;
        *self
    }
}
#[automatically_derived]
impl<'a> ::core::marker::Copy for TodoStateSeeds<'a> {}
///The PDA seeds for `TodoState`, including the bump seed.
pub struct TodoStateSeedsWithBump<'a> {
    inner: TodoStateSeeds<'a>,
    _bump: [u8; 1],
}
impl TodoState {
    /// Build the PDA seeds for this account.
    pub fn seeds<'a>(owner: &'a Address) -> TodoStateSeeds<'a> {
        TodoStateSeeds { owner: owner }
    }
    /// Find the canonical PDA for this account and its bump seed.
    pub fn try_find_pda(
        owner: &Address,
        program_id: &Address,
    ) -> ::core::option::Option<(::pina::Address, u8)> {
        let seeds = Self::seeds(owner);
        ::pina::try_find_program_address(&seeds.as_slices(), program_id)
    }
    /// Find the canonical PDA for this account and its bump seed.
    ///
    /// # Panics
    ///
    /// Panics if no valid PDA exists for the given seeds.
    pub fn find_pda(owner: &Address, program_id: &Address) -> (::pina::Address, u8) {
        Self::try_find_pda(owner, program_id)
            .unwrap_or_else(|| {
                ::core::panicking::panic_fmt(
                    format_args!("could not find program address from seeds"),
                );
            })
    }
    ///Assert that `account` is the PDA for the given seeds, using the stored `bump` field.
    pub fn assert_seeds(
        account: &::pina::AccountView,
        owner: &Address,
        program_id: &::pina::Address,
    ) -> ::core::result::Result<(), ::pina::ProgramError> {
        let bump = ::pina::AsAccount::as_account::<Self>(account, program_id)?.bump;
        let seeds = Self::seeds(owner).with_bump(bump);
        <&::pina::AccountView as ::pina::AccountInfoValidation>::assert_seeds_with_bump(
                account,
                &seeds.as_slices(),
                program_id,
            )
            .map(|_| ())
    }
}
impl<'a> TodoStateSeeds<'a> {
    /// The seeds as byte slices, without the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 2usize] {
        [b"todo", self.owner.as_ref()]
    }
    /// Append the bump seed to the seeds.
    pub fn with_bump(&self, bump: u8) -> TodoStateSeedsWithBump<'a> {
        TodoStateSeedsWithBump {
            inner: *self,
            _bump: [bump],
        }
    }
}
impl<'a> TodoStateSeedsWithBump<'a> {
    /// The seeds as byte slices, including the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 3usize] {
        [b"todo", self.inner.owner.as_ref(), &self._bump]
    }
    /// The seeds as Pinocchio CPI seed values, including the bump seed.
    pub fn as_seed_array(&self) -> [::pina::Seed<'_>; 3usize] {
        self.as_slices().map(::pina::Seed::from)
    }
    /// The seeds as an owned PDA signer helper.
    pub fn to_signer(&self) -> ::pina::PdaSigner<'_, 3usize> {
        ::pina::PdaSigner::from_seed_array(self.as_seed_array())
    }
}
const _: fn(Address) -> pina::Address = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::Address>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<Address>();
    assert_storage::<pina::Address>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::Address>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::Address>() == 1",
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
    assert_storage::<TodoStateZc>();
};
const _: () = {
    if !(::core::mem::align_of::<TodoStateZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<TodoStateZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<TodoStateZc>()
        == PdaDisc::BYTES + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<TodoStateZc>() ==\n    PdaDisc::BYTES + ::core::mem::size_of::<pina::Address>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl TodoState {
    /// The exact number of bytes required by the PinaPod representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return PinaPod's immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// PinaPod zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// validation fails, PinaPod zeros the complete slice again.
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for TodoState {
    type Type = PdaDisc;
    const VALUE: Self::Type = PdaDisc::TodoState;
}
impl pina::AccountValidation for TodoStateZc {
    #[track_caller]
    fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        if condition(self) {
            return Ok(self);
        }
        pina::log_caller();
        Err(pina::ProgramError::InvalidAccountData)
    }
    #[track_caller]
    fn assert_msg<F>(&self, condition: F, msg: &str) -> Result<&Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        match pina::assert(
            condition(self),
            pina::ProgramError::InvalidAccountData,
            msg,
        ) {
            Err(err) => Err(err),
            Ok(()) => Ok(self),
        }
    }
    #[track_caller]
    fn assert_mut<F>(&mut self, condition: F) -> Result<&mut Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        if condition(self) {
            return Ok(self);
        }
        pina::log_caller();
        Err(pina::ProgramError::InvalidAccountData)
    }
    #[track_caller]
    fn assert_mut_msg<F>(
        &mut self,
        condition: F,
        msg: &str,
    ) -> Result<&mut Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        match pina::assert(
            condition(self),
            pina::ProgramError::InvalidAccountData,
            msg,
        ) {
            Err(err) => Err(err),
            Ok(()) => Ok(self),
        }
    }
}
impl pina::PinaAccount for TodoState {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
pub struct AuthorityState {}
///The PDA seeds for `AuthorityState`.
pub struct AuthorityStateSeeds<'a> {
    _marker: ::core::marker::PhantomData<&'a ()>,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl<'a> ::core::clone::TrivialClone for AuthorityStateSeeds<'a> {}
#[automatically_derived]
impl<'a> ::core::clone::Clone for AuthorityStateSeeds<'a> {
    #[inline]
    fn clone(&self) -> AuthorityStateSeeds<'a> {
        let _: ::core::clone::AssertParamIsClone<::core::marker::PhantomData<&'a ()>>;
        *self
    }
}
#[automatically_derived]
impl<'a> ::core::marker::Copy for AuthorityStateSeeds<'a> {}
///The PDA seeds for `AuthorityState`, including the bump seed.
pub struct AuthorityStateSeedsWithBump<'a> {
    inner: AuthorityStateSeeds<'a>,
    _bump: [u8; 1],
}
impl AuthorityState {
    /// Build the PDA seeds for this account.
    pub fn seeds<'a>() -> AuthorityStateSeeds<'a> {
        AuthorityStateSeeds {
            _marker: ::core::marker::PhantomData,
        }
    }
    /// Find the canonical PDA for this account and its bump seed.
    pub fn try_find_pda(
        program_id: &Address,
    ) -> ::core::option::Option<(pina::Address, u8)> {
        let seeds = Self::seeds();
        pina::try_find_program_address(&seeds.as_slices(), program_id)
    }
    /// Find the canonical PDA for this account and its bump seed.
    ///
    /// # Panics
    ///
    /// Panics if no valid PDA exists for the given seeds.
    pub fn find_pda(program_id: &Address) -> (pina::Address, u8) {
        Self::try_find_pda(program_id)
            .unwrap_or_else(|| {
                ::core::panicking::panic_fmt(
                    format_args!("could not find program address from seeds"),
                );
            })
    }
}
impl<'a> AuthorityStateSeeds<'a> {
    /// The seeds as byte slices, without the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 1usize] {
        [b"authority"]
    }
    /// Append the bump seed to the seeds.
    pub fn with_bump(&self, bump: u8) -> AuthorityStateSeedsWithBump<'a> {
        AuthorityStateSeedsWithBump {
            inner: *self,
            _bump: [bump],
        }
    }
}
impl<'a> AuthorityStateSeedsWithBump<'a> {
    /// The seeds as byte slices, including the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 2usize] {
        [b"authority", &self._bump]
    }
    /// The seeds as Pinocchio CPI seed values, including the bump seed.
    pub fn as_seed_array(&self) -> [pina::Seed<'_>; 2usize] {
        self.as_slices().map(pina::Seed::from)
    }
    /// The seeds as an owned PDA signer helper.
    pub fn to_signer(&self) -> pina::PdaSigner<'_, 2usize> {
        pina::PdaSigner::from_seed_array(self.as_seed_array())
    }
}
pub struct NumericState {
    pub nonce: u64,
    pub tag: [u8; 8],
}
///The PDA seeds for `NumericState`.
pub struct NumericStateSeeds<'a> {
    ///The `nonce` seed.
    ///The `tag` seed.
    pub nonce: [u8; 8],
    pub tag: [u8; 8usize],
    _marker: ::core::marker::PhantomData<&'a ()>,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl<'a> ::core::clone::TrivialClone for NumericStateSeeds<'a> {}
#[automatically_derived]
impl<'a> ::core::clone::Clone for NumericStateSeeds<'a> {
    #[inline]
    fn clone(&self) -> NumericStateSeeds<'a> {
        let _: ::core::clone::AssertParamIsClone<[u8; 8]>;
        let _: ::core::clone::AssertParamIsClone<[u8; 8usize]>;
        let _: ::core::clone::AssertParamIsClone<::core::marker::PhantomData<&'a ()>>;
        *self
    }
}
#[automatically_derived]
impl<'a> ::core::marker::Copy for NumericStateSeeds<'a> {}
///The PDA seeds for `NumericState`, including the bump seed.
pub struct NumericStateSeedsWithBump<'a> {
    inner: NumericStateSeeds<'a>,
    _bump: [u8; 1],
}
impl NumericState {
    /// Build the PDA seeds for this account.
    pub fn seeds<'a>(nonce: u64, tag: [u8; 8usize]) -> NumericStateSeeds<'a> {
        NumericStateSeeds {
            nonce: nonce.to_le_bytes(),
            tag: tag,
            _marker: ::core::marker::PhantomData,
        }
    }
    /// Find the canonical PDA for this account and its bump seed.
    pub fn try_find_pda(
        nonce: u64,
        tag: [u8; 8usize],
        program_id: &Address,
    ) -> ::core::option::Option<(pina::Address, u8)> {
        let seeds = Self::seeds(nonce, tag);
        pina::try_find_program_address(&seeds.as_slices(), program_id)
    }
    /// Find the canonical PDA for this account and its bump seed.
    ///
    /// # Panics
    ///
    /// Panics if no valid PDA exists for the given seeds.
    pub fn find_pda(
        nonce: u64,
        tag: [u8; 8usize],
        program_id: &Address,
    ) -> (pina::Address, u8) {
        Self::try_find_pda(nonce, tag, program_id)
            .unwrap_or_else(|| {
                ::core::panicking::panic_fmt(
                    format_args!("could not find program address from seeds"),
                );
            })
    }
}
impl<'a> NumericStateSeeds<'a> {
    /// The seeds as byte slices, without the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 3usize] {
        [b"numeric", &self.nonce, &self.tag]
    }
    /// Append the bump seed to the seeds.
    pub fn with_bump(&self, bump: u8) -> NumericStateSeedsWithBump<'a> {
        NumericStateSeedsWithBump {
            inner: *self,
            _bump: [bump],
        }
    }
}
impl<'a> NumericStateSeedsWithBump<'a> {
    /// The seeds as byte slices, including the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 4usize] {
        [b"numeric", &self.inner.nonce, &self.inner.tag, &self._bump]
    }
    /// The seeds as Pinocchio CPI seed values, including the bump seed.
    pub fn as_seed_array(&self) -> [pina::Seed<'_>; 4usize] {
        self.as_slices().map(pina::Seed::from)
    }
    /// The seeds as an owned PDA signer helper.
    pub fn to_signer(&self) -> pina::PdaSigner<'_, 4usize> {
        pina::PdaSigner::from_seed_array(self.as_seed_array())
    }
}
