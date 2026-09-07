use pina::*;
#[repr(u8)]
#[non_exhaustive]
pub enum AccountDisc {
    ConfigState = 1,
    GameState = 2,
    DataAccount = 3,
    BalanceAccount = 4,
    Custom = 5,
    LargeState = 6,
}
#[automatically_derived]
impl ::core::fmt::Debug for AccountDisc {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::write_str(
            f,
            match self {
                AccountDisc::ConfigState => "ConfigState",
                AccountDisc::GameState => "GameState",
                AccountDisc::DataAccount => "DataAccount",
                AccountDisc::BalanceAccount => "BalanceAccount",
                AccountDisc::Custom => "Custom",
                AccountDisc::LargeState => "LargeState",
            },
        )
    }
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for AccountDisc {}
#[automatically_derived]
impl ::core::clone::Clone for AccountDisc {
    #[inline]
    fn clone(&self) -> AccountDisc {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for AccountDisc {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for AccountDisc {}
#[automatically_derived]
impl ::core::cmp::PartialEq for AccountDisc {
    #[inline]
    fn eq(&self, other: &AccountDisc) -> bool {
        let __self_discr = ::core::intrinsics::discriminant_value(self);
        let __arg1_discr = ::core::intrinsics::discriminant_value(other);
        __self_discr == __arg1_discr
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for AccountDisc {
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
impl ::core::convert::From<AccountDisc> for u8 {
    #[inline]
    fn from(enum_value: AccountDisc) -> Self {
        enum_value as Self
    }
}
impl ::core::convert::TryFrom<u8> for AccountDisc {
    type Error = ::pina::ProgramError;
    #[inline]
    fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
        #![allow(non_upper_case_globals)]
        const __CONFIG_STATE: u8 = 1;
        const __GAME_STATE: u8 = 2;
        const __DATA_ACCOUNT: u8 = 3;
        const __BALANCE_ACCOUNT: u8 = 4;
        const __CUSTOM: u8 = 5;
        const __LARGE_STATE: u8 = 6;
        #[deny(unreachable_patterns)]
        match number {
            __CONFIG_STATE => ::core::result::Result::Ok(Self::ConfigState),
            __GAME_STATE => ::core::result::Result::Ok(Self::GameState),
            __DATA_ACCOUNT => ::core::result::Result::Ok(Self::DataAccount),
            __BALANCE_ACCOUNT => ::core::result::Result::Ok(Self::BalanceAccount),
            __CUSTOM => ::core::result::Result::Ok(Self::Custom),
            __LARGE_STATE => ::core::result::Result::Ok(Self::LargeState),
            #[allow(unreachable_patterns)]
            _ => {
                ::core::result::Result::Err(
                    ::pina::PinaProgramError::InvalidDiscriminator.into(),
                )
            }
        }
    }
}
const _: () = if !(::core::mem::size_of::<AccountDisc>() == ::core::mem::size_of::<u8>())
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "The size of the enum `AccountDisc` must match the size of its primitive representation\n\t\t\t\t`u8`.",
            ),
        );
    }
};
impl ::pina::IntoDiscriminator for AccountDisc {
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
pub struct ConfigState {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; AccountDisc::BYTES],
    pub version: u8,
    pub bump: u8,
}
#[repr(C)]
pub struct ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub version: <u8 as pina::pinapod::ZcField>::Pod,
    pub bump: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<ConfigStateZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<ConfigStateZc>() == 1",
    )
};
impl ConfigStateZc {
    #[inline(always)]
    pub fn version(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.version
    }
    #[inline(always)]
    pub fn bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.bump
    }
}
impl pina::pinapod::ZcValidate for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.version,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.bump,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for ConfigState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for ConfigState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = ConfigStateZc;
}
unsafe impl pina::pinapod::ZcField for ConfigState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = ConfigStateZc;
}
unsafe impl pina::pinapod::ZcElem for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<ConfigStateZc>();
};
const _: () = {
    if !(::core::mem::align_of::<ConfigStateZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<ConfigStateZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<ConfigStateZc>()
        == AccountDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<ConfigStateZc>() ==\n    AccountDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl ConfigState {
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
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for ConfigState {
    type Type = AccountDisc;
    const VALUE: Self::Type = AccountDisc::ConfigState;
}
impl pina::AccountValidation for ConfigStateZc {
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
impl pina::PinaAccount for ConfigState {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct GameState {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; AccountDisc::BYTES],
    pub score: u8,
    pub level: u8,
}
#[automatically_derived]
impl ::core::fmt::Debug for GameState {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field3_finish(
            f,
            "GameState",
            "discriminator",
            &self.discriminator,
            "score",
            &self.score,
            "level",
            &&self.level,
        )
    }
}
#[repr(C)]
pub struct GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub score: <u8 as pina::pinapod::ZcField>::Pod,
    pub level: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<GameStateZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<GameStateZc>() == 1",
    )
};
impl GameStateZc {
    #[inline(always)]
    pub fn score(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.score
    }
    #[inline(always)]
    pub fn level(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.level
    }
}
impl pina::pinapod::ZcValidate for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.score,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.level,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for GameState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for GameState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = GameStateZc;
}
unsafe impl pina::pinapod::ZcField for GameState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = GameStateZc;
}
unsafe impl pina::pinapod::ZcElem for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<GameStateZc>();
};
const _: () = {
    if !(::core::mem::align_of::<GameStateZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<GameStateZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<GameStateZc>()
        == AccountDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<GameStateZc>() ==\n    AccountDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl GameState {
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
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for GameState {
    type Type = AccountDisc;
    const VALUE: Self::Type = AccountDisc::GameState;
}
impl pina::AccountValidation for GameStateZc {
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
impl pina::PinaAccount for GameState {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct DataAccount {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; AccountDisc::BYTES],
    pub authority: [u8; 32],
    pub data: [u8; 64],
    pub flags: [u8; 4],
}
#[repr(C)]
pub struct DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub authority: <[u8; 32] as pina::pinapod::ZcField>::Pod,
    pub data: <[u8; 64] as pina::pinapod::ZcField>::Pod,
    pub flags: <[u8; 4] as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<DataAccountZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<DataAccountZc>() == 1",
    )
};
impl DataAccountZc {
    #[inline(always)]
    pub fn authority(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.authority
    }
    #[inline(always)]
    pub fn data(&self) -> &<[u8; 64] as pina::pinapod::ZcField>::Pod {
        &self.data
    }
    #[inline(always)]
    pub fn flags(&self) -> &<[u8; 4] as pina::pinapod::ZcField>::Pod {
        &self.flags
    }
}
impl pina::pinapod::ZcValidate for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.authority,
        )?;
        <<[u8; 64] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.data,
        )?;
        <<[u8; 4] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.flags,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for DataAccount
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for DataAccount
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = DataAccountZc;
}
unsafe impl pina::pinapod::ZcField for DataAccount
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = DataAccountZc;
}
unsafe impl pina::pinapod::ZcElem for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
const _: fn([u8; 64]) -> [::core::primitive::u8; 64] = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 64]>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<[u8; 64]>();
    assert_storage::<[::core::primitive::u8; 64]>();
};
const _: () = {
    if !(::core::mem::align_of::<[::core::primitive::u8; 64]>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 64]>() == 1",
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
    assert_storage::<DataAccountZc>();
};
const _: () = {
    if !(::core::mem::align_of::<DataAccountZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<DataAccountZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<DataAccountZc>()
        == AccountDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<[::core::primitive::u8; 64]>()
            + ::core::mem::size_of::<[::core::primitive::u8; 4]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<DataAccountZc>() ==\n    AccountDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()\n            + ::core::mem::size_of::<[::core::primitive::u8; 64]>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 4]>()",
        )
    }
};
impl DataAccount {
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
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for DataAccount {
    type Type = AccountDisc;
    const VALUE: Self::Type = AccountDisc::DataAccount;
}
impl pina::AccountValidation for DataAccountZc {
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
impl pina::PinaAccount for DataAccount {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct BalanceAccount {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; AccountDisc::BYTES],
    pub owner: [u8; 32],
    pub amount: PodU64,
    pub decimals: u8,
    pub is_frozen: PodBool,
}
#[repr(C)]
pub struct BalanceAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub owner: <[u8; 32] as pina::pinapod::ZcField>::Pod,
    pub amount: <PodU64 as pina::pinapod::ZcField>::Pod,
    pub decimals: <u8 as pina::pinapod::ZcField>::Pod,
    pub is_frozen: <PodBool as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for BalanceAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for BalanceAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<BalanceAccountZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<BalanceAccountZc>() == 1",
    )
};
impl BalanceAccountZc {
    #[inline(always)]
    pub fn owner(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.owner
    }
    #[inline(always)]
    pub fn amount(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.amount
    }
    #[inline(always)]
    pub fn decimals(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.decimals
    }
    #[inline(always)]
    pub fn is_frozen(&self) -> &<PodBool as pina::pinapod::ZcField>::Pod {
        &self.is_frozen
    }
}
impl pina::pinapod::ZcValidate for BalanceAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.owner,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.amount,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.decimals,
        )?;
        <<PodBool as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.is_frozen,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for BalanceAccount
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for BalanceAccount
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = BalanceAccountZc;
}
unsafe impl pina::pinapod::ZcField for BalanceAccount
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = BalanceAccountZc;
}
unsafe impl pina::pinapod::ZcElem for BalanceAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodBool: pina::pinapod::ZcField,
    <PodBool as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
