use pina::*;
#[doc(hidden)]
#[allow(dead_code, non_upper_case_globals)]
const __PINA_ENTRYPOINT_MUST_BE_UNIQUE_PER_PROGRAM: () = ();
#[repr(u8)]
#[non_exhaustive]
pub enum CounterInstruction {
    Initialize = 0,
    Increment = 1,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for CounterInstruction {}
#[automatically_derived]
impl ::core::clone::Clone for CounterInstruction {
    #[inline]
    fn clone(&self) -> CounterInstruction {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for CounterInstruction {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for CounterInstruction {}
#[automatically_derived]
impl ::core::cmp::PartialEq for CounterInstruction {
    #[inline]
    fn eq(&self, other: &CounterInstruction) -> bool {
        let __self_discr = ::core::intrinsics::discriminant_value(self);
        let __arg1_discr = ::core::intrinsics::discriminant_value(other);
        __self_discr == __arg1_discr
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for CounterInstruction {
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
impl ::core::convert::From<CounterInstruction> for u8 {
    #[inline]
    fn from(enum_value: CounterInstruction) -> Self {
        enum_value as Self
    }
}
impl ::core::convert::TryFrom<u8> for CounterInstruction {
    type Error = ::pina::ProgramError;
    #[inline]
    fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
        #![allow(non_upper_case_globals)]
        const __INITIALIZE: u8 = 0;
        const __INCREMENT: u8 = 1;
        const _: () = {
            if !(__INITIALIZE != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `Initialize` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        const _: () = {
            if !(__INCREMENT != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `Increment` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        #[deny(unreachable_patterns)]
        match number {
            __INITIALIZE => ::core::result::Result::Ok(Self::Initialize),
            __INCREMENT => ::core::result::Result::Ok(Self::Increment),
            #[allow(unreachable_patterns)]
            _ => {
                ::core::result::Result::Err(
                    ::pina::PinaProgramError::InvalidDiscriminator.into(),
                )
            }
        }
    }
}
const _: () = if !(::core::mem::size_of::<CounterInstruction>()
    == ::core::mem::size_of::<u8>())
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "The size of the enum `CounterInstruction` must match the size of its primitive representation\n\t\t\t\t`u8`.",
            ),
        );
    }
};
impl ::pina::IntoDiscriminator for CounterInstruction {
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
impl CounterInstruction {
    /// Upper bound on the accounts this program reads in one instruction.
    ///
    /// Derived from every instruction's declared `ACCOUNT_BOUND` and
    /// saturated at the entrypoint's account array, so a variant that
    /// declares an unbounded trailing slice cannot inflate the cap.
    ///
    /// This is the count a program declares, not a security boundary. Passing
    /// it to `nostd_entrypoint!` would size the runtime's account array below
    /// the transaction maximum, and the loader *skips* any account beyond that
    /// array instead of failing, so `finish_exact` would no longer reject an
    /// instruction that supplies too many accounts. Keep the entrypoint at its
    /// default maximum and use this constant as the declaration and test
    /// contract it is.
    pub const MAX_INSTRUCTION_ACCOUNTS: usize = {
        const fn maximum(values: [usize; 2]) -> usize {
            let mut index = 0;
            let mut highest = 0;
            while index < values.len() {
                if values[index] > highest {
                    highest = values[index];
                }
                index += 1;
            }
            highest
        }
        const fn clamp(value: usize, limit: usize) -> usize {
            if value > limit { limit } else { value }
        }
        clamp(
            maximum([
                {
                    const fn __pina_account_bound<'a, T>() -> usize
                    where
                        T: ::pina::ParseAccounts<'a>,
                    {
                        <T as ::pina::ParseAccounts<'a>>::ACCOUNT_BOUND
                    }
                    __pina_account_bound::<'static, InitializeAccounts>()
                },
                {
                    const fn __pina_account_bound<'a, T>() -> usize
                    where
                        T: ::pina::ParseAccounts<'a>,
                    {
                        <T as ::pina::ParseAccounts<'a>>::ACCOUNT_BOUND
                    }
                    __pina_account_bound::<'static, IncrementAccounts>()
                },
            ]),
            ::pina::pinocchio::MAX_TX_ACCOUNTS,
        )
    };
    /// Dispatches one instruction to its accounts struct.
    ///
    /// Pass this to `nostd_entrypoint!` as
    /// `nostd_entrypoint!(Self::process_instruction)`. Program-specific behavior beyond
    /// routing belongs in each accounts struct's `ProcessAccountInfos::process`.
    #[inline(always)]
    pub fn process_instruction(
        program_id: &::pina::Address,
        accounts: &mut [::pina::AccountView],
        data: &[u8],
    ) -> ::pina::ProgramResult {
        let instruction: CounterInstruction = ::pina::parse_instruction(
            program_id,
            &ID,
            data,
        )?;
        match instruction {
            CounterInstruction::Initialize => {
                let __pina_accounts = <InitializeAccounts as ::core::convert::TryFrom<
                    (&::pina::Address, &mut [::pina::AccountView]),
                >>::try_from((program_id, accounts))?;
                <InitializeAccounts as ::pina::ProcessAccountInfos>::process(
                    __pina_accounts,
                    data,
                )
            }
            CounterInstruction::Increment => {
                let __pina_accounts = <IncrementAccounts as ::core::convert::TryFrom<
                    (&::pina::Address, &mut [::pina::AccountView]),
                >>::try_from((program_id, accounts))?;
                <IncrementAccounts as ::pina::ProcessAccountInfos>::process(
                    __pina_accounts,
                    data,
                )
            }
        }
    }
}
#[doc(hidden)]
#[allow(dead_code, non_upper_case_globals)]
const __PINA_ENTRYPOINT_MUST_BE_UNIQUE_PER_PROGRAM: () = ();
#[repr(u8)]
#[non_exhaustive]
pub enum OverrideInstruction {
    Routed = 0,
    Untouched = 1,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for OverrideInstruction {}
#[automatically_derived]
impl ::core::clone::Clone for OverrideInstruction {
    #[inline]
    fn clone(&self) -> OverrideInstruction {
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for OverrideInstruction {}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for OverrideInstruction {}
#[automatically_derived]
impl ::core::cmp::PartialEq for OverrideInstruction {
    #[inline]
    fn eq(&self, other: &OverrideInstruction) -> bool {
        let __self_discr = ::core::intrinsics::discriminant_value(self);
        let __arg1_discr = ::core::intrinsics::discriminant_value(other);
        __self_discr == __arg1_discr
    }
}
#[automatically_derived]
impl ::core::cmp::Eq for OverrideInstruction {
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
impl ::core::convert::From<OverrideInstruction> for u8 {
    #[inline]
    fn from(enum_value: OverrideInstruction) -> Self {
        enum_value as Self
    }
}
impl ::core::convert::TryFrom<u8> for OverrideInstruction {
    type Error = ::pina::ProgramError;
    #[inline]
    fn try_from(number: u8) -> ::core::result::Result<Self, ::pina::ProgramError> {
        #![allow(non_upper_case_globals)]
        const __ROUTED: u8 = 0;
        const __UNTOUCHED: u8 = 1;
        const _: () = {
            if !(__ROUTED != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `Routed` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        const _: () = {
            if !(__UNTOUCHED != !0) {
                {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "discriminator value for `Untouched` is the all-ones value reserved by Pina for the framework `Migrate` instruction; choose another value",
                        ),
                    );
                }
            }
        };
        #[deny(unreachable_patterns)]
        match number {
            __ROUTED => ::core::result::Result::Ok(Self::Routed),
            __UNTOUCHED => ::core::result::Result::Ok(Self::Untouched),
            #[allow(unreachable_patterns)]
            _ => {
                ::core::result::Result::Err(
                    ::pina::PinaProgramError::InvalidDiscriminator.into(),
                )
            }
        }
    }
}
const _: () = if !(::core::mem::size_of::<OverrideInstruction>()
    == ::core::mem::size_of::<u8>())
{
    {
        ::core::panicking::panic_fmt(
            format_args!(
                "The size of the enum `OverrideInstruction` must match the size of its primitive representation\n\t\t\t\t`u8`.",
            ),
        );
    }
};
impl ::pina::IntoDiscriminator for OverrideInstruction {
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
impl OverrideInstruction {
    /// Upper bound on the accounts this program reads in one instruction.
    ///
    /// Derived from every instruction's declared `ACCOUNT_BOUND` and
    /// saturated at the entrypoint's account array, so a variant that
    /// declares an unbounded trailing slice cannot inflate the cap.
    ///
    /// This is the count a program declares, not a security boundary. Passing
    /// it to `nostd_entrypoint!` would size the runtime's account array below
    /// the transaction maximum, and the loader *skips* any account beyond that
    /// array instead of failing, so `finish_exact` would no longer reject an
    /// instruction that supplies too many accounts. Keep the entrypoint at its
    /// default maximum and use this constant as the declaration and test
    /// contract it is.
    pub const MAX_INSTRUCTION_ACCOUNTS: usize = {
        const fn maximum(values: [usize; 2]) -> usize {
            let mut index = 0;
            let mut highest = 0;
            while index < values.len() {
                if values[index] > highest {
                    highest = values[index];
                }
                index += 1;
            }
            highest
        }
        const fn clamp(value: usize, limit: usize) -> usize {
            if value > limit { limit } else { value }
        }
        clamp(
            maximum([
                {
                    const fn __pina_account_bound<'a, T>() -> usize
                    where
                        T: ::pina::ParseAccounts<'a>,
                    {
                        <T as ::pina::ParseAccounts<'a>>::ACCOUNT_BOUND
                    }
                    __pina_account_bound::<'static, IncrementAccounts>()
                },
                {
                    const fn __pina_account_bound<'a, T>() -> usize
                    where
                        T: ::pina::ParseAccounts<'a>,
                    {
                        <T as ::pina::ParseAccounts<'a>>::ACCOUNT_BOUND
                    }
                    __pina_account_bound::<'static, UntouchedAccounts>()
                },
            ]),
            ::pina::pinocchio::MAX_TX_ACCOUNTS,
        )
    };
    /// Dispatches one instruction to its accounts struct.
    ///
    /// Pass this to `nostd_entrypoint!` as
    /// `nostd_entrypoint!(Self::process_instruction)`. Program-specific behavior beyond
    /// routing belongs in each accounts struct's `ProcessAccountInfos::process`.
    #[inline(always)]
    pub fn process_instruction(
        program_id: &::pina::Address,
        accounts: &mut [::pina::AccountView],
        data: &[u8],
    ) -> ::pina::ProgramResult {
        let instruction: OverrideInstruction = ::pina::parse_instruction(
            program_id,
            &ID,
            data,
        )?;
        match instruction {
            OverrideInstruction::Routed => {
                let __pina_accounts = <IncrementAccounts as ::core::convert::TryFrom<
                    (&::pina::Address, &mut [::pina::AccountView]),
                >>::try_from((program_id, accounts))?;
                <IncrementAccounts as ::pina::ProcessAccountInfos>::process(
                    __pina_accounts,
                    data,
                )
            }
            OverrideInstruction::Untouched => {
                let __pina_accounts = <UntouchedAccounts as ::core::convert::TryFrom<
                    (&::pina::Address, &mut [::pina::AccountView]),
                >>::try_from((program_id, accounts))?;
                <UntouchedAccounts as ::pina::ProcessAccountInfos>::process(
                    __pina_accounts,
                    data,
                )
            }
        }
    }
}
#[pina(crate = pina)]
pub struct InitializeAccounts<'a> {
    pub authority: &'a AccountView,
    pub counter: &'a mut AccountView,
    pub system_program: &'a AccountView,
}
impl<'a> pina::ParseAccounts<'a> for InitializeAccounts<'a> {
    const ACCOUNT_BOUND: usize = 3usize;
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let authority = cursor.next()?;
        let counter = cursor.next_mut()?;
        let system_program = cursor.next()?;
        Ok(Self {
            authority,
            counter,
            system_program,
        })
    }
    #[inline]
    fn validate_accounts(&self) -> pina::ProgramResult {
        <Self as pina::PinaValidate>::validate(self)
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for InitializeAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        cursor.finish_exact()?;
        <InitializeAccounts<'a> as pina::PinaValidate>::validate(&parsed)?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for InitializeAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
impl<'a> pina::PinaValidate for InitializeAccounts<'a> {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for InitializeAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field3_finish(
            f,
            "InitializeAccounts",
            "authority",
            &self.authority,
            "counter",
            &self.counter,
            "system_program",
            &&self.system_program,
        )
    }
}
#[pina(crate = pina)]
pub struct IncrementAccounts<'a> {
    pub authority: &'a AccountView,
    pub counter: &'a mut AccountView,
}
impl<'a> pina::ParseAccounts<'a> for IncrementAccounts<'a> {
    const ACCOUNT_BOUND: usize = 2usize;
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let authority = cursor.next()?;
        let counter = cursor.next_mut()?;
        Ok(Self { authority, counter })
    }
    #[inline]
    fn validate_accounts(&self) -> pina::ProgramResult {
        <Self as pina::PinaValidate>::validate(self)
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for IncrementAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        cursor.finish_exact()?;
        <IncrementAccounts<'a> as pina::PinaValidate>::validate(&parsed)?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for IncrementAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
impl<'a> pina::PinaValidate for IncrementAccounts<'a> {
    #[inline]
    fn validate(&self) -> pina::ProgramResult {
        Ok(())
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for IncrementAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "IncrementAccounts",
            "authority",
            &self.authority,
            "counter",
            &&self.counter,
        )
    }
}
