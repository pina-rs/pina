//! Fast Kani proofs over Pina's pure arithmetic and byte-level boundaries.

#![allow(unsafe_code)]

use crate::Address;
use crate::CloseAccountWithRecipient;
use crate::CpiHandle;
use crate::IntoDiscriminator;
use crate::LamportTransfer;
#[cfg(feature = "account-resize")]
use crate::MAX_PERMITTED_DATA_INCREASE;
use crate::ProgramError;
#[cfg(feature = "account-resize")]
use crate::ReallocPlan;
#[cfg(feature = "account-resize")]
use crate::RentAdjustment;
use crate::impls::checked_close_balance;
use crate::impls::checked_send_balances;
use crate::pinocchio::AccountView;
use crate::pinocchio::account::NOT_BORROWED;
use crate::pinocchio::account::RuntimeAccount;
use crate::transaction;

#[cfg(feature = "compact")]
mod compact;
#[cfg(feature = "derive")]
mod zero_copy;

#[kani::proof]
fn quick_discriminator_u8_roundtrips() {
	let value: u8 = kani::any();
	let mut bytes = [0u8; 1];
	value.write_discriminator(&mut bytes);

	assert_eq!(u8::discriminator_from_bytes(&bytes), Ok(value));
	assert!(value.matches_discriminator(&bytes));
}

#[kani::proof]
fn quick_discriminator_u16_roundtrips() {
	let value: u16 = kani::any();
	let mut bytes = [0u8; 2];
	value.write_discriminator(&mut bytes);

	assert_eq!(u16::discriminator_from_bytes(&bytes), Ok(value));
	assert!(value.matches_discriminator(&bytes));
}

#[kani::proof]
fn quick_discriminator_u32_roundtrips() {
	let value: u32 = kani::any();
	let mut bytes = [0u8; 4];
	value.write_discriminator(&mut bytes);

	assert_eq!(u32::discriminator_from_bytes(&bytes), Ok(value));
	assert!(value.matches_discriminator(&bytes));
}

#[kani::proof]
fn quick_discriminator_u64_roundtrips() {
	let value: u64 = kani::any();
	let mut bytes = [0u8; 8];
	value.write_discriminator(&mut bytes);

	assert_eq!(u64::discriminator_from_bytes(&bytes), Ok(value));
	assert!(value.matches_discriminator(&bytes));
}

#[kani::proof]
fn quick_discriminator_parsers_reject_every_short_length() {
	let bytes: [u8; 8] = kani::any();
	let len: usize = kani::any();
	kani::assume(len < bytes.len());
	let data = &bytes[..len];

	if len < u8::BYTES {
		assert_eq!(
			u8::discriminator_from_bytes(data),
			Err(ProgramError::InvalidInstructionData)
		);
	}
	if len < u16::BYTES {
		assert_eq!(
			u16::discriminator_from_bytes(data),
			Err(ProgramError::InvalidInstructionData)
		);
		assert!(!kani::any::<u16>().matches_discriminator(data));
	}
	if len < u32::BYTES {
		assert_eq!(
			u32::discriminator_from_bytes(data),
			Err(ProgramError::InvalidInstructionData)
		);
		assert!(!kani::any::<u32>().matches_discriminator(data));
	}
	assert_eq!(
		u64::discriminator_from_bytes(data),
		Err(ProgramError::InvalidInstructionData)
	);
	assert!(!kani::any::<u64>().matches_discriminator(data));
}

#[kani::proof]
fn quick_discriminator_matching_agrees_with_parsing() {
	let bytes: [u8; 8] = kani::any();
	let expected: u64 = kani::any();
	let parsed = u64::discriminator_from_bytes(&bytes);

	assert_eq!(
		expected.matches_discriminator(&bytes),
		parsed == Ok(expected)
	);
}

