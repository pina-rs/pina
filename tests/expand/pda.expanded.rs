use pina::*;
const SEED_COUNTER: &[u8] = b"counter";
#[repr(u8)]
#[non_exhaustive]
pub enum PdaDisc {
    CounterState = 1,
    AllSeedState = 2,
    TodoState = 3,
    Compact = 4,
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
                PdaDisc::Compact => "Compact",
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
        const __COMPACT: u8 = 4;
        #[deny(unreachable_patterns)]
        match number {
            __COUNTER_STATE => ::core::result::Result::Ok(Self::CounterState),
            __ALL_SEED_STATE => ::core::result::Result::Ok(Self::AllSeedState),
            __TODO_STATE => ::core::result::Result::Ok(Self::TodoState),
            __COMPACT => ::core::result::Result::Ok(Self::Compact),
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
    ///Load and validate `CounterState` and its stored-bump PDA address in one pass.
    #[inline(always)]
    pub fn load_pda<'account>(
        account: &'account pina::AccountView,
        authority: &Address,
        program_id: &pina::Address,
    ) -> ::core::result::Result<
        pina::Ref<'account, <Self as pina::PinaPodFixed>::Zc>,
        pina::ProgramError,
    > {
        let account_address = *account.address();
        let state = pina::AsAccount::as_account::<Self>(account, program_id)?;
        let seeds = Self::seeds(authority).with_bump(state.bump);
        let expected_address = pina::create_program_address(
            &seeds.as_slices(),
            program_id,
        )?;
        if account_address != expected_address {
            return Err(pina::ProgramError::InvalidSeeds);
        }
        Ok(state)
    }
    ///Mutably load and validate `CounterState` and its stored-bump PDA address in one pass.
    #[inline(always)]
    pub fn load_pda_mut<'account>(
        account: &'account mut pina::AccountView,
        authority: &Address,
        program_id: &pina::Address,
    ) -> ::core::result::Result<
        pina::RefMut<'account, <Self as pina::PinaPodFixed>::Zc>,
        pina::ProgramError,
    > {
        let account_address = *account.address();
        let state = pina::AsAccount::as_account_mut::<Self>(account, program_id)?;
        let seeds = Self::seeds(authority).with_bump(state.bump);
        let expected_address = pina::create_program_address(
            &seeds.as_slices(),
            program_id,
        )?;
        if account_address != expected_address {
            return Err(pina::ProgramError::InvalidSeeds);
        }
        Ok(state)
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
        == PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<CounterStateZc>() ==\n    PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl CounterState {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        let value = <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        <<Self as pina::PinaPodFixed>::Zc as pina::PinaValidate>::validate(value)?;
        Ok(value)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// structural or application validation fails, the complete slice is
    /// zeroed again. Application validation returns its declared `ProgramError`.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or structural validation rejects the representation.
    /// Application validation returns its declared `ProgramError`.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        let value = <Self as pina::PinaAccount>::initialize(data, initialize)?;
        Ok(value)
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
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
impl pina::PinaValidate for CounterStateZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for CounterState {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <CounterStateZc as pina::PinaValidate>::validate(value)
    }
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
    ///Load and validate `AllSeedState` and its stored-bump PDA address in one pass.
    #[inline(always)]
    pub fn load_pda<'account>(
        account: &'account ::pina::AccountView,
        authority: &Address,
        amount: u64,
        side: u8,
        tag: [u8; 8usize],
        width: u16,
        height: u32,
        program_id: &::pina::Address,
    ) -> ::core::result::Result<
        ::pina::Ref<'account, <Self as ::pina::PinaPodFixed>::Zc>,
        ::pina::ProgramError,
    > {
        let account_address = *account.address();
        let state = ::pina::AsAccount::as_account::<Self>(account, program_id)?;
        let seeds = Self::seeds(authority, amount, side, tag, width, height)
            .with_bump(state.bump);
        let expected_address = ::pina::create_program_address(
            &seeds.as_slices(),
            program_id,
        )?;
        if account_address != expected_address {
            return Err(::pina::ProgramError::InvalidSeeds);
        }
        Ok(state)
    }
    ///Mutably load and validate `AllSeedState` and its stored-bump PDA address in one pass.
    #[inline(always)]
    pub fn load_pda_mut<'account>(
        account: &'account mut ::pina::AccountView,
        authority: &Address,
        amount: u64,
        side: u8,
        tag: [u8; 8usize],
        width: u16,
        height: u32,
        program_id: &::pina::Address,
    ) -> ::core::result::Result<
        ::pina::RefMut<'account, <Self as ::pina::PinaPodFixed>::Zc>,
        ::pina::ProgramError,
    > {
        let account_address = *account.address();
        let state = ::pina::AsAccount::as_account_mut::<Self>(account, program_id)?;
        let seeds = Self::seeds(authority, amount, side, tag, width, height)
            .with_bump(state.bump);
        let expected_address = ::pina::create_program_address(
            &seeds.as_slices(),
            program_id,
        )?;
        if account_address != expected_address {
            return Err(::pina::ProgramError::InvalidSeeds);
        }
        Ok(state)
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
        == PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<[::core::primitive::u8; 8]>()
            + ::core::mem::size_of::<pina::PodU16>()
            + ::core::mem::size_of::<pina::PodU32>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<AllSeedStateZc>() ==\n    PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>() +\n                            ::core::mem::size_of::<pina::PodU64>() +\n                        ::core::mem::size_of::<::core::primitive::u8>() +\n                    ::core::mem::size_of::<[::core::primitive::u8; 8]>() +\n                ::core::mem::size_of::<pina::PodU16>() +\n            ::core::mem::size_of::<pina::PodU32>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl AllSeedState {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        let value = <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        <<Self as pina::PinaPodFixed>::Zc as pina::PinaValidate>::validate(value)?;
        Ok(value)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// structural or application validation fails, the complete slice is
    /// zeroed again. Application validation returns its declared `ProgramError`.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or structural validation rejects the representation.
    /// Application validation returns its declared `ProgramError`.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        let value = <Self as pina::PinaAccount>::initialize(data, initialize)?;
        Ok(value)
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
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
impl pina::PinaValidate for AllSeedStateZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for AllSeedState {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <AllSeedStateZc as pina::PinaValidate>::validate(value)
    }
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
    ///Load and validate `TodoState` and its stored-bump PDA address in one pass.
    #[inline(always)]
    pub fn load_pda<'account>(
        account: &'account ::pina::AccountView,
        owner: &Address,
        program_id: &::pina::Address,
    ) -> ::core::result::Result<
        ::pina::Ref<'account, <Self as ::pina::PinaPodFixed>::Zc>,
        ::pina::ProgramError,
    > {
        let account_address = *account.address();
        let state = ::pina::AsAccount::as_account::<Self>(account, program_id)?;
        let seeds = Self::seeds(owner).with_bump(state.bump);
        let expected_address = ::pina::create_program_address(
            &seeds.as_slices(),
            program_id,
        )?;
        if account_address != expected_address {
            return Err(::pina::ProgramError::InvalidSeeds);
        }
        Ok(state)
    }
    ///Mutably load and validate `TodoState` and its stored-bump PDA address in one pass.
    #[inline(always)]
    pub fn load_pda_mut<'account>(
        account: &'account mut ::pina::AccountView,
        owner: &Address,
        program_id: &::pina::Address,
    ) -> ::core::result::Result<
        ::pina::RefMut<'account, <Self as ::pina::PinaPodFixed>::Zc>,
        ::pina::ProgramError,
    > {
        let account_address = *account.address();
        let state = ::pina::AsAccount::as_account_mut::<Self>(account, program_id)?;
        let seeds = Self::seeds(owner).with_bump(state.bump);
        let expected_address = ::pina::create_program_address(
            &seeds.as_slices(),
            program_id,
        )?;
        if account_address != expected_address {
            return Err(::pina::ProgramError::InvalidSeeds);
        }
        Ok(state)
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
        == PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<TodoStateZc>() ==\n    PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl TodoState {
    /// The exact number of bytes required by the `PinaPod` representation.
    pub const SIZE: usize = ::core::mem::size_of::<<Self as pina::PinaPodFixed>::Zc>();
    /// Validate `data` and return `PinaPod`'s immutable zero-copy companion.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<&<Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        if data.len() != Self::SIZE
            || !<Self as pina::HasDiscriminator>::matches_discriminator(data)
        {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        let value = <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        <<Self as pina::PinaPodFixed>::Zc as pina::PinaValidate>::validate(value)?;
        Ok(value)
    }
    /// Initialize caller-owned storage with a complete typed configuration.
    ///
    /// `PinaPod` zeros the complete slice before calling `initialize`, then
    /// validates the finished representation once. The discriminator is written
    /// before the caller configures the remaining fields. If the closure or final
    /// structural or application validation fails, the complete slice is
    /// zeroed again. Application validation returns its declared `ProgramError`.
    ///
    /// # Errors
    ///
    /// Returns the generated invalid-data error when `data` has the wrong length,
    /// the closure fails, or structural validation rejects the representation.
    /// Application validation returns its declared `ProgramError`.
    pub fn initialize<'data>(
        data: &'data mut [u8],
        initialize: impl FnOnce(
            &mut <Self as pina::PinaPodFixed>::Zc,
        ) -> Result<(), pina::PinaPodError>,
    ) -> Result<&'data mut <Self as pina::PinaPodFixed>::Zc, pina::ProgramError> {
        let value = <Self as pina::PinaAccount>::initialize(data, initialize)?;
        Ok(value)
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
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
impl pina::PinaValidate for TodoStateZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for TodoState {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <TodoStateZc as pina::PinaValidate>::validate(value)
    }
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
#[pinapod(compact)]
pub struct CompactState {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; PdaDisc::BYTES],
    pub authority: Address,
    pub bump: u8,
    pub values: Vec<u64, 4>,
}
#[doc(hidden)]
#[allow(dead_code, non_snake_case, unused_imports)]
mod __pinapod_compact_CompactState {
    use super::*;
    #[inline(always)]
    fn __pinapod_checked_add(
        left: usize,
        right: usize,
    ) -> Result<usize, pina::pinapod::PinaPodError> {
        left.checked_add(right).ok_or(pina::pinapod::PinaPodError::Overflow)
    }
    #[inline(always)]
    fn __pinapod_checked_mul(
        left: usize,
        right: usize,
    ) -> Result<usize, pina::pinapod::PinaPodError> {
        left.checked_mul(right).ok_or(pina::pinapod::PinaPodError::Overflow)
    }
    const fn __pinapod_gcd(mut left: usize, mut right: usize) -> usize {
        while right != 0 {
            let remainder = left % right;
            left = right;
            right = remainder;
        }
        left
    }
    #[inline(always)]
    fn __pinapod_prefix_max(width: usize) -> Option<usize> {
        match width {
            1 => Some(u8::MAX as usize),
            2 => Some(u16::MAX as usize),
            4 => usize::try_from(u32::MAX).ok(),
            8 => Some(usize::MAX),
            _ => None,
        }
    }
    #[inline(always)]
    fn __pinapod_check_prefix(
        value: usize,
        width: usize,
    ) -> Result<(), pina::pinapod::PinaPodError> {
        match __pinapod_prefix_max(width) {
            Some(max) if value <= max => Ok(()),
            _ => Err(pina::pinapod::PinaPodError::Overflow),
        }
    }
    #[inline(always)]
    fn __pinapod_decode_prefix(
        bytes: &[u8],
    ) -> Result<usize, pina::pinapod::PinaPodError> {
        let value = match bytes {
            [a] => u64::from(*a),
            [a, b] => u64::from(u16::from_le_bytes([*a, *b])),
            [a, b, c, d] => u64::from(u32::from_le_bytes([*a, *b, *c, *d])),
            [a, b, c, d, e, f, g, h] => {
                u64::from_le_bytes([*a, *b, *c, *d, *e, *f, *g, *h])
            }
            _ => return Err(pina::pinapod::PinaPodError::InvalidLength),
        };
        usize::try_from(value).map_err(|_| pina::pinapod::PinaPodError::InvalidLength)
    }
    #[inline(always)]
    fn __pinapod_read_prefix(
        data: &[u8],
        offset: usize,
        width: usize,
    ) -> Result<usize, pina::pinapod::PinaPodError> {
        let end = __pinapod_checked_add(offset, width)?;
        let bytes = data
            .get(offset..end)
            .ok_or(pina::pinapod::PinaPodError::BufferTooSmall)?;
        __pinapod_decode_prefix(bytes)
    }
    #[inline(always)]
    fn __pinapod_write_prefix(
        data: &mut [u8],
        offset: usize,
        width: usize,
        value: usize,
    ) -> Result<(), pina::pinapod::PinaPodError> {
        __pinapod_check_prefix(value, width)?;
        let end = __pinapod_checked_add(offset, width)?;
        let destination = data
            .get_mut(offset..end)
            .ok_or(pina::pinapod::PinaPodError::BufferTooSmall)?;
        let bytes = (value as u64).to_le_bytes();
        destination.copy_from_slice(&bytes[..width]);
        Ok(())
    }
    #[repr(C)]
    pub struct CompactStateHeader
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        discriminator: [u8; PdaDisc::BYTES],
        pub authority: <Address as pina::pinapod::ZcField>::Pod,
        pub bump: <u8 as pina::pinapod::ZcField>::Pod,
        __values_len: [u8; 2usize],
    }
    const _: () = if !(core::mem::align_of::<CompactStateHeader>() == 1) {
        ::core::panicking::panic(
            "assertion failed: core::mem::align_of::<CompactStateHeader>() == 1",
        )
    };
    impl Copy for CompactStateHeader
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {}
    impl Clone for CompactStateHeader
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        fn clone(&self) -> Self {
            *self
        }
    }
    impl pina::pinapod::ZcValidate for CompactStateHeader
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        fn validate_ref(value: &Self) -> Result<(), pina::pinapod::PinaPodError> {
            <[u8; PdaDisc::BYTES] as pina::pinapod::ZcValidate>::validate_ref(
                &value.discriminator,
            )?;
            <<Address as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.authority,
            )?;
            <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.bump,
            )?;
            Ok(())
        }
    }
    unsafe impl pina::pinapod::ZcElem for CompactStateHeader
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {}
    impl pina::pinapod::PinaPod for CompactState
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {}
    unsafe impl pina::pinapod::PinaPodCompact for CompactState
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        type Header = CompactStateHeader;
        const MIN_SIZE: usize = core::mem::size_of::<CompactStateHeader>();
        const MAX_SIZE: usize = {
            let mut __size = core::mem::size_of::<CompactStateHeader>();
            __size = match __size
                .checked_add(
                    match (4 as usize)
                        .checked_mul(
                            core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
                        )
                    {
                        Some(value) => value,
                        None => {
                            ::core::panicking::panic_fmt(
                                format_args!("compact schema maximum size overflows usize"),
                            );
                        }
                    },
                )
            {
                Some(value) => value,
                None => {
                    ::core::panicking::panic_fmt(
                        format_args!("compact schema maximum size overflows usize"),
                    );
                }
            };
            __size
        };
        const TAIL_ALIGNMENT: usize = {
            let mut __alignment = 0usize;
            __alignment = __pinapod_gcd(
                __alignment,
                core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
            );
            if __alignment == 0 { 1 } else { __alignment }
        };
        const HEADER_SIZE: usize = core::mem::size_of::<CompactStateHeader>();
        fn validate(data: &[u8]) -> Result<(), pina::pinapod::PinaPodError> {
            let __pinapod_type_check: fn(
                Vec<u64, 4>,
            ) -> pina::pinapod::pod::PodVecRepr<
                <u64 as pina::pinapod::ZcField>::Pod,
                4,
                2usize,
            > = |value| value;
            let _ = __pinapod_type_check;
            let _ = pina::pinapod::pod::PodVec::<u8, 4, 2usize>::VALID;
            let _ = const {
                if !(core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>() != 0) {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "compact vector elements must not be zero-sized",
                            ),
                        );
                    }
                }
            };
            Self::validate_storage_len(data.len())?;
            if data.len() < core::mem::size_of::<CompactStateHeader>() {
                return Err(pina::pinapod::PinaPodError::BufferTooSmall);
            }
            let __hdr = unsafe { &*(data.as_ptr() as *const CompactStateHeader) };
            <CompactStateHeader as pina::pinapod::ZcValidate>::validate_ref(__hdr)?;
            let mut __tail_offset = core::mem::size_of::<CompactStateHeader>();
            let __values_len = __pinapod_decode_prefix(&__hdr.__values_len)?;
            if __values_len > 4 {
                return Err(pina::pinapod::PinaPodError::InvalidLength);
            }
            let __elem_size = core::mem::size_of::<
                <u64 as pina::pinapod::ZcField>::Pod,
            >();
            if __elem_size == 0 {
                return Err(pina::pinapod::PinaPodError::InvalidLength);
            }
            let __byte_len = __pinapod_checked_mul(__values_len, __elem_size)?;
            let __tail_end = __pinapod_checked_add(__tail_offset, __byte_len)?;
            let __tail = data
                .get(__tail_offset..__tail_end)
                .ok_or(pina::pinapod::PinaPodError::BufferTooSmall)?;
            for __chunk in __tail.chunks_exact(__elem_size) {
                let __elem = unsafe {
                    &*(__chunk.as_ptr() as *const <u64 as pina::pinapod::ZcField>::Pod)
                };
                <<u64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                    __elem,
                )?;
            }
            __tail_offset = __tail_end;
            Ok(())
        }
    }
    pub struct CompactStateRef<'__pinapod_data>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        data: &'__pinapod_data [u8],
        encoded_len: usize,
    }
    impl<'__pinapod_data> core::ops::Deref for CompactStateRef<'__pinapod_data>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        type Target = CompactStateHeader;
        fn deref(&self) -> &CompactStateHeader {
            self.header()
        }
    }
    impl<'__pinapod_data> CompactStateRef<'__pinapod_data>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        pub fn new(
            data: &'__pinapod_data [u8],
        ) -> Result<Self, pina::pinapod::PinaPodError> {
            <CompactState as pina::pinapod::PinaPodCompact>::validate(data)?;
            let mut value = Self { data, encoded_len: 0 };
            value.encoded_len = value.current_encoded_len();
            Ok(value)
        }
        fn header(&self) -> &'__pinapod_data CompactStateHeader {
            unsafe { &*(self.data.as_ptr() as *const CompactStateHeader) }
        }
        fn current_encoded_len(&self) -> usize {
            let __hdr = self.header();
            let mut __offset = core::mem::size_of::<CompactStateHeader>();
            let __values_offset_count = u16::from_le_bytes(__hdr.__values_len) as usize;
            __offset
                += __values_offset_count
                    * core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>();
            __offset
        }
        pub fn encoded_len(&self) -> usize {
            self.encoded_len
        }
        pub fn storage_len(&self) -> usize {
            self.data.len()
        }
        pub fn spare_capacity(&self) -> usize {
            self.data.len() - self.encoded_len
        }
        pub fn values(&self) -> &'__pinapod_data [<u64 as pina::pinapod::ZcField>::Pod] {
            let __hdr = self.header();
            let __count = u16::from_le_bytes(__hdr.__values_len) as usize;
            let mut __offset = core::mem::size_of::<CompactStateHeader>();
            unsafe {
                let __ptr = self.data.as_ptr().add(__offset)
                    as *const <u64 as pina::pinapod::ZcField>::Pod;
                core::slice::from_raw_parts(__ptr, __count)
            }
        }
    }
    struct CompactStateMut<'__pinapod_data>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        data: &'__pinapod_data mut [u8],
        total_len: usize,
        __values_edit: Option<(*const u8, usize)>,
    }
    impl<'__pinapod_data> core::ops::Deref for CompactStateMut<'__pinapod_data>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        type Target = CompactStateHeader;
        fn deref(&self) -> &CompactStateHeader {
            self.header()
        }
    }
    impl<'__pinapod_data> CompactStateMut<'__pinapod_data>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        pub fn new(
            data: &'__pinapod_data mut [u8],
        ) -> Result<Self, pina::pinapod::PinaPodError> {
            <CompactState as pina::pinapod::PinaPodCompact>::validate(data)?;
            let mut value = Self {
                data,
                total_len: 0,
                __values_edit: None,
            };
            value.total_len = value.current_encoded_len();
            Ok(value)
        }
        /// # Safety
        /// Caller must ensure `data` is at least `HEADER_SIZE` bytes and
        /// contains a valid compact header. The tail region must be
        /// consistent with the header length prefixes.
        unsafe fn new_unchecked(data: &'__pinapod_data mut [u8]) -> Self {
            let mut value = Self {
                data,
                total_len: 0,
                __values_edit: None,
            };
            value.total_len = value.current_encoded_len();
            value
        }
        fn header(&self) -> &CompactStateHeader {
            unsafe { &*(self.data.as_ptr() as *const CompactStateHeader) }
        }
        fn header_mut(&mut self) -> &mut CompactStateHeader {
            unsafe { &mut *(self.data.as_mut_ptr() as *mut CompactStateHeader) }
        }
        pub fn discriminator_mut(&mut self) -> &mut [u8; PdaDisc::BYTES] {
            &mut self.header_mut().discriminator
        }
        pub fn authority_mut(
            &mut self,
        ) -> &mut <Address as pina::pinapod::ZcField>::Pod {
            &mut self.header_mut().authority
        }
        pub fn bump_mut(&mut self) -> &mut <u8 as pina::pinapod::ZcField>::Pod {
            &mut self.header_mut().bump
        }
        pub fn set_values(
            &mut self,
            value: &'__pinapod_data [<u64 as pina::pinapod::ZcField>::Pod],
        ) -> Result<(), pina::pinapod::PinaPodError> {
            if value.len() > 4 || __pinapod_check_prefix(value.len(), 2usize).is_err() {
                return Err(pina::pinapod::PinaPodError::Overflow);
            }
            for __item in value {
                <<u64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                    __item,
                )?;
            }
            self.__values_edit = Some((value.as_ptr() as *const u8, value.len()));
            Ok(())
        }
        fn current_encoded_len(&self) -> usize {
            let __hdr = self.header();
            let mut __offset = core::mem::size_of::<CompactStateHeader>();
            let __values_offset_count = u16::from_le_bytes(__hdr.__values_len) as usize;
            __offset
                += __values_offset_count
                    * core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>();
            __offset
        }
        fn try_projected_size(&self) -> Result<usize, pina::pinapod::PinaPodError> {
            let mut __total = self.total_len;
            if let Some((_, __new_count)) = self.__values_edit {
                let __hdr = self.header();
                let __old_count = u16::from_le_bytes(__hdr.__values_len) as usize;
                let __old_len = __pinapod_checked_mul(
                    __old_count,
                    core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
                )?;
                let __new_len = __pinapod_checked_mul(
                    __new_count,
                    core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
                )?;
                __total = __total
                    .checked_sub(__old_len)
                    .ok_or(pina::pinapod::PinaPodError::Overflow)?;
                __total = __pinapod_checked_add(__total, __new_len)?;
            }
            Ok(__total)
        }
        pub fn projected_size(&self) -> usize {
            self.try_projected_size().unwrap_or(usize::MAX)
        }
        pub fn commit(&mut self) -> Result<usize, pina::pinapod::PinaPodError> {
            let __old_off_values: usize = core::mem::size_of::<CompactStateHeader>();
            let __new_off_values: usize = core::mem::size_of::<CompactStateHeader>();
            let __old_len_values: usize = {
                let __hdr = self.header();
                let __count = u16::from_le_bytes(__hdr.__values_len) as usize;
                __pinapod_checked_mul(
                    __count,
                    core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
                )?
            };
            let __new_len_values: usize = match self.__values_edit {
                Some((_, __count)) => {
                    __pinapod_checked_mul(
                        __count,
                        core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
                    )?
                }
                None => __old_len_values,
            };
            let __old_end = __pinapod_checked_add(__old_off_values, __old_len_values)?;
            if __old_end > self.total_len {
                return Err(pina::pinapod::PinaPodError::BufferTooSmall);
            }
            let __new_end = __pinapod_checked_add(__new_off_values, __new_len_values)?;
            if __new_end > self.data.len() {
                return Err(pina::pinapod::PinaPodError::BufferTooSmall);
            }
            let __old_total = self.total_len;
            let __final_total: usize = __pinapod_checked_add(
                __new_off_values,
                __new_len_values,
            )?;
            if __final_total > self.data.len() {
                return Err(pina::pinapod::PinaPodError::BufferTooSmall);
            }
            let __buf_ptr = self.data.as_mut_ptr();
            if self.__values_edit.is_none() && __new_off_values < __old_off_values
                && __old_len_values > 0
            {
                unsafe {
                    core::ptr::copy(
                        __buf_ptr.add(__old_off_values) as *const u8,
                        __buf_ptr.add(__new_off_values),
                        __old_len_values,
                    );
                }
            }
            if self.__values_edit.is_none() && __new_off_values > __old_off_values
                && __old_len_values > 0
            {
                unsafe {
                    core::ptr::copy(
                        __buf_ptr.add(__old_off_values) as *const u8,
                        __buf_ptr.add(__new_off_values),
                        __old_len_values,
                    );
                }
            }
            if let Some((__src_ptr, _)) = self.__values_edit {
                if __new_len_values > 0 {
                    let __source = unsafe {
                        core::slice::from_raw_parts(__src_ptr, __new_len_values)
                    };
                    let __end = __pinapod_checked_add(
                        __new_off_values,
                        __new_len_values,
                    )?;
                    self.data[__new_off_values..__end].copy_from_slice(__source);
                }
            }
            if let Some((_, __count)) = self.__values_edit {
                __pinapod_write_prefix(
                    &mut self.header_mut().__values_len,
                    0,
                    2usize,
                    __count,
                )?;
            }
            if __final_total < __old_total {
                self.data[__final_total..__old_total].fill(0);
            }
            self.total_len = __final_total;
            self.__values_edit = None;
            Ok(__final_total)
        }
    }
    pub struct CompactStatePatch<'__pinapod_patch>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        authority: Option<<Address as pina::pinapod::ZcField>::Pod>,
        bump: Option<<u8 as pina::pinapod::ZcField>::Pod>,
        values: Option<&'__pinapod_patch [<u64 as pina::pinapod::ZcField>::Pod]>,
        __pinapod_lifetime: core::marker::PhantomData<&'__pinapod_patch ()>,
    }
    impl<'__pinapod_patch> CompactStatePatch<'__pinapod_patch>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        pub fn new() -> Self {
            Self {
                authority: None,
                bump: None,
                values: None,
                __pinapod_lifetime: core::marker::PhantomData,
            }
        }
        pub fn authority(
            mut self,
            value: impl Into<<Address as pina::pinapod::ZcField>::Pod>,
        ) -> Self {
            self.authority = Some(value.into());
            self
        }
        pub fn bump(
            mut self,
            value: impl Into<<u8 as pina::pinapod::ZcField>::Pod>,
        ) -> Self {
            self.bump = Some(value.into());
            self
        }
        pub fn replace_values(
            mut self,
            value: &'__pinapod_patch [<u64 as pina::pinapod::ZcField>::Pod],
        ) -> Self {
            self.values = Some(value);
            self
        }
        fn validate_inputs(&self) -> Result<(), pina::pinapod::PinaPodError> {
            if let Some(value) = &self.authority {
                <<Address as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                    value,
                )?;
            }
            if let Some(value) = &self.bump {
                <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                    value,
                )?;
            }
            if let Some(value) = self.values {
                if value.len() > 4 {
                    return Err(pina::pinapod::PinaPodError::Overflow);
                }
                __pinapod_check_prefix(value.len(), 2usize)?;
                if core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>() == 0 {
                    return Err(pina::pinapod::PinaPodError::InvalidLength);
                }
                for item in value {
                    <<u64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                        item,
                    )?;
                }
            }
            Ok(())
        }
        pub fn updated_len(
            &self,
            data: &[u8],
        ) -> Result<usize, pina::pinapod::PinaPodError> {
            self.validate_inputs()?;
            <CompactState as pina::pinapod::PinaPodCompact>::validate(data)?;
            let __hdr = unsafe { &*(data.as_ptr() as *const CompactStateHeader) };
            let mut __offset = core::mem::size_of::<CompactStateHeader>();
            let __old_encoded_values: usize = __pinapod_checked_mul(
                u16::from_le_bytes(__hdr.__values_len) as usize,
                core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
            )?;
            __offset = __pinapod_checked_add(__offset, __old_encoded_values)?;
            let mut updated_len = __offset;
            if let Some(value) = self.values {
                let new_len = __pinapod_checked_mul(
                    value.len(),
                    core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
                )?;
                updated_len = updated_len
                    .checked_sub(__old_encoded_values)
                    .ok_or(pina::pinapod::PinaPodError::Overflow)?;
                updated_len = __pinapod_checked_add(updated_len, new_len)?;
            }
            Ok(updated_len)
        }
        fn initialized_len(&self) -> Result<usize, pina::pinapod::PinaPodError> {
            self.validate_inputs()?;
            let mut initialized_len = core::mem::size_of::<CompactStateHeader>();
            if let Some(value) = self.values {
                let new_len = __pinapod_checked_mul(
                    value.len(),
                    core::mem::size_of::<<u64 as pina::pinapod::ZcField>::Pod>(),
                )?;
                initialized_len = __pinapod_checked_add(initialized_len, new_len)?;
            }
            Ok(initialized_len)
        }
        pub fn update(
            &self,
            data: &mut [u8],
        ) -> Result<usize, pina::pinapod::PinaPodError> {
            let expected_len = self.updated_len(data)?;
            if expected_len > data.len() {
                return Err(pina::pinapod::PinaPodError::BufferTooSmall);
            }
            let mut writer = unsafe { <CompactStateMut<'_>>::new_unchecked(data) };
            if let Some(value) = self.values {
                writer.__values_edit = Some((value.as_ptr() as *const u8, value.len()));
            }
            let encoded_len = writer.commit()?;
            if let Some(value) = self.authority {
                *writer.authority_mut() = value;
            }
            if let Some(value) = self.bump {
                *writer.bump_mut() = value;
            }
            if true {
                match (&encoded_len, &expected_len) {
                    (left_val, right_val) => {
                        if !(*left_val == *right_val) {
                            let kind = ::core::panicking::AssertKind::Eq;
                            ::core::panicking::assert_failed(
                                kind,
                                &*left_val,
                                &*right_val,
                                ::core::option::Option::None,
                            );
                        }
                    }
                };
            }
            Ok(encoded_len)
        }
        fn try_initialize(
            &self,
            data: &mut [u8],
        ) -> Result<usize, pina::pinapod::PinaPodError> {
            <CompactState as pina::pinapod::PinaPodCompact>::validate_storage_len(
                data.len(),
            )?;
            let expected_len = self.initialized_len()?;
            if expected_len > data.len() {
                return Err(pina::pinapod::PinaPodError::BufferTooSmall);
            }
            let encoded_len = {
                let mut writer = unsafe { <CompactStateMut<'_>>::new_unchecked(data) };
                if let Some(value) = self.values {
                    writer.__values_edit = Some((
                        value.as_ptr() as *const u8,
                        value.len(),
                    ));
                }
                let encoded_len = writer.commit()?;
                if let Some(value) = self.authority {
                    *writer.authority_mut() = value;
                }
                if let Some(value) = self.bump {
                    *writer.bump_mut() = value;
                }
                encoded_len
            };
            <CompactState as pina::pinapod::PinaPodCompact>::validate(
                &data[..encoded_len],
            )?;
            if true {
                match (&encoded_len, &expected_len) {
                    (left_val, right_val) => {
                        if !(*left_val == *right_val) {
                            let kind = ::core::panicking::AssertKind::Eq;
                            ::core::panicking::assert_failed(
                                kind,
                                &*left_val,
                                &*right_val,
                                ::core::option::Option::None,
                            );
                        }
                    }
                };
            }
            Ok(encoded_len)
        }
        pub fn initialize(
            &self,
            data: &mut [u8],
        ) -> Result<usize, pina::pinapod::PinaPodError> {
            data.fill(0);
            let result = self.try_initialize(data);
            if result.is_err() {
                data.fill(0);
            }
            result
        }
    }
    impl<'__pinapod_patch> pina::pinapod::PinaPodPatch<CompactState>
    for CompactStatePatch<'__pinapod_patch>
    where
        [u8; PdaDisc::BYTES]: pina::pinapod::ZcElem,
        <Address as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
        <u64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    {
        fn updated_len(
            &self,
            data: &[u8],
        ) -> Result<usize, pina::pinapod::PinaPodError> {
            <CompactStatePatch<'__pinapod_patch>>::updated_len(self, data)
        }
        fn update(&self, data: &mut [u8]) -> Result<usize, pina::pinapod::PinaPodError> {
            <CompactStatePatch<'__pinapod_patch>>::update(self, data)
        }
        fn initialize(
            &self,
            data: &mut [u8],
        ) -> Result<usize, pina::pinapod::PinaPodError> {
            <CompactStatePatch<'__pinapod_patch>>::initialize(self, data)
        }
    }
}
#[allow(unused_imports)]
pub use __pinapod_compact_CompactState::{
    CompactStateHeader, CompactStatePatch, CompactStateRef,
};
///The PDA seeds for `CompactState`.
pub struct CompactStateSeeds<'a> {
    ///The `authority` seed.
    pub authority: &'a Address,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl<'a> ::core::clone::TrivialClone for CompactStateSeeds<'a> {}