const _: fn(PodBool) -> pina::PodBool = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = pina::PodBool>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<PodBool>();
    assert_storage::<pina::PodBool>();
};
const _: () = {
    if !(::core::mem::align_of::<pina::PodBool>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<pina::PodBool>() == 1",
        )
    }
};
const _: fn() = || {
    fn assert_storage<T: pina::ZcElem>() {}
    assert_storage::<BalanceAccountZc>();
};
const _: () = {
    if !(::core::mem::align_of::<BalanceAccountZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<BalanceAccountZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<BalanceAccountZc>()
        == AccountDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<pina::PodBool>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<BalanceAccountZc>() ==\n    AccountDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()\n                + ::core::mem::size_of::<pina::PodU64>() +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<pina::PodBool>()",
        )
    }
};
impl BalanceAccount {
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
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for BalanceAccount {
    type Type = AccountDisc;
    const VALUE: Self::Type = AccountDisc::BalanceAccount;
}
impl pina::AccountValidation for BalanceAccountZc {
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
impl pina::PinaAccount for BalanceAccount {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct MyStruct {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; AccountDisc::BYTES],
    pub value: u8,
}
#[repr(C)]
pub struct MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub value: <u8 as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<MyStructZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<MyStructZc>() == 1",
    )
};
impl MyStructZc {
    #[inline(always)]
    pub fn value(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.value
    }
}
impl pina::pinapod::ZcValidate for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.value,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for MyStruct
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for MyStruct
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = MyStructZc;
}
unsafe impl pina::pinapod::ZcField for MyStruct
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = MyStructZc;
}
unsafe impl pina::pinapod::ZcElem for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
    assert_storage::<MyStructZc>();
};
const _: () = {
    if !(::core::mem::align_of::<MyStructZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<MyStructZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<MyStructZc>()
        == AccountDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<MyStructZc>() ==\n    AccountDisc::BYTES + ::core::mem::size_of::<::core::primitive::u8>()",
        )
    }
};
impl MyStruct {
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
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for MyStruct {
    type Type = AccountDisc;
    const VALUE: Self::Type = AccountDisc::Custom;
}
impl pina::AccountValidation for MyStructZc {
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
impl pina::PinaAccount for MyStruct {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
#[pinapod(crate = pina::pinapod, no_inherent)]
pub struct LargeState {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; AccountDisc::BYTES],
    pub authority: [u8; 32],
    pub bump: u8,
    pub treasury_bump: u8,
    pub mint_bump: u8,
    pub version: u8,
    pub padding: [u8; 3],
    pub total_supply: PodU64,
    pub name: [u8; 32],
}
#[repr(C)]
pub struct LargeStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod,
    pub authority: <[u8; 32] as pina::pinapod::ZcField>::Pod,
    pub bump: <u8 as pina::pinapod::ZcField>::Pod,
    pub treasury_bump: <u8 as pina::pinapod::ZcField>::Pod,
    pub mint_bump: <u8 as pina::pinapod::ZcField>::Pod,
    pub version: <u8 as pina::pinapod::ZcField>::Pod,
    pub padding: <[u8; 3] as pina::pinapod::ZcField>::Pod,
    pub total_supply: <PodU64 as pina::pinapod::ZcField>::Pod,
    pub name: <[u8; 32] as pina::pinapod::ZcField>::Pod,
}
impl ::core::marker::Copy for LargeStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
impl ::core::clone::Clone for LargeStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn clone(&self) -> Self {
        *self
    }
}
const _: () = if !(::core::mem::align_of::<LargeStateZc>() == 1) {
    ::core::panicking::panic(
        "assertion failed: ::core::mem::align_of::<LargeStateZc>() == 1",
    )
};
impl LargeStateZc {
    #[inline(always)]
    pub fn authority(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.authority
    }
    #[inline(always)]
    pub fn bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.bump
    }
    #[inline(always)]
    pub fn treasury_bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.treasury_bump
    }
    #[inline(always)]
    pub fn mint_bump(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.mint_bump
    }
    #[inline(always)]
    pub fn version(&self) -> <u8 as pina::pinapod::ZcField>::Pod {
        self.version
    }
    #[inline(always)]
    pub fn padding(&self) -> &<[u8; 3] as pina::pinapod::ZcField>::Pod {
        &self.padding
    }
    #[inline(always)]
    pub fn total_supply(&self) -> &<PodU64 as pina::pinapod::ZcField>::Pod {
        &self.total_supply
    }
    #[inline(always)]
    pub fn name(&self) -> &<[u8; 32] as pina::pinapod::ZcField>::Pod {
        &self.name
    }
}
impl pina::pinapod::ZcValidate for LargeStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    fn validate_ref(
        value: &Self,
    ) -> ::core::result::Result<(), pina::pinapod::PinaPodError> {
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.discriminator,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.authority,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.bump,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.treasury_bump,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.mint_bump,
        )?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.version,
        )?;
        <<[u8; 3] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.padding,
        )?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.total_supply,
        )?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
            &value.name,
        )?;
        ::core::result::Result::Ok(())
    }
}
impl pina::pinapod::PinaPod for LargeState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{}
unsafe impl pina::pinapod::PinaPodFixed for LargeState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Zc = LargeStateZc;
}
unsafe impl pina::pinapod::ZcField for LargeState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    type Pod = LargeStateZc;
}
unsafe impl pina::pinapod::ZcElem for LargeStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 3]: pina::pinapod::ZcField,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    PodU64: pina::pinapod::ZcField,
    <PodU64 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
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
const _: fn([u8; 3]) -> [::core::primitive::u8; 3] = |value| value;
const _: fn() = || {
    fn assert_mapping<T: pina::ZcField<Pod = [::core::primitive::u8; 3]>>() {}
    fn assert_storage<T: pina::ZcElem>() {}
    assert_mapping::<[u8; 3]>();
    assert_storage::<[::core::primitive::u8; 3]>();
};
const _: () = {
    if !(::core::mem::align_of::<[::core::primitive::u8; 3]>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<[::core::primitive::u8; 3]>() == 1",
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
    assert_storage::<LargeStateZc>();
};
const _: () = {
    if !(::core::mem::align_of::<LargeStateZc>() == 1) {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::align_of::<LargeStateZc>() == 1",
        )
    }
    if !(::core::mem::size_of::<LargeStateZc>()
        == AccountDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<[::core::primitive::u8; 3]>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<LargeStateZc>() ==\n    AccountDisc::BYTES + ::core::mem::size_of::<[::core::primitive::u8; 32]>()\n                                + ::core::mem::size_of::<::core::primitive::u8>() +\n                            ::core::mem::size_of::<::core::primitive::u8>() +\n                        ::core::mem::size_of::<::core::primitive::u8>() +\n                    ::core::mem::size_of::<::core::primitive::u8>() +\n                ::core::mem::size_of::<[::core::primitive::u8; 3]>() +\n            ::core::mem::size_of::<pina::PodU64>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 32]>()",
        )
    }
};
impl LargeState {
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
        <Self as pina::PinaPodFixed>::read_exact(data)
            .map_err(|_| pina::ProgramError::InvalidAccountData)
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
        <Self as pina::PinaAccount>::initialize(data, initialize)
    }
}
impl pina::HasDiscriminator for LargeState {
    type Type = AccountDisc;
    const VALUE: Self::Type = AccountDisc::LargeState;
}
impl pina::AccountValidation for LargeStateZc {
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
impl pina::PinaAccount for LargeState {
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