#[kani::proof]
fn quick_checked_send_conserves_lamports_or_rejects_without_a_result() {
	let sender: u64 = kani::any();
	let recipient: u64 = kani::any();
	let lamports: u64 = kani::any();
	let result = checked_send_balances(sender, recipient, lamports);

	match result {
		Ok((new_sender, new_recipient)) => {
			assert_eq!(new_sender, sender - lamports);
			assert_eq!(new_recipient, recipient + lamports);
			assert_eq!(
				u128::from(new_sender) + u128::from(new_recipient),
				u128::from(sender) + u128::from(recipient)
			);
		}
		Err(ProgramError::InsufficientFunds) => assert!(sender < lamports),
		Err(ProgramError::ArithmeticOverflow) => {
			assert!(sender >= lamports);
			assert!(recipient.checked_add(lamports).is_none());
		}
		Err(error) => panic!("unexpected transfer error: {error:?}"),
	}
}

#[kani::proof]
fn quick_checked_close_conserves_lamports_or_rejects_overflow() {
	let sender: u64 = kani::any();
	let recipient: u64 = kani::any();
	let result = checked_close_balance(sender, recipient);

	match result {
		Ok(new_recipient) => {
			assert_eq!(new_recipient, sender + recipient);
			assert_eq!(
				u128::from(new_recipient),
				u128::from(sender) + u128::from(recipient)
			);
		}
		Err(ProgramError::ArithmeticOverflow) => {
			assert!(recipient.checked_add(sender).is_none());
		}
		Err(error) => panic!("unexpected close error: {error:?}"),
	}
}

#[cfg(feature = "account-resize")]
#[kani::proof]
fn quick_realloc_plan_rejects_only_oversized_growth() {
	let current_size: usize = kani::any();
	let target_size: usize = kani::any();
	let current_lamports: u64 = kani::any();
	let minimum_balance: u64 = kani::any();
	let result = ReallocPlan::try_new(current_size, target_size, current_lamports, minimum_balance);
	let oversized = target_size
		.checked_sub(current_size)
		.is_some_and(|growth| growth > MAX_PERMITTED_DATA_INCREASE);

	assert_eq!(result.is_err(), oversized);
}

#[cfg(feature = "account-resize")]
#[kani::proof]
fn quick_unchanged_realloc_never_transfers_lamports() {
	let size: usize = kani::any();
	let current_lamports: u64 = kani::any();
	let minimum_balance: u64 = kani::any();
	let result = ReallocPlan::try_new(size, size, current_lamports, minimum_balance);

	assert_eq!(
		result,
		Ok(ReallocPlan {
			target_size: size,
			adjustment: RentAdjustment::None,
		})
	);
}

#[cfg(feature = "account-resize")]
#[kani::proof]
fn quick_realloc_plan_direction_and_amount_are_exact() {
	let current_size: usize = kani::any();
	let target_size: usize = kani::any();
	let current_lamports: u64 = kani::any();
	let minimum_balance: u64 = kani::any();
	let result = ReallocPlan::try_new(current_size, target_size, current_lamports, minimum_balance);

	let Ok(plan) = result else {
		assert!(target_size > current_size);
		return;
	};

	match plan.adjustment {
		RentAdjustment::Fund { lamports } => {
			assert!(target_size > current_size);
			assert!(minimum_balance > current_lamports);
			assert_eq!(lamports, minimum_balance - current_lamports);
			assert_eq!(current_lamports + lamports, minimum_balance);
		}
		RentAdjustment::Refund { lamports } => {
			assert!(target_size < current_size);
			assert!(current_lamports > minimum_balance);
			assert_eq!(lamports, current_lamports - minimum_balance);
			assert_eq!(current_lamports - lamports, minimum_balance);
		}
		RentAdjustment::None => {
			assert!(
				target_size == current_size
					|| current_lamports == minimum_balance
					|| (target_size > current_size && current_lamports > minimum_balance)
					|| (target_size < current_size && current_lamports < minimum_balance)
			);
		}
	}
}

