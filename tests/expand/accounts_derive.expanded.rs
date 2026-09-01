use pina::*;
#[pina(crate = pina)]
pub struct InitAccounts<'a> {
    pub payer: &'a AccountView,
    pub config: &'a AccountView,
    pub system_program: &'a AccountView,
}
impl<'a> pina::ParseAccounts<'a> for InitAccounts<'a> {
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let payer = cursor.next()?;
        let config = cursor.next()?;
        let system_program = cursor.next()?;
        Ok(Self {
            payer,
            config,
            system_program,
        })
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for InitAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        cursor.finish_exact()?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for InitAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for InitAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field3_finish(
            f,
            "InitAccounts",
            "payer",
            &self.payer,
            "config",
            &self.config,
            "system_program",
            &&self.system_program,
        )
    }
}
#[pina(crate = pina)]
pub struct TransferAccounts<'a> {
    pub authority: &'a AccountView,
    pub source: &'a AccountView,
    pub destination: &'a AccountView,
    #[pina(remaining)]
    pub extra: &'a [AccountView],
}
impl<'a> pina::ParseAccounts<'a> for TransferAccounts<'a> {
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let authority = cursor.next()?;
        let source = cursor.next()?;
        let destination = cursor.next()?;
        let extra = cursor.take_remaining();
        Ok(Self {
            authority,
            source,
            destination,
            extra,
        })
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for TransferAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for TransferAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for TransferAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field4_finish(
            f,
            "TransferAccounts",
            "authority",
            &self.authority,
            "source",
            &self.source,
            "destination",
            &self.destination,
            "extra",
            &&self.extra,
        )
    }
}
#[pina(crate = pina)]
pub struct MutableTransferAccounts<'a> {
    pub authority: &'a mut AccountView,
    #[pina(remaining)]
    pub extra: &'a mut [AccountView],
}
impl<'a> pina::ParseAccounts<'a> for MutableTransferAccounts<'a> {
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let authority = cursor.next_mut()?;
        let extra = cursor.remaining_mut_distinct()?;
        Ok(Self { authority, extra })
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for MutableTransferAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for MutableTransferAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for MutableTransferAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "MutableTransferAccounts",
            "authority",
            &self.authority,
            "extra",
            &&self.extra,
        )
    }
}
#[pina(crate = pina)]
pub struct DuplicateMutableRemainingAccounts<'a> {
    pub authority: &'a AccountView,
    /// Duplicate accounts represent repeated weights in this instruction.
    #[pina(remaining, distinct = false)]
    pub extra: &'a mut [AccountView],
}
impl<'a> pina::ParseAccounts<'a> for DuplicateMutableRemainingAccounts<'a> {
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let authority = cursor.next()?;
        let extra = cursor.remaining_mut()?;
        Ok(Self { authority, extra })
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for DuplicateMutableRemainingAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for DuplicateMutableRemainingAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for DuplicateMutableRemainingAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "DuplicateMutableRemainingAccounts",
            "authority",
            &self.authority,
            "extra",
            &&self.extra,
        )
    }
}
#[pina(crate = pina)]
pub struct SingleAccount<'a> {
    pub account: &'a AccountView,
}
impl<'a> pina::ParseAccounts<'a> for SingleAccount<'a> {
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let account = cursor.next()?;
        Ok(Self { account })
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for SingleAccount<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        cursor.finish_exact()?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for SingleAccount<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for SingleAccount<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "SingleAccount",
            "account",
            &&self.account,
        )
    }
}
#[pina(crate = pina)]
pub struct EscrowAccounts<'a> {
    pub maker: &'a AccountView,
    pub escrow: &'a AccountView,
    pub mint_a: &'a AccountView,
    pub mint_b: &'a AccountView,
    pub maker_ata_a: &'a AccountView,
    pub vault: &'a AccountView,
    pub token_program: &'a AccountView,
    pub associated_token_program: &'a AccountView,
    pub system_program: &'a AccountView,
}
impl<'a> pina::ParseAccounts<'a> for EscrowAccounts<'a> {
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let maker = cursor.next()?;
        let escrow = cursor.next()?;
        let mint_a = cursor.next()?;
        let mint_b = cursor.next()?;
        let maker_ata_a = cursor.next()?;
        let vault = cursor.next()?;
        let token_program = cursor.next()?;
        let associated_token_program = cursor.next()?;
        let system_program = cursor.next()?;
        Ok(Self {
            maker,
            escrow,
            mint_a,
            mint_b,
            maker_ata_a,
            vault,
            token_program,
            associated_token_program,
            system_program,
        })
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for EscrowAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        cursor.finish_exact()?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for EscrowAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for EscrowAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        let names: &'static _ = &[
            "maker",
            "escrow",
            "mint_a",
            "mint_b",
            "maker_ata_a",
            "vault",
            "token_program",
            "associated_token_program",
            "system_program",
        ];
        let values: &[&dyn ::core::fmt::Debug] = &[
            &self.maker,
            &self.escrow,
            &self.mint_a,
            &self.mint_b,
            &self.maker_ata_a,
            &self.vault,
            &self.token_program,
            &self.associated_token_program,
            &&self.system_program,
        ];
        ::core::fmt::Formatter::debug_struct_fields_finish(
            f,
            "EscrowAccounts",
            names,
            values,
        )
    }
}
pub struct DefaultCrateAccounts<'a> {
    pub authority: &'a AccountView,
    pub data: &'a AccountView,
}
impl<'a> ::pina::ParseAccounts<'a> for DefaultCrateAccounts<'a> {
    fn parse_accounts(
        cursor: &mut ::pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, ::pina::ProgramError> {
        let authority = cursor.next()?;
        let data = cursor.next()?;
        Ok(Self { authority, data })
    }
}
impl<'a> ::pina::TryFromAccountInfos<'a> for DefaultCrateAccounts<'a> {
    fn try_from_account_infos(
        program_id: &::pina::Address,
        accounts: &'a mut [::pina::AccountView],
    ) -> ::core::result::Result<Self, ::pina::ProgramError> {
        let mut cursor = ::pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as ::pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        cursor.finish_exact()?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a ::pina::Address, &'a mut [::pina::AccountView])>
for DefaultCrateAccounts<'a> {
    type Error = ::pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a ::pina::Address, &'a mut [::pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as ::pina::TryFromAccountInfos>::try_from_account_infos(
            program_id,
            accounts,
        )
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for DefaultCrateAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "DefaultCrateAccounts",
            "authority",
            &self.authority,
            "data",
            &&self.data,
        )
    }
}
#[pina(crate = pina)]
pub struct MakeAccounts<'a> {
    pub maker: &'a mut AccountView,
    pub escrow: Option<&'a mut AccountView>,
    pub witness: Option<&'a AccountView>,
    pub system_program: &'a AccountView,
}
impl<'a> pina::ParseAccounts<'a> for MakeAccounts<'a> {
    fn parse_accounts(
        cursor: &mut pina::AccountsCursor<'a>,
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let maker = cursor.next_mut()?;
        let escrow = cursor.next_mut_opt()?;
        let witness = cursor.next_opt()?;
        let system_program = cursor.next()?;
        Ok(Self {
            maker,
            escrow,
            witness,
            system_program,
        })
    }
}
impl<'a> pina::TryFromAccountInfos<'a> for MakeAccounts<'a> {
    fn try_from_account_infos(
        program_id: &pina::Address,
        accounts: &'a mut [pina::AccountView],
    ) -> ::core::result::Result<Self, pina::ProgramError> {
        let mut cursor = pina::AccountsCursor::new(*program_id, accounts);
        let parsed = <Self as pina::ParseAccounts>::parse_accounts(&mut cursor)?;
        cursor.finish_exact()?;
        Ok(parsed)
    }
}
impl<'a> ::core::convert::TryFrom<(&'a pina::Address, &'a mut [pina::AccountView])>
for MakeAccounts<'a> {
    type Error = pina::ProgramError;
    fn try_from(
        (program_id, accounts): (&'a pina::Address, &'a mut [pina::AccountView]),
    ) -> ::core::result::Result<Self, Self::Error> {
        <Self as pina::TryFromAccountInfos>::try_from_account_infos(program_id, accounts)
    }
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for MakeAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field4_finish(
            f,
            "MakeAccounts",
            "maker",
            &self.maker,
            "escrow",
            &self.escrow,
            "witness",
            &self.witness,
            "system_program",
            &&self.system_program,
        )
    }
}
#[pina(crate = pina)]
pub struct BadAccounts<'a> {
    pub payer: &'a AccountView,
    pub weird: Option<u8>,
}
#[automatically_derived]
impl<'a> ::core::fmt::Debug for BadAccounts<'a> {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "BadAccounts",
            "payer",
            &self.payer,
            "weird",
            &&self.weird,
        )
    }
}
