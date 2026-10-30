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
impl ::core::cmp::Eq for AccountDisc {}
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
        const _: () = {
            if !(__CONFIG_STATE != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `ConfigState` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        const _: () = {
            if !(__GAME_STATE != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `GameState` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        const _: () = {
            if !(__DATA_ACCOUNT != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `DataAccount` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        const _: () = {
            if !(__BALANCE_ACCOUNT != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `BalanceAccount` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        const _: () = {
            if !(__CUSTOM != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `Custom` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        const _: () = {
            if !(__LARGE_STATE != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `LargeState` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
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
    fn try_write_discriminator(
        &self,
        bytes: &mut [u8],
    ) -> ::core::result::Result<(), ::pina::ProgramError> {
        (*self as u8).try_write_discriminator(bytes)
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
impl ::core::cmp::PartialEq for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> ::core::primitive::bool {
        true && self.discriminator == other.discriminator
            && self.version == other.version && self.bump == other.bump
    }
}
impl ::core::cmp::Eq for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
{}
impl ::core::hash::Hash for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
{
    fn hash<__State: ::core::hash::Hasher>(&self, state: &mut __State) {
        self.discriminator.hash(state);
        self.version.hash(state);
        self.bump.hash(state);
    }
}
impl ::core::fmt::Debug for ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter
            .debug_struct("ConfigState")
            .field("discriminator", &self.discriminator)
            .field("version", &self.version)
            .field("bump", &self.bump)
            .finish()
    }
}
impl ConfigStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
{
    /// Materialize the declared schema type from this stored representation.
    ///
    /// The result is a fully owned, self-contained value: snapshots,
    /// logging, tests, and off-chain decoding all want a shape a
    /// borrowed view cannot serve. For a value that reached the caller
    /// through a validating reader this always succeeds; a stored enum
    /// discriminant naming no variant is the only failure.
    #[inline]
    pub fn to_native(
        &self,
    ) -> ::core::result::Result<ConfigState, pina::pinapod::PinaPodError> {
        ::core::result::Result::Ok(ConfigState {
            discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::FromZc>::from_zc(
                &self.discriminator,
            )?,
            version: <u8 as pina::pinapod::FromZc>::from_zc(&self.version)?,
            bump: <u8 as pina::pinapod::FromZc>::from_zc(&self.bump)?,
        })
    }
}
impl pina::pinapod::FromZc for ConfigState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
{
    #[inline]
    fn from_zc(
        pod: &Self::Pod,
    ) -> ::core::result::Result<Self, pina::pinapod::PinaPodError> {
        pod.to_native()
    }
}
impl ConfigState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    /// Validate and attribute: names the first field whose stored value
    /// a reader would reject.
    ///
    /// Accepts exactly what [`PinaPodFixed::validate_prefix`](pinapod::PinaPodFixed::validate_prefix)
    /// accepts, so a diagnostic run after a rejected read answers
    /// *which field*, never *whether*. A nested schema field is
    /// attributed at its own name; call the nested `diagnose` on the
    /// field's bytes for one level deeper.
    #[inline]
    pub fn diagnose(
        data: &[::core::primitive::u8],
    ) -> ::core::result::Result<(), pina::pinapod::Diagnostic> {
        let size = ::core::mem::size_of::<ConfigStateZc>();
        if data.len() < size {
            return ::core::result::Result::Err(
                pina::pinapod::Diagnostic::new(
                    "ConfigState",
                    pina::pinapod::PinaPodError::BufferTooSmall,
                ),
            );
        }
        let value: &ConfigStateZc = unsafe { &*data.as_ptr().cast::<ConfigStateZc>() };
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.discriminator,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("discriminator", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.version,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("version", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.bump,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("bump", error))?;
        ::core::result::Result::Ok(())
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
        == AccountDisc::BYTES + 0usize + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<ConfigStateZc>() ==\n    AccountDisc::BYTES + 0usize +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
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
            #[cold]
            #[inline(never)]
            fn __pina_header_error(is_size: bool) -> pina::ProgramError {
                if is_size {
                    return pina::PinaProgramError::InvalidAccountSize.into();
                }
                pina::PinaProgramError::InvalidDiscriminator.into()
            }
            return Err(__pina_header_error(data.len() != Self::SIZE));
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
impl pina::PinaValidate for ConfigStateZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for ConfigState {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <ConfigStateZc as pina::PinaValidate>::validate(value)
    }
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
impl ::core::cmp::PartialEq for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> ::core::primitive::bool {
        true && self.discriminator == other.discriminator && self.score == other.score
            && self.level == other.level
    }
}
impl ::core::cmp::Eq for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
{}
impl ::core::hash::Hash for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
{
    fn hash<__State: ::core::hash::Hasher>(&self, state: &mut __State) {
        self.discriminator.hash(state);
        self.score.hash(state);
        self.level.hash(state);
    }
}
impl ::core::fmt::Debug for GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter
            .debug_struct("GameState")
            .field("discriminator", &self.discriminator)
            .field("score", &self.score)
            .field("level", &self.level)
            .finish()
    }
}
impl GameStateZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
{
    /// Materialize the declared schema type from this stored representation.
    ///
    /// The result is a fully owned, self-contained value: snapshots,
    /// logging, tests, and off-chain decoding all want a shape a
    /// borrowed view cannot serve. For a value that reached the caller
    /// through a validating reader this always succeeds; a stored enum
    /// discriminant naming no variant is the only failure.
    #[inline]
    pub fn to_native(
        &self,
    ) -> ::core::result::Result<GameState, pina::pinapod::PinaPodError> {
        ::core::result::Result::Ok(GameState {
            discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::FromZc>::from_zc(
                &self.discriminator,
            )?,
            score: <u8 as pina::pinapod::FromZc>::from_zc(&self.score)?,
            level: <u8 as pina::pinapod::FromZc>::from_zc(&self.level)?,
        })
    }
}
impl pina::pinapod::FromZc for GameState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
{
    #[inline]
    fn from_zc(
        pod: &Self::Pod,
    ) -> ::core::result::Result<Self, pina::pinapod::PinaPodError> {
        pod.to_native()
    }
}
impl GameState
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    /// Validate and attribute: names the first field whose stored value
    /// a reader would reject.
    ///
    /// Accepts exactly what [`PinaPodFixed::validate_prefix`](pinapod::PinaPodFixed::validate_prefix)
    /// accepts, so a diagnostic run after a rejected read answers
    /// *which field*, never *whether*. A nested schema field is
    /// attributed at its own name; call the nested `diagnose` on the
    /// field's bytes for one level deeper.
    #[inline]
    pub fn diagnose(
        data: &[::core::primitive::u8],
    ) -> ::core::result::Result<(), pina::pinapod::Diagnostic> {
        let size = ::core::mem::size_of::<GameStateZc>();
        if data.len() < size {
            return ::core::result::Result::Err(
                pina::pinapod::Diagnostic::new(
                    "GameState",
                    pina::pinapod::PinaPodError::BufferTooSmall,
                ),
            );
        }
        let value: &GameStateZc = unsafe { &*data.as_ptr().cast::<GameStateZc>() };
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.discriminator,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("discriminator", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.score,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("score", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.level,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("level", error))?;
        ::core::result::Result::Ok(())
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
        == AccountDisc::BYTES + 0usize + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<GameStateZc>() ==\n    AccountDisc::BYTES + 0usize +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<::core::primitive::u8>()",
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
            #[cold]
            #[inline(never)]
            fn __pina_header_error(is_size: bool) -> pina::ProgramError {
                if is_size {
                    return pina::PinaProgramError::InvalidAccountSize.into();
                }
                pina::PinaProgramError::InvalidDiscriminator.into()
            }
            return Err(__pina_header_error(data.len() != Self::SIZE));
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
impl pina::PinaValidate for GameStateZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for GameState {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <GameStateZc as pina::PinaValidate>::validate(value)
    }
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
impl ::core::cmp::PartialEq for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> ::core::primitive::bool {
        true && self.discriminator == other.discriminator
            && self.authority == other.authority && self.data == other.data
            && self.flags == other.flags
    }
}
impl ::core::cmp::Eq for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
{}
impl ::core::hash::Hash for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
{
    fn hash<__State: ::core::hash::Hasher>(&self, state: &mut __State) {
        self.discriminator.hash(state);
        self.authority.hash(state);
        self.data.hash(state);
        self.flags.hash(state);
    }
}
impl ::core::fmt::Debug for DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter
            .debug_struct("DataAccount")
            .field("discriminator", &self.discriminator)
            .field("authority", &self.authority)
            .field("data", &self.data)
            .field("flags", &self.flags)
            .finish()
    }
}
impl DataAccountZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
    [u8; 64]: pina::pinapod::FromZc,
    [u8; 4]: pina::pinapod::FromZc,
{
    /// Materialize the declared schema type from this stored representation.
    ///
    /// The result is a fully owned, self-contained value: snapshots,
    /// logging, tests, and off-chain decoding all want a shape a
    /// borrowed view cannot serve. For a value that reached the caller
    /// through a validating reader this always succeeds; a stored enum
    /// discriminant naming no variant is the only failure.
    #[inline]
    pub fn to_native(
        &self,
    ) -> ::core::result::Result<DataAccount, pina::pinapod::PinaPodError> {
        ::core::result::Result::Ok(DataAccount {
            discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::FromZc>::from_zc(
                &self.discriminator,
            )?,
            authority: <[u8; 32] as pina::pinapod::FromZc>::from_zc(&self.authority)?,
            data: <[u8; 64] as pina::pinapod::FromZc>::from_zc(&self.data)?,
            flags: <[u8; 4] as pina::pinapod::FromZc>::from_zc(&self.flags)?,
        })
    }
}
impl pina::pinapod::FromZc for DataAccount
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 32]: pina::pinapod::ZcField,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 64]: pina::pinapod::ZcField,
    <[u8; 64] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; 4]: pina::pinapod::ZcField,
    <[u8; 4] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
    [u8; 64]: pina::pinapod::FromZc,
    [u8; 4]: pina::pinapod::FromZc,
{
    #[inline]
    fn from_zc(
        pod: &Self::Pod,
    ) -> ::core::result::Result<Self, pina::pinapod::PinaPodError> {
        pod.to_native()
    }
}
impl DataAccount
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
    /// Validate and attribute: names the first field whose stored value
    /// a reader would reject.
    ///
    /// Accepts exactly what [`PinaPodFixed::validate_prefix`](pinapod::PinaPodFixed::validate_prefix)
    /// accepts, so a diagnostic run after a rejected read answers
    /// *which field*, never *whether*. A nested schema field is
    /// attributed at its own name; call the nested `diagnose` on the
    /// field's bytes for one level deeper.
    #[inline]
    pub fn diagnose(
        data: &[::core::primitive::u8],
    ) -> ::core::result::Result<(), pina::pinapod::Diagnostic> {
        let size = ::core::mem::size_of::<DataAccountZc>();
        if data.len() < size {
            return ::core::result::Result::Err(
                pina::pinapod::Diagnostic::new(
                    "DataAccount",
                    pina::pinapod::PinaPodError::BufferTooSmall,
                ),
            );
        }
        let value: &DataAccountZc = unsafe { &*data.as_ptr().cast::<DataAccountZc>() };
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.discriminator,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("discriminator", error))?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.authority,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("authority", error))?;
        <<[u8; 64] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.data,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("data", error))?;
        <<[u8; 4] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.flags,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("flags", error))?;
        ::core::result::Result::Ok(())
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
        == AccountDisc::BYTES + 0usize
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<[::core::primitive::u8; 64]>()
            + ::core::mem::size_of::<[::core::primitive::u8; 4]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<DataAccountZc>() ==\n    AccountDisc::BYTES + 0usize +\n                ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n            ::core::mem::size_of::<[::core::primitive::u8; 64]>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 4]>()",
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
            #[cold]
            #[inline(never)]
            fn __pina_header_error(is_size: bool) -> pina::ProgramError {
                if is_size {
                    return pina::PinaProgramError::InvalidAccountSize.into();
                }
                pina::PinaProgramError::InvalidDiscriminator.into()
            }
            return Err(__pina_header_error(data.len() != Self::SIZE));
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
impl pina::PinaValidate for DataAccountZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for DataAccount {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <DataAccountZc as pina::PinaValidate>::validate(value)
    }
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
impl ::core::cmp::PartialEq for BalanceAccountZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <PodBool as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> ::core::primitive::bool {
        true && self.discriminator == other.discriminator && self.owner == other.owner
            && self.amount == other.amount && self.decimals == other.decimals
            && self.is_frozen == other.is_frozen
    }
}
impl ::core::cmp::Eq for BalanceAccountZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <PodBool as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
{}
impl ::core::hash::Hash for BalanceAccountZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <PodBool as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
{
    fn hash<__State: ::core::hash::Hasher>(&self, state: &mut __State) {
        self.discriminator.hash(state);
        self.owner.hash(state);
        self.amount.hash(state);
        self.decimals.hash(state);
        self.is_frozen.hash(state);
    }
}
impl ::core::fmt::Debug for BalanceAccountZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <PodBool as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter
            .debug_struct("BalanceAccount")
            .field("discriminator", &self.discriminator)
            .field("owner", &self.owner)
            .field("amount", &self.amount)
            .field("decimals", &self.decimals)
            .field("is_frozen", &self.is_frozen)
            .finish()
    }
}
impl BalanceAccountZc
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
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
    PodU64: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    PodBool: pina::pinapod::FromZc,
{
    /// Materialize the declared schema type from this stored representation.
    ///
    /// The result is a fully owned, self-contained value: snapshots,
    /// logging, tests, and off-chain decoding all want a shape a
    /// borrowed view cannot serve. For a value that reached the caller
    /// through a validating reader this always succeeds; a stored enum
    /// discriminant naming no variant is the only failure.
    #[inline]
    pub fn to_native(
        &self,
    ) -> ::core::result::Result<BalanceAccount, pina::pinapod::PinaPodError> {
        ::core::result::Result::Ok(BalanceAccount {
            discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::FromZc>::from_zc(
                &self.discriminator,
            )?,
            owner: <[u8; 32] as pina::pinapod::FromZc>::from_zc(&self.owner)?,
            amount: <PodU64 as pina::pinapod::FromZc>::from_zc(&self.amount)?,
            decimals: <u8 as pina::pinapod::FromZc>::from_zc(&self.decimals)?,
            is_frozen: <PodBool as pina::pinapod::FromZc>::from_zc(&self.is_frozen)?,
        })
    }
}
impl pina::pinapod::FromZc for BalanceAccount
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
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
    PodU64: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    PodBool: pina::pinapod::FromZc,
{
    #[inline]
    fn from_zc(
        pod: &Self::Pod,
    ) -> ::core::result::Result<Self, pina::pinapod::PinaPodError> {
        pod.to_native()
    }
}
impl BalanceAccount
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
    /// Validate and attribute: names the first field whose stored value
    /// a reader would reject.
    ///
    /// Accepts exactly what [`PinaPodFixed::validate_prefix`](pinapod::PinaPodFixed::validate_prefix)
    /// accepts, so a diagnostic run after a rejected read answers
    /// *which field*, never *whether*. A nested schema field is
    /// attributed at its own name; call the nested `diagnose` on the
    /// field's bytes for one level deeper.
    #[inline]
    pub fn diagnose(
        data: &[::core::primitive::u8],
    ) -> ::core::result::Result<(), pina::pinapod::Diagnostic> {
        let size = ::core::mem::size_of::<BalanceAccountZc>();
        if data.len() < size {
            return ::core::result::Result::Err(
                pina::pinapod::Diagnostic::new(
                    "BalanceAccount",
                    pina::pinapod::PinaPodError::BufferTooSmall,
                ),
            );
        }
        let value: &BalanceAccountZc = unsafe {
            &*data.as_ptr().cast::<BalanceAccountZc>()
        };
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.discriminator,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("discriminator", error))?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.owner,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("owner", error))?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.amount,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("amount", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.decimals,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("decimals", error))?;
        <<PodBool as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.is_frozen,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("is_frozen", error))?;
        ::core::result::Result::Ok(())
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
        == AccountDisc::BYTES + 0usize
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<pina::PodBool>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<BalanceAccountZc>() ==\n    AccountDisc::BYTES + 0usize +\n                    ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n                ::core::mem::size_of::<pina::PodU64>() +\n            ::core::mem::size_of::<::core::primitive::u8>() +\n        ::core::mem::size_of::<pina::PodBool>()",
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
            #[cold]
            #[inline(never)]
            fn __pina_header_error(is_size: bool) -> pina::ProgramError {
                if is_size {
                    return pina::PinaProgramError::InvalidAccountSize.into();
                }
                pina::PinaProgramError::InvalidDiscriminator.into()
            }
            return Err(__pina_header_error(data.len() != Self::SIZE));
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
impl pina::PinaValidate for BalanceAccountZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for BalanceAccount {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <BalanceAccountZc as pina::PinaValidate>::validate(value)
    }
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
impl ::core::cmp::PartialEq for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> ::core::primitive::bool {
        true && self.discriminator == other.discriminator && self.value == other.value
    }
}
impl ::core::cmp::Eq for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
{}
impl ::core::hash::Hash for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
{
    fn hash<__State: ::core::hash::Hasher>(&self, state: &mut __State) {
        self.discriminator.hash(state);
        self.value.hash(state);
    }
}
impl ::core::fmt::Debug for MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter
            .debug_struct("MyStruct")
            .field("discriminator", &self.discriminator)
            .field("value", &self.value)
            .finish()
    }
}
impl MyStructZc
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
{
    /// Materialize the declared schema type from this stored representation.
    ///
    /// The result is a fully owned, self-contained value: snapshots,
    /// logging, tests, and off-chain decoding all want a shape a
    /// borrowed view cannot serve. For a value that reached the caller
    /// through a validating reader this always succeeds; a stored enum
    /// discriminant naming no variant is the only failure.
    #[inline]
    pub fn to_native(
        &self,
    ) -> ::core::result::Result<MyStruct, pina::pinapod::PinaPodError> {
        ::core::result::Result::Ok(MyStruct {
            discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::FromZc>::from_zc(
                &self.discriminator,
            )?,
            value: <u8 as pina::pinapod::FromZc>::from_zc(&self.value)?,
        })
    }
}
impl pina::pinapod::FromZc for MyStruct
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
{
    #[inline]
    fn from_zc(
        pod: &Self::Pod,
    ) -> ::core::result::Result<Self, pina::pinapod::PinaPodError> {
        pod.to_native()
    }
}
impl MyStruct
where
    [u8; AccountDisc::BYTES]: pina::pinapod::ZcField,
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
    u8: pina::pinapod::ZcField,
    <u8 as pina::pinapod::ZcField>::Pod: pina::pinapod::ZcElem,
{
    /// Validate and attribute: names the first field whose stored value
    /// a reader would reject.
    ///
    /// Accepts exactly what [`PinaPodFixed::validate_prefix`](pinapod::PinaPodFixed::validate_prefix)
    /// accepts, so a diagnostic run after a rejected read answers
    /// *which field*, never *whether*. A nested schema field is
    /// attributed at its own name; call the nested `diagnose` on the
    /// field's bytes for one level deeper.
    #[inline]
    pub fn diagnose(
        data: &[::core::primitive::u8],
    ) -> ::core::result::Result<(), pina::pinapod::Diagnostic> {
        let size = ::core::mem::size_of::<MyStructZc>();
        if data.len() < size {
            return ::core::result::Result::Err(
                pina::pinapod::Diagnostic::new(
                    "MyStruct",
                    pina::pinapod::PinaPodError::BufferTooSmall,
                ),
            );
        }
        let value: &MyStructZc = unsafe { &*data.as_ptr().cast::<MyStructZc>() };
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.discriminator,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("discriminator", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.value,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("value", error))?;
        ::core::result::Result::Ok(())
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
        == AccountDisc::BYTES + 0usize + ::core::mem::size_of::<::core::primitive::u8>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<MyStructZc>() ==\n    AccountDisc::BYTES + 0usize +\n        ::core::mem::size_of::<::core::primitive::u8>()",
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
            #[cold]
            #[inline(never)]
            fn __pina_header_error(is_size: bool) -> pina::ProgramError {
                if is_size {
                    return pina::PinaProgramError::InvalidAccountSize.into();
                }
                pina::PinaProgramError::InvalidDiscriminator.into()
            }
            return Err(__pina_header_error(data.len() != Self::SIZE));
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
impl pina::PinaValidate for MyStructZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for MyStruct {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <MyStructZc as pina::PinaValidate>::validate(value)
    }
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
impl ::core::cmp::PartialEq for LargeStateZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> ::core::primitive::bool {
        true && self.discriminator == other.discriminator
            && self.authority == other.authority && self.bump == other.bump
            && self.treasury_bump == other.treasury_bump
            && self.mint_bump == other.mint_bump && self.version == other.version
            && self.padding == other.padding && self.total_supply == other.total_supply
            && self.name == other.name
    }
}
impl ::core::cmp::Eq for LargeStateZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::cmp::Eq,
{}
impl ::core::hash::Hash for LargeStateZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::hash::Hash,
{
    fn hash<__State: ::core::hash::Hasher>(&self, state: &mut __State) {
        self.discriminator.hash(state);
        self.authority.hash(state);
        self.bump.hash(state);
        self.treasury_bump.hash(state);
        self.mint_bump.hash(state);
        self.version.hash(state);
        self.padding.hash(state);
        self.total_supply.hash(state);
        self.name.hash(state);
    }
}
impl ::core::fmt::Debug for LargeStateZc
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
    <[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <u8 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <[u8; 3] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <PodU64 as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
    <[u8; 32] as pina::pinapod::ZcField>::Pod: ::core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        formatter
            .debug_struct("LargeState")
            .field("discriminator", &self.discriminator)
            .field("authority", &self.authority)
            .field("bump", &self.bump)
            .field("treasury_bump", &self.treasury_bump)
            .field("mint_bump", &self.mint_bump)
            .field("version", &self.version)
            .field("padding", &self.padding)
            .field("total_supply", &self.total_supply)
            .field("name", &self.name)
            .finish()
    }
}
impl LargeStateZc
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
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    [u8; 3]: pina::pinapod::FromZc,
    PodU64: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
{
    /// Materialize the declared schema type from this stored representation.
    ///
    /// The result is a fully owned, self-contained value: snapshots,
    /// logging, tests, and off-chain decoding all want a shape a
    /// borrowed view cannot serve. For a value that reached the caller
    /// through a validating reader this always succeeds; a stored enum
    /// discriminant naming no variant is the only failure.
    #[inline]
    pub fn to_native(
        &self,
    ) -> ::core::result::Result<LargeState, pina::pinapod::PinaPodError> {
        ::core::result::Result::Ok(LargeState {
            discriminator: <[u8; AccountDisc::BYTES] as pina::pinapod::FromZc>::from_zc(
                &self.discriminator,
            )?,
            authority: <[u8; 32] as pina::pinapod::FromZc>::from_zc(&self.authority)?,
            bump: <u8 as pina::pinapod::FromZc>::from_zc(&self.bump)?,
            treasury_bump: <u8 as pina::pinapod::FromZc>::from_zc(&self.treasury_bump)?,
            mint_bump: <u8 as pina::pinapod::FromZc>::from_zc(&self.mint_bump)?,
            version: <u8 as pina::pinapod::FromZc>::from_zc(&self.version)?,
            padding: <[u8; 3] as pina::pinapod::FromZc>::from_zc(&self.padding)?,
            total_supply: <PodU64 as pina::pinapod::FromZc>::from_zc(
                &self.total_supply,
            )?,
            name: <[u8; 32] as pina::pinapod::FromZc>::from_zc(&self.name)?,
        })
    }
}
impl pina::pinapod::FromZc for LargeState
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
    [u8; AccountDisc::BYTES]: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    u8: pina::pinapod::FromZc,
    [u8; 3]: pina::pinapod::FromZc,
    PodU64: pina::pinapod::FromZc,
    [u8; 32]: pina::pinapod::FromZc,
{
    #[inline]
    fn from_zc(
        pod: &Self::Pod,
    ) -> ::core::result::Result<Self, pina::pinapod::PinaPodError> {
        pod.to_native()
    }
}
impl LargeState
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
    /// Validate and attribute: names the first field whose stored value
    /// a reader would reject.
    ///
    /// Accepts exactly what [`PinaPodFixed::validate_prefix`](pinapod::PinaPodFixed::validate_prefix)
    /// accepts, so a diagnostic run after a rejected read answers
    /// *which field*, never *whether*. A nested schema field is
    /// attributed at its own name; call the nested `diagnose` on the
    /// field's bytes for one level deeper.
    #[inline]
    pub fn diagnose(
        data: &[::core::primitive::u8],
    ) -> ::core::result::Result<(), pina::pinapod::Diagnostic> {
        let size = ::core::mem::size_of::<LargeStateZc>();
        if data.len() < size {
            return ::core::result::Result::Err(
                pina::pinapod::Diagnostic::new(
                    "LargeState",
                    pina::pinapod::PinaPodError::BufferTooSmall,
                ),
            );
        }
        let value: &LargeStateZc = unsafe { &*data.as_ptr().cast::<LargeStateZc>() };
        <<[u8; AccountDisc::BYTES] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.discriminator,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("discriminator", error))?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.authority,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("authority", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.bump,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("bump", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.treasury_bump,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("treasury_bump", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.mint_bump,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("mint_bump", error))?;
        <<u8 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.version,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("version", error))?;
        <<[u8; 3] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.padding,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("padding", error))?;
        <<PodU64 as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.total_supply,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("total_supply", error))?;
        <<[u8; 32] as pina::pinapod::ZcField>::Pod as pina::pinapod::ZcValidate>::validate_ref(
                &value.name,
            )
            .map_err(|error| pina::pinapod::Diagnostic::new("name", error))?;
        ::core::result::Result::Ok(())
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
        == AccountDisc::BYTES + 0usize
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<::core::primitive::u8>()
            + ::core::mem::size_of::<[::core::primitive::u8; 3]>()
            + ::core::mem::size_of::<pina::PodU64>()
            + ::core::mem::size_of::<[::core::primitive::u8; 32]>())
    {
        ::core::panicking::panic(
            "assertion failed: ::core::mem::size_of::<LargeStateZc>() ==\n    AccountDisc::BYTES + 0usize +\n                                    ::core::mem::size_of::<[::core::primitive::u8; 32]>() +\n                                ::core::mem::size_of::<::core::primitive::u8>() +\n                            ::core::mem::size_of::<::core::primitive::u8>() +\n                        ::core::mem::size_of::<::core::primitive::u8>() +\n                    ::core::mem::size_of::<::core::primitive::u8>() +\n                ::core::mem::size_of::<[::core::primitive::u8; 3]>() +\n            ::core::mem::size_of::<pina::PodU64>() +\n        ::core::mem::size_of::<[::core::primitive::u8; 32]>()",
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
            #[cold]
            #[inline(never)]
            fn __pina_header_error(is_size: bool) -> pina::ProgramError {
                if is_size {
                    return pina::PinaProgramError::InvalidAccountSize.into();
                }
                pina::PinaProgramError::InvalidDiscriminator.into()
            }
            return Err(__pina_header_error(data.len() != Self::SIZE));
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
impl pina::PinaValidate for LargeStateZc {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
impl pina::PinaAccount for LargeState {
    fn validate_account_value(value: &Self::Zc) -> pina::ProgramResult {
        <LargeStateZc as pina::PinaValidate>::validate(value)
    }
    fn write_zc_discriminator(value: &mut <Self as pina::PinaPodFixed>::Zc) {
        <Self as pina::HasDiscriminator>::write_discriminator(&mut value.discriminator);
    }
}