#[kani::proof]
fn quick_instruction_metadata_helpers_preserve_address_and_flags() {
	let address = Address::new_from_array(kani::any());

	assert_eq!(
		transaction::writable_signer(&address),
		(address, true, true)
	);
	assert_eq!(transaction::writable(&address), (address, false, true));
	assert_eq!(
		transaction::readonly_signer(&address),
		(address, true, false)
	);
	assert_eq!(transaction::readonly(&address), (address, false, false));
}

#[repr(C)]
struct StoredAccount {
	header: RuntimeAccount,
	data: [u8; 1],
}

impl StoredAccount {
	fn new(address: Address) -> Self {
		Self {
			header: RuntimeAccount {
				borrow_state: NOT_BORROWED,
				is_signer: 0,
				is_writable: 1,
				executable: 0,
				padding: [0; 4],
				address,
				owner: Address::new_from_array([0; 32]),
				lamports: 0,
				data_len: 1,
			},
			data: [0],
		}
	}

	fn view(&mut self) -> AccountView {
		// SAFETY: `StoredAccount` is `repr(C)` and its one-byte data region
		// immediately follows a header whose `data_len` is exactly one.
		unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
	}
}

#[kani::proof]
fn quick_direct_self_transfer_is_rejected_without_mutation() {
	let address = Address::new_from_array(kani::any());
	let sender_lamports: u64 = kani::any();
	let recipient_lamports: u64 = kani::any();
	let lamports: u64 = kani::any();
	let mut sender = StoredAccount::new(address);
	let mut recipient = StoredAccount::new(address);
	sender.header.lamports = sender_lamports;
	recipient.header.lamports = recipient_lamports;
	let mut sender_view = sender.view();
	let mut recipient_view = recipient.view();

	assert_eq!(
		sender_view.send(lamports, &mut recipient_view),
		Err(ProgramError::InvalidArgument)
	);
	assert_eq!(sender_view.lamports(), sender_lamports);
	assert_eq!(recipient_view.lamports(), recipient_lamports);
}

#[kani::proof]
fn quick_close_to_self_is_rejected_without_mutation() {
	let address = Address::new_from_array(kani::any());
	let sender_lamports: u64 = kani::any();
	let recipient_lamports: u64 = kani::any();
	let mut sender = StoredAccount::new(address);
	let mut recipient = StoredAccount::new(address);
	sender.header.lamports = sender_lamports;
	recipient.header.lamports = recipient_lamports;
	let mut sender_view = sender.view();
	let mut recipient_view = recipient.view();

	assert_eq!(
		sender_view.close_with_recipient(&mut recipient_view),
		Err(ProgramError::InvalidArgument)
	);
	assert_eq!(sender_view.lamports(), sender_lamports);
	assert_eq!(recipient_view.lamports(), recipient_lamports);
}

#[kani::proof]
fn quick_cpi_handle_conversion_preserves_order_and_privileges() {
	let first_address = Address::new_from_array(kani::any());
	let second_address = Address::new_from_array(kani::any());
	let third_address = Address::new_from_array(kani::any());
	let mut first = StoredAccount::new(first_address);
	let mut second = StoredAccount::new(second_address);
	let mut third = StoredAccount::new(third_address);
	let first_view = first.view();
	let second_view = second.view();
	let third_view = third.view();
	let handles = [
		CpiHandle::writable_signer(&first_view)
			.unwrap_or_else(|error| panic!("writable fixture rejected: {error:?}")),
		CpiHandle::writable(&second_view)
			.unwrap_or_else(|error| panic!("writable fixture rejected: {error:?}")),
		CpiHandle::readonly(&third_view),
	];
	let accounts = handles.map(CpiHandle::instruction_account);

	assert_eq!(*accounts[0].address, first_address);
	assert!(accounts[0].is_writable);
	assert!(accounts[0].is_signer);
	assert_eq!(*accounts[1].address, second_address);
	assert!(accounts[1].is_writable);
	assert!(!accounts[1].is_signer);
	assert_eq!(*accounts[2].address, third_address);
	assert!(!accounts[2].is_writable);
	assert!(!accounts[2].is_signer);
}