#[automatically_derived]
impl<'a> ::core::clone::Clone for CompactStateSeeds<'a> {
    #[inline]
    fn clone(&self) -> CompactStateSeeds<'a> {
        let _: ::core::clone::AssertParamIsClone<&'a Address>;
        *self
    }
}
#[automatically_derived]
impl<'a> ::core::marker::Copy for CompactStateSeeds<'a> {}
///The PDA seeds for `CompactState`, including the bump seed.
pub struct CompactStateSeedsWithBump<'a> {
    inner: CompactStateSeeds<'a>,
    _bump: [u8; 1],
}
impl CompactState {
    /// Build the PDA seeds for this account.
    pub fn seeds<'a>(authority: &'a Address) -> CompactStateSeeds<'a> {
        CompactStateSeeds {
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
        let bump = pina::AsCompactAccount::with_compact_account::<
            Self,
            _,
        >(account, program_id, |state| Ok(state.bump))?;
        let seeds = Self::seeds(authority).with_bump(bump);
        <&pina::AccountView as pina::AccountInfoValidation>::assert_seeds_with_bump(
                account,
                &seeds.as_slices(),
                program_id,
            )
            .map(|_| ())
    }
    ///Load and validate `CompactState`, its canonical stored bump, and its PDA address for the duration of `use_account`.
    #[inline(always)]
    pub fn with_pda<R>(
        account: &pina::AccountView,
        authority: &Address,
        program_id: &pina::Address,
        use_account: impl FnOnce(
            <Self as pina::PinaCompactAccount>::Ref<'_>,
        ) -> ::core::result::Result<R, pina::ProgramError>,
    ) -> ::core::result::Result<R, pina::ProgramError> {
        let account_address = *account.address();
        pina::AsCompactAccount::with_compact_account::<
            Self,
            _,
        >(
            account,
            program_id,
            |state| {
                let seeds = Self::seeds(authority);
                let Some((expected_address, canonical_bump)) = pina::try_find_program_address(
                    &seeds.as_slices(),
                    program_id,
                ) else {
                    return Err(pina::ProgramError::InvalidSeeds);
                };
                if account_address != expected_address || state.bump != canonical_bump {
                    return Err(pina::ProgramError::InvalidSeeds);
                }
                use_account(state)
            },
        )
    }
}
impl<'a> CompactStateSeeds<'a> {
    /// The seeds as byte slices, without the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 2usize] {
        [b"compact", self.authority.as_ref()]
    }
    /// Append the bump seed to the seeds.
    pub fn with_bump(&self, bump: u8) -> CompactStateSeedsWithBump<'a> {
        CompactStateSeedsWithBump {
            inner: *self,
            _bump: [bump],
        }
    }
}
impl<'a> CompactStateSeedsWithBump<'a> {
    /// The seeds as byte slices, including the bump seed.
    pub fn as_slices(&self) -> [&[u8]; 3usize] {
        [b"compact", self.inner.authority.as_ref(), &self._bump]
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
const _: fn(u64) -> ::core::primitive::u64 = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::PodU64>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<u64>();
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
    fn assert_layout<T: pina::PinaPodCompact<Header = CompactStateHeader>>() {}
    assert_layout::<CompactState>();
};
const _: () = {
    if !(::core::mem::align_of::<CompactStateHeader>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<CompactStateHeader>() == 1",
        )
    }
    if !(::core::mem::size_of::<CompactStateHeader>()
        == PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<::core::primitive::u8>() + 2usize)
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<CompactStateHeader>() ==\n    PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>() +\n            ::core::mem::size_of::<::core::primitive::u8>() + 2usize",
        )
    }
    if !(<CompactState as pina::PinaPodCompact>::MIN_SIZE
        == PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<::core::primitive::u8>() + 2usize)
    {
        ::core::panicking::panic(
            "assertion failed: <CompactState as pina::PinaPodCompact>::MIN_SIZE ==\n    PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>() +\n            ::core::mem::size_of::<::core::primitive::u8>() + 2usize",
        )
    }
    if !(<CompactState as pina::PinaPodCompact>::MAX_SIZE
        == PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>()
            + ::core::mem::size_of::<::core::primitive::u8>() + 2usize
            + 4 * ::core::mem::size_of::<pina::PodU64>())
    {
        ::core::panicking::panic(
            "assertion failed: <CompactState as pina::PinaPodCompact>::MAX_SIZE ==\n    PdaDisc::BYTES + 0usize + ::core::mem::size_of::<pina::Address>() +\n                ::core::mem::size_of::<::core::primitive::u8>() + 2usize +\n        4 * ::core::mem::size_of::<pina::PodU64>()",
        )
    }
    if !(<CompactState as pina::PinaPodCompact>::TAIL_ALIGNMENT
        == {
            const fn gcd(mut left: usize, mut right: usize) -> usize {
                while right != 0 {
                    let remainder = left % right;
                    left = right;
                    right = remainder;
                }
                left
            }
            let mut alignment = 0;
            alignment = gcd(alignment, ::core::mem::size_of::<pina::PodU64>());
            alignment
        })
    {
        ::core::panicking::panic(
            "assertion failed: <CompactState as pina::PinaPodCompact>::TAIL_ALIGNMENT ==\n    {\n        const fn gcd(mut left: usize, mut right: usize) -> usize {\n            while right != 0 {\n                let remainder = left % right;\n                left = right;\n                right = remainder;\n            }\n            left\n        }\n        let mut alignment = 0;\n        alignment = gcd(alignment, ::core::mem::size_of::<pina::PodU64>());\n        alignment\n    }",
        )
    }
    if !(::core::mem::size_of::<pina::PodU64>() > 0) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<pina::PodU64>() > 0",
        )
    }
    if !(4 <= ::core::primitive::u16::MAX as usize) {
        ::core::panicking::panic(
            "assertion failed: 4 <= ::core::primitive::u16::MAX as usize",
        )
    }
};
impl CompactState {
    /// The fixed header size, including the discriminator and tail length prefix.
    pub const HEADER_SIZE: usize = <Self as pina::PinaPodCompact>::HEADER_SIZE;
    /// The minimum encoded and allocated size permitted by this compact schema.
    pub const MIN_SIZE: usize = <Self as pina::PinaPodCompact>::MIN_SIZE;
    /// The maximum encoded size permitted by this compact schema.
    pub const MAX_SIZE: usize = <Self as pina::PinaPodCompact>::MAX_SIZE;
    /// Byte granularity of valid compact account allocations.
    pub const TAIL_ALIGNMENT: usize = <Self as pina::PinaPodCompact>::TAIL_ALIGNMENT;
    ///Maximum element count for the `values` compact tail.
    pub const VALUES_CAPACITY: usize = 4;
    /// Calculate the exact encoded size for the requested compact tail counts.
    ///
    /// Count arguments follow the compact tails' declaration order.
    ///
    /// # Errors
    ///
    /// Returns `InvalidAccountData` when any count exceeds its declared capacity or the
    /// byte-size calculation overflows.
    pub fn projected_bytes(values_count: usize) -> Result<usize, pina::ProgramError> {
        if values_count > Self::VALUES_CAPACITY {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        let size = Self::HEADER_SIZE;
        let tail_size = values_count
            .checked_mul(::core::mem::size_of::<pina::PodU64>())
            .ok_or(pina::ProgramError::InvalidAccountData)?;
        let size = size
            .checked_add(tail_size)
            .ok_or(pina::ProgramError::InvalidAccountData)?;
        Ok(size)
    }
    /// Validate and borrow a compact account view.
    pub fn try_from_bytes(
        data: &[u8],
    ) -> Result<CompactStateRef<'_>, pina::ProgramError> {
        <Self as pina::PinaPodCompact>::validate_storage_len(data.len())
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        if !<Self as pina::HasDiscriminator>::matches_discriminator(data) {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        let value = CompactStateRef::new(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        <CompactStateRef<'_> as pina::PinaValidate>::validate(&value)?;
        Ok(value)
    }
    /// Calculate the encoded length after applying `patch` without changing `data`.
    pub fn updated_len(
        data: &[u8],
        patch: &CompactStatePatch<'_>,
    ) -> Result<usize, pina::ProgramError> {
        <Self as pina::PinaPodCompact>::validate_storage_len(data.len())
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        if !<Self as pina::HasDiscriminator>::matches_discriminator(data) {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        patch.updated_len(data).map_err(|_| pina::ProgramError::InvalidAccountData)
    }
    /// Apply `patch` to initialized compact account storage.
    ///
    /// Structural patch failures are atomic. With the `validation` feature,
    /// application validation runs after the patch is written. Propagate an
    /// application-validation error so the Solana runtime rolls the instruction
    /// back instead of committing the rejected representation.
    pub fn update(
        data: &mut [u8],
        patch: &CompactStatePatch<'_>,
    ) -> Result<usize, pina::ProgramError> {
        <Self as pina::PinaPodCompact>::validate_storage_len(data.len())
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        if !<Self as pina::HasDiscriminator>::matches_discriminator(data) {
            return Err(pina::ProgramError::InvalidAccountData);
        }
        let encoded_len = patch
            .update(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        <Self as pina::HasDiscriminator>::write_discriminator(data);
        let value = CompactStateRef::new(&data[..encoded_len])
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        <CompactStateRef<'_> as pina::PinaValidate>::validate(&value)?;
        Ok(encoded_len)
    }
    /// Initialize compact account storage from one complete patch.
    pub fn initialize(
        data: &mut [u8],
        patch: &CompactStatePatch<'_>,
    ) -> Result<usize, pina::ProgramError> {
        let encoded_len = patch
            .initialize(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        <Self as pina::HasDiscriminator>::write_discriminator(data);
        let value = CompactStateRef::new(&data[..encoded_len])
            .map_err(|_| pina::ProgramError::InvalidAccountData)?;
        if let Err(error) = <CompactStateRef<
            '_,
        > as pina::PinaValidate>::validate(&value) {
            data.fill(0);
            return Err(error);
        }
        Ok(encoded_len)
    }
}
impl pina::HasDiscriminator for CompactState {
    type Type = PdaDisc;
    const VALUE: Self::Type = PdaDisc::Compact;
}
impl pina::AccountValidation for CompactStateHeader {
    #[track_caller]
    fn assert<F>(&self, condition: F) -> Result<&Self, pina::ProgramError>
    where
        F: Fn(&Self) -> bool,
    {
        if condition(self) {
            return Ok(self);
        }
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
        ::pina::solana_program_log::logger::log_message("Account is invalid".as_bytes());
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
impl<'__pina_validation> pina::PinaValidate for CompactStateRef<'__pina_validation> {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaCompactAccount for CompactState {
    type Ref<'data> = CompactStateRef<'data>;
    type Patch<'patch> = CompactStatePatch<'patch>;
    fn try_from_bytes(data: &[u8]) -> Result<Self::Ref<'_>, pina::ProgramError> {
        Self::try_from_bytes(data)
    }
    fn validate_account_data(data: &[u8]) -> Result<(), pina::ProgramError> {
        Self::try_from_bytes(data).map(|_| ())
    }
    fn updated_len(
        data: &[u8],
        patch: &Self::Patch<'_>,
    ) -> Result<usize, pina::ProgramError> {
        Self::updated_len(data, patch)
    }
    fn update(
        data: &mut [u8],
        patch: &Self::Patch<'_>,
    ) -> Result<usize, pina::ProgramError> {
        Self::update(data, patch)
    }
    fn initialize(
        data: &mut [u8],
        patch: &Self::Patch<'_>,
    ) -> Result<usize, pina::ProgramError> {
        Self::initialize(data, patch)
    }
}
impl<'__pina_patch> pina::PinaCompactPatch<CompactState>
for CompactStatePatch<'__pina_patch> {
    #[inline(always)]
    fn as_pina_patch(&self) -> &<CompactState as pina::PinaCompactAccount>::Patch<'_> {
        self
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
