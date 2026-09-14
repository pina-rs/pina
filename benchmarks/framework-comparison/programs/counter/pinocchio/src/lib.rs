//! Hand-written pinocchio counter, mirroring the pina `counter_program`
//! semantics: PDA seeded by `b"counter" + authority`, 10-byte account
//! (discriminator, bump, count), `initialize` + `increment`.
#![no_std]

use pinocchio::account::AccountView;
use pinocchio::address::Address;
use pinocchio::error::ProgramError;
use pinocchio::error::ProgramResult;
use pinocchio::instruction::cpi::Seed;
use pinocchio::instruction::cpi::Signer;
use pinocchio::sysvars::Sysvar;
use pinocchio_system::instructions::CreateAccount;

pinocchio::program_entrypoint!(process_instruction);
pinocchio::no_allocator!();
pinocchio::nostd_panic_handler!();

/// Space: 1-byte discriminator + 1-byte bump + 8-byte count.
const COUNTER_SPACE: u64 = 10;
const SEED_COUNTER: &[u8] = b"counter";
/// Account discriminator written as the first byte of the counter account.
const ACCOUNT_DISCRIMINATOR: u8 = 1;
const SYSTEM_PROGRAM: Address = Address::new_from_array([0u8; 32]);

fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let (&discriminator, rest) = data
		.split_first()
		.ok_or(ProgramError::InvalidInstructionData)?;
	match discriminator {
		0 => initialize(program_id, accounts, rest),
		1 => increment(program_id, accounts),
		_ => Err(ProgramError::InvalidInstructionData),
	}
}

fn initialize(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
	let [authority, counter, system_program] = accounts else {
		return Err(ProgramError::NotEnoughAccountKeys);
	};
	let &bump = data.first().ok_or(ProgramError::InvalidInstructionData)?;

	// Validate accounts.
	if !authority.is_signer() {
		return Err(ProgramError::MissingRequiredSignature);
	}
	if !counter.is_data_empty() {
		return Err(ProgramError::AccountAlreadyInitialized);
	}
	if system_program.address() != &SYSTEM_PROGRAM {
		return Err(ProgramError::InvalidArgument);
	}

	// Create the PDA account via CPI.
	let bump_seed = [bump];
	let seeds = [
		Seed::from(SEED_COUNTER),
		Seed::from(authority.address().as_ref()),
		Seed::from(&bump_seed[..]),
	];
	CreateAccount {
		from: authority,
		to: &mut *counter,
		lamports: pinocchio::sysvars::rent::Rent::get()?.try_minimum_balance(10)?,
		space: COUNTER_SPACE,
		owner: program_id,
	}
	.invoke_signed(&[Signer::from(&seeds)])?;

	// Write initial state.
	let mut data = counter.try_borrow_mut()?;
	data[0] = ACCOUNT_DISCRIMINATOR;
	data[1] = bump;
	data[2..10].copy_from_slice(&0u64.to_le_bytes());

	solana_program_log::log("Counter initialized");
	Ok(())
}

fn increment(program_id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
	let [authority, counter] = accounts else {
		return Err(ProgramError::NotEnoughAccountKeys);
	};
	if !authority.is_signer() {
		return Err(ProgramError::MissingRequiredSignature);
	}
	if !counter.owned_by(program_id) {
		return Err(ProgramError::IncorrectProgramId);
	}
	let counter_address = counter.address();

	// Read the stored state.
	let (bump, count) = {
		let data = counter.try_borrow()?;
		if data[0] != ACCOUNT_DISCRIMINATOR {
			return Err(ProgramError::InvalidAccountData);
		}
		let bump = data[1];
		let count = u64::from_le_bytes(data[2..10].try_into().unwrap());
		(bump, count)
	};

	// Re-derive the PDA to confirm the account matches the authority's seeds.
	let derived = Address::create_program_address(
		&[SEED_COUNTER, authority.address().as_ref(), &[bump]],
		program_id,
	)
	.map_err(|_| ProgramError::InvalidSeeds)?;
	if counter_address != &derived {
		return Err(ProgramError::InvalidSeeds);
	}

	let next = count
		.checked_add(1)
		.ok_or(ProgramError::ArithmeticOverflow)?;

	let mut data = counter.try_borrow_mut()?;
	data[2..10].copy_from_slice(&next.to_le_bytes());

	solana_program_log::log("Counter incremented");
	Ok(())
}
