#![no_std]

use pina::*;

pub const ID: Address = Address::new_from_array([77; 32]);

nostd_entrypoint!(process_instruction);

#[inline(always)]
fn account<'a>(accounts: &'a [AccountView], index: usize) -> Result<&'a AccountView, ProgramError> {
	accounts.get(index).ok_or(ProgramError::NotEnoughAccountKeys)
}

#[inline(never)]
fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	if program_id != &ID {
		return Err(ProgramError::IncorrectProgramId);
	}

	let target = account(accounts, 0)?;
	match data.first().copied() {
		Some(0) => {
			let mint = target.as_token_mint()?;
			core::hint::black_box(mint.decimals());
		}
		Some(1) => {
			let token_account = target.as_token_account()?;
			core::hint::black_box(token_account.amount());
		}
		Some(2) => {
			let mint = target.as_token_2022_mint()?;
			core::hint::black_box(mint.base.decimals());
		}
		Some(3) => {
			let token_account = target.as_token_2022_account()?;
			core::hint::black_box(token_account.base.amount());
		}
		Some(4) => {
			let wallet = account(accounts, 1)?;
			let mint = account(accounts, 2)?;
			let token_account = target.as_associated_token_account(
				wallet.address(),
				mint.address(),
				&token::ID,
			)?;
			core::hint::black_box(token_account.amount());
		}
		Some(5) => {
			let wallet = account(accounts, 1)?;
			let mint = account(accounts, 2)?;
			let token_account = target.as_associated_token_account(
				wallet.address(),
				mint.address(),
				&token_2022::ID,
			)?;
			core::hint::black_box(token_account.amount());
		}
		Some(6) => {
			target.assert_owner(&token::ID)?;
			let token_account = target.as_token_account()?;
			core::hint::black_box(token_account.amount());
		}
		_ => return Err(ProgramError::InvalidInstructionData),
	}

	Ok(())
}
