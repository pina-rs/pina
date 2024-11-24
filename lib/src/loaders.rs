use bytemuck::Pod;
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    AccountDeserialize, AccountInfoValidation, AsAccount, CloseAccount, Discriminator,
    LamportTransfer,
};

impl AccountInfoValidation for AccountInfo<'_> {
    fn is_signer(&self) -> Result<&Self, ProgramError> {
        if !self.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        Ok(self)
    }

    fn is_writable(&self) -> Result<&Self, ProgramError> {
        if !self.is_writable {
            return Err(ProgramError::MissingRequiredSignature);
        }

        Ok(self)
    }

    fn is_executable(&self) -> Result<&Self, ProgramError> {
        if !self.executable {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(self)
    }

    fn is_empty(&self) -> Result<&Self, ProgramError> {
        if !self.data_is_empty() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        Ok(self)
    }

    fn is_program(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
        self.has_address(program_id)?.is_executable()
    }

    fn is_type<T: Discriminator>(&self, program_id: &Pubkey) -> Result<&Self, ProgramError> {
        self.has_owner(program_id)?;
        let data = self.try_borrow_data()?;
        let data_len = 8 + std::mem::size_of::<T>();

        if data[0].ne(&T::discriminator()) {
            return Err(ProgramError::InvalidAccountData);
        }

        if data.len() != data_len {
            return Err(ProgramError::AccountDataTooSmall);
        }

        Ok(self)
    }

    fn has_owner(&self, owner: &Pubkey) -> Result<&Self, ProgramError> {
        if self.owner.ne(owner) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        Ok(self)
    }

    fn has_address(&self, address: &Pubkey) -> Result<&Self, ProgramError> {
        if self.key.ne(&address) {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(self)
    }

    fn has_seeds(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<&Self, ProgramError> {
        let pda = Pubkey::find_program_address(seeds, program_id);
        if self.key.ne(&pda.0) {
            return Err(ProgramError::InvalidSeeds);
        }

        Ok(self)
    }

    fn is_sysvar(&self, sysvar_id: &Pubkey) -> Result<&Self, ProgramError> {
        self.has_owner(&solana_program::sysvar::ID)?
            .has_address(sysvar_id)
    }
}

impl AsAccount for AccountInfo<'_> {
    fn as_account<T>(&self, program_id: &Pubkey) -> Result<&T, ProgramError>
    where
        T: AccountDeserialize + Discriminator + Pod,
    {
        unsafe {
            self.has_owner(program_id)?;
            T::try_from_bytes(std::slice::from_raw_parts(
                self.try_borrow_data()?.as_ptr(),
                8 + std::mem::size_of::<T>(),
            ))
        }
    }

    fn as_account_mut<T>(&self, program_id: &Pubkey) -> Result<&mut T, ProgramError>
    where
        T: AccountDeserialize + Discriminator + Pod,
    {
        unsafe {
            self.has_owner(program_id)?;
            T::try_from_bytes_mut(std::slice::from_raw_parts_mut(
                self.try_borrow_mut_data()?.as_mut_ptr(),
                8 + std::mem::size_of::<T>(),
            ))
        }
    }
}

impl<'a, 'info> LamportTransfer<'a, 'info> for AccountInfo<'info> {
    #[inline(always)]
    fn send(&'a self, lamports: u64, to: &'a AccountInfo<'info>) {
        **self.lamports.borrow_mut() -= lamports;
        **to.lamports.borrow_mut() += lamports;
    }

    #[inline(always)]
    fn collect(&'a self, lamports: u64, from: &'a AccountInfo<'info>) -> Result<(), ProgramError> {
        solana_program::program::invoke(
            &solana_program::system_instruction::transfer(from.key, self.key, lamports),
            &[from.clone(), self.clone()],
        )
    }
}

impl<'a, 'info> CloseAccount<'a, 'info> for AccountInfo<'info> {
    fn close(&'a self, to: &'a AccountInfo<'info>) -> Result<(), ProgramError> {
        // Realloc data to zero.
        self.realloc(0, true)?;

        // Return rent lamports.
        self.send(self.lamports(), to);

        Ok(())
    }
}

