#![allow(unsafe_code)]

use core::mem::size_of;

use pina::Address;
use pina::AllocateAccount;
use pina::AllocateAccountWithBump;
use pina::CloseAccount;
use pina::CloseAccountZeroed;
use pina::CpiContext;
use pina::CpiHandle;
use pina::CpiProgramId;
use pina::CreateAccount;
use pina::CreateProgramAccount;
use pina::CreateProgramAccountWithBump;
use pina::IntoDiscriminator;
use pina::PdaSigner;
use pina::Program;
use pina::ProgramError;
#[cfg(feature = "account-resize")]
use pina::ReallocAccount;
#[cfg(feature = "account-resize")]
use pina::ReallocAccountZeroed;
use pina::Seed;
use pina::Signer;
use pina::ToCpiAccounts;
use pina::combine_seeds_with_bump;
use pina::try_find_program_address;
use pinocchio::AccountView;
#[cfg(feature = "account-resize")]
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pinocchio::account::NOT_BORROWED;
use pinocchio::account::RuntimeAccount;
use pinocchio::address::MAX_SEEDS;

#[pina::discriminator(crate = ::pina)]
enum BuilderAccountType {
	BuilderState = 1,
}

#[pina::account(crate = ::pina, discriminator = BuilderAccountType)]
#[allow(dead_code)]
struct BuilderState {
	value: u8,
}

#[test]
fn combine_seeds_with_bump_basic() {
	let seed_a: &[u8] = b"escrow";
	let seed_b: &[u8] = &[1, 2, 3];
	let bump = [42u8; 1];

	let result = combine_seeds_with_bump(&[seed_a, seed_b], &bump)
		.unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], b"escrow");
	assert_eq!(&*result[1], &[1, 2, 3]);
	assert_eq!(&*result[2], &[42]);
	for slot in &result[3..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_single_seed() {
	let seed: &[u8] = b"hello";
	let bump = [0u8; 1];

	let result =
		combine_seeds_with_bump(&[seed], &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], b"hello");
	assert_eq!(&*result[1], &[0]);
	for slot in &result[2..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_empty_seeds() {
	let bump = [255u8; 1];

	let result = combine_seeds_with_bump(&[], &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	assert_eq!(&*result[0], &[255]);
	for slot in &result[1..] {
		assert!(slot.is_empty());
	}
}

#[test]
fn combine_seeds_with_bump_at_max_minus_one() {
	let seeds: Vec<&[u8]> = (0..MAX_SEEDS - 1).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	let result = combine_seeds_with_bump(&seeds, &bump).unwrap_or_else(|e| panic!("failed: {e:?}"));

	for (i, slot) in result.iter().enumerate().take(MAX_SEEDS - 1) {
		assert_eq!(&**slot, &[1u8], "slot {i} should be the original seed");
	}
	assert_eq!(&*result[MAX_SEEDS - 1], &[7]);
}

#[test]
fn combine_seeds_with_bump_too_many_seeds_fails() {
	let seeds: Vec<&[u8]> = (0..MAX_SEEDS).map(|_| &[1u8][..]).collect();
	let bump = [7u8; 1];

	let result = combine_seeds_with_bump(&seeds, &bump);
	assert!(result.is_err());
}

#[test]
fn pda_signer_borrows_owned_seed_array() {
	let seed_a: &[u8] = b"escrow";
	let seed_b: &[u8] = &[9, 8, 7];
	let bump: &[u8] = &[42];
	let signer = PdaSigner::from_slices([seed_a, seed_b, bump]);
	let _borrowed = signer.as_signer();

	assert_eq!(&*signer.as_seeds()[0], b"escrow");
	assert_eq!(&*signer.as_seeds()[1], &[9, 8, 7]);
	assert_eq!(&*signer.as_seeds()[2], &[42]);
}

#[test]
fn pda_signer_from_slice_array_matches_constructor() {
	let seed_a: &[u8] = b"escrow";
	let bump: &[u8] = &[42];
	let signer = PdaSigner::from([seed_a, bump]);

	assert_eq!(&*signer.as_seeds()[0], b"escrow");
	assert_eq!(&*signer.as_seeds()[1], &[42]);
}

#[test]
fn pda_signer_from_seed_array_matches_constructor() {
	let seed_a: &[u8] = b"escrow";
	let bump: &[u8] = &[42];
	let seed_array = [Seed::from(seed_a), Seed::from(bump)];
	let signer = PdaSigner::from(seed_array);

	assert_eq!(&*signer.as_seeds()[0], b"escrow");
	assert_eq!(&*signer.as_seeds()[1], &[42]);
}

#[cfg(feature = "account-resize")]
#[test]
fn max_permitted_data_increase_is_10_kib() {
	assert_eq!(MAX_PERMITTED_DATA_INCREASE, 10_240);
}

#[test]
fn create_account_builder_reaches_rent_lookup() {
	let owner = Address::new_from_array([9u8; 32]);
	let mut stored_from = TestAccount::<0>::new(Address::new_from_array([1u8; 32]), true, true);
	let mut stored_to = TestAccount::<0>::new(Address::new_from_array([2u8; 32]), true, true);
	let from = stored_from.view();
	let to = stored_to.view();

	let result = CreateAccount {
		from: &from,
		to: &to,
		space: 8,
		owner: &owner,
	}
	.invoke();

	assert_eq!(result, Err(ProgramError::UnsupportedSysvar));
}

#[test]
fn program_account_builders_validate_the_target_before_rent_lookup() {
	let owner = Address::new_from_array([5u8; 32]);
	let seeds: &[&[u8]] = &[b"builder"];
	let (address, bump) =
		try_find_program_address(seeds, &owner).unwrap_or_else(|| panic!("expected builder PDA"));
	let mut stored_target = TestAccount::<0>::new(address, false, true);
	stored_target.header.lamports = 0;
	let mut stored_payer = TestAccount::<0>::new(Address::new_from_array([6u8; 32]), true, true);
	let mut target = stored_target.view();
	let payer = stored_payer.view();

	let canonical_result = CreateProgramAccount {
		account: &mut target,
		payer: &payer,
		owner: &owner,
		seeds,
	}
	.invoke::<BuilderState>();
	assert_eq!(canonical_result, Err(ProgramError::UnsupportedSysvar));

	let explicit_result = CreateProgramAccountWithBump {
		account: &mut target,
		payer: &payer,
		owner: &owner,
		seeds,
		bump,
	}
	.invoke::<BuilderState>();
	assert_eq!(explicit_result, Err(ProgramError::UnsupportedSysvar));
}

#[test]
fn close_account_builders_transfer_lamports_and_optionally_clear_data() {
	let mut stored_plain = TestAccount::<8>::new(Address::new_from_array([1u8; 32]), false, true);
	let mut stored_zeroed = TestAccount::<8>::new(Address::new_from_array([2u8; 32]), false, true);
	let mut stored_recipient =
		TestAccount::<0>::new(Address::new_from_array([3u8; 32]), false, true);
	stored_plain.header.lamports = 10;
	stored_zeroed.header.lamports = 20;
	stored_recipient.header.lamports = 5;
	stored_plain.data.fill(7);
	stored_zeroed.data.fill(9);
	let mut plain = stored_plain.view();
	let mut zeroed = stored_zeroed.view();
	let mut recipient = stored_recipient.view();

	CloseAccount {
		account: &mut plain,
		recipient: &mut recipient,
	}
	.invoke()
	.unwrap_or_else(|error| panic!("close plain account: {error:?}"));
	CloseAccountZeroed {
		account: &mut zeroed,
		recipient: &mut recipient,
	}
	.invoke()
	.unwrap_or_else(|error| panic!("close zeroed account: {error:?}"));

	assert_eq!(plain.lamports(), 0);
	assert_eq!(zeroed.lamports(), 0);
	assert_eq!(recipient.lamports(), 35);
	assert_eq!(stored_plain.data, [7; 8]);
	assert_eq!(stored_zeroed.data, [0; 8]);
}

#[cfg(feature = "account-resize")]
#[test]
fn realloc_builders_are_exported() {
	assert!(size_of::<ReallocAccount<'static, 'static, 'static>>() > 0);
	assert!(size_of::<ReallocAccountZeroed<'static, 'static, 'static>>() > 0);
}

#[cfg(feature = "account-resize")]
#[test]
fn realloc_zeroed_builder_accepts_an_unchanged_size() {
	let owner = Address::new_from_array([9u8; 32]);
	let mut stored_account = TestAccount::<8>::new(Address::new_from_array([1u8; 32]), false, true);
	let mut stored_payer = TestAccount::<8>::new(Address::new_from_array([2u8; 32]), true, true);
	let mut account = stored_account.view();
	let mut payer = stored_payer.view();

	let result = ReallocAccountZeroed {
		account: &mut account,
		payer: &mut payer,
		new_size: 8,
		program_id: &owner,
	}
	.invoke();

	assert_eq!(result, Ok(()));
}

#[cfg(feature = "account-resize")]
#[test]
fn realloc_rejects_an_active_borrow_before_moving_rent() {
	let owner = Address::new_from_array([9u8; 32]);
	let mut stored_account = TestAccount::<8>::new(Address::new_from_array([1u8; 32]), false, true);
	let mut stored_payer = TestAccount::<8>::new(Address::new_from_array([2u8; 32]), true, true);
	let mut account = stored_account.view();
	let duplicate = account;
	let mut payer = stored_payer.view();
	let account_lamports = account.lamports();
	let payer_lamports = payer.lamports();
	let data = duplicate
		.try_borrow()
		.unwrap_or_else(|error| panic!("borrow account data: {error:?}"));

	let result = ReallocAccount {
		account: &mut account,
		payer: &mut payer,
		new_size: 16,
		program_id: &owner,
	}
	.invoke();

	assert_eq!(result, Err(ProgramError::AccountBorrowFailed));
	assert_eq!(account.lamports(), account_lamports);
	assert_eq!(payer.lamports(), payer_lamports);
	assert_eq!(account.data_len(), 8);
	drop(data);
}

#[cfg(feature = "account-resize")]
#[test]
fn realloc_rejects_oversized_single_growth_before_moving_rent() {
	let owner = Address::new_from_array([9u8; 32]);
	let mut stored_account = TestAccount::<8>::new(Address::new_from_array([1u8; 32]), false, true);
	let mut stored_payer = TestAccount::<8>::new(Address::new_from_array([2u8; 32]), true, true);
	let mut account = stored_account.view();
	let mut payer = stored_payer.view();
	let account_lamports = account.lamports();
	let payer_lamports = payer.lamports();
	let new_size = account.data_len() + MAX_PERMITTED_DATA_INCREASE + 1;

	let result = ReallocAccount {
		account: &mut account,
		payer: &mut payer,
		new_size,
		program_id: &owner,
	}
	.invoke();

	assert_eq!(result, Err(ProgramError::InvalidRealloc));
	assert_eq!(account.lamports(), account_lamports);
	assert_eq!(payer.lamports(), payer_lamports);
	assert_eq!(account.data_len(), 8);
}

#[cfg(feature = "account-resize")]
#[test]
fn realloc_allows_the_maximum_single_growth_through_preflight() {
	let owner = Address::new_from_array([9u8; 32]);
	let mut stored_account = TestAccount::<8>::new(Address::new_from_array([1u8; 32]), false, true);
	let mut stored_payer = TestAccount::<8>::new(Address::new_from_array([2u8; 32]), true, true);
	let mut account = stored_account.view();
	let mut payer = stored_payer.view();
	let account_lamports = account.lamports();
	let payer_lamports = payer.lamports();
	let new_size = account.data_len() + MAX_PERMITTED_DATA_INCREASE;

	let result = ReallocAccount {
		account: &mut account,
		payer: &mut payer,
		new_size,
		program_id: &owner,
	}
	.invoke();

	// Host tests cannot load the on-chain Rent sysvar. Reaching that error proves
	// the exact runtime growth limit passed the preflight `>` check.
	assert_eq!(result, Err(ProgramError::UnsupportedSysvar));
	assert_eq!(account.lamports(), account_lamports);
	assert_eq!(payer.lamports(), payer_lamports);
	assert_eq!(account.data_len(), 8);
}

#[repr(C)]
struct TestAccount<const N: usize> {
	header: RuntimeAccount,
	data: [u8; N],
}

impl<const N: usize> TestAccount<N> {
	fn new(address: Address, is_signer: bool, is_writable: bool) -> Self {
		Self::new_with_executable(address, is_signer, is_writable, false)
	}

	fn new_with_executable(
		address: Address,
		is_signer: bool,
		is_writable: bool,
		executable: bool,
	) -> Self {
		Self {
			header: RuntimeAccount {
				borrow_state: NOT_BORROWED,
				is_signer: u8::from(is_signer),
				is_writable: u8::from(is_writable),
				executable: u8::from(executable),
				padding: [0; 4],
				address,
				owner: Address::new_from_array([9u8; 32]),
				lamports: 1,
				data_len: N as u64,
			},
			data: [0u8; N],
		}
	}

	fn view(&mut self) -> AccountView {
		unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
	}
}

struct ExampleProgram;

impl CpiProgramId for ExampleProgram {
	const ID: Address = Address::new_from_array([6u8; 32]);
}

#[derive(Clone, Copy)]
struct ExampleAccounts<'a> {
	first: CpiHandle<'a>,
	second: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a, 2> for ExampleAccounts<'a> {
	fn to_cpi_handles(&self) -> [CpiHandle<'a>; 2] {
		[self.first, self.second]
	}
}

#[test]
fn cpi_handle_preserves_writable_and_signer_flags() {
	let mut writable = TestAccount::<8>::new(Address::new_from_array([1u8; 32]), true, true);
	let mut readonly = TestAccount::<8>::new(Address::new_from_array([2u8; 32]), false, false);
	let writable_view = writable.view();
	let readonly_view = readonly.view();

	let writable_handle = CpiHandle::writable_signer(&writable_view)
		.unwrap_or_else(|e| panic!("writable signer handle: {e:?}"));
	let readonly_handle = CpiHandle::readonly(&readonly_view);

	assert!(writable_handle.is_writable());
	assert!(writable_handle.is_signer());
	assert!(!readonly_handle.is_writable());
	assert!(!readonly_handle.is_signer());
	assert_eq!(writable_handle.address(), writable_view.address());
	assert_eq!(readonly_handle.address(), readonly_view.address());
}

#[test]
fn non_signer_cpi_handle_does_not_forward_transaction_signature() {
	let mut signed = TestAccount::<8>::new(Address::new_from_array([8u8; 32]), true, false);
	let signed_view = signed.view();
	let handle = CpiHandle::readonly(&signed_view);

	assert!(!handle.is_signer());
}

#[test]
fn signer_cpi_handles_encode_target_instruction_requirements() {
	let mut account = TestAccount::<8>::new(Address::new_from_array([8u8; 32]), false, true);
	let view = account.view();
	let readonly = CpiHandle::readonly_signer(&view);
	let writable = CpiHandle::writable_signer(&view)
		.unwrap_or_else(|e| panic!("writable signer handle: {e:?}"));

	assert!(readonly.is_signer());
	assert!(!readonly.is_writable());
	assert!(writable.is_signer());
	assert!(writable.is_writable());
}

#[test]
fn cpi_handle_rejects_readonly_writable_requests() {
	let mut readonly = TestAccount::<8>::new(Address::new_from_array([3u8; 32]), false, false);
	let readonly_view = readonly.view();

	let result = CpiHandle::writable(&readonly_view);
	assert!(matches!(result, Err(ProgramError::InvalidAccountData)));
	let signer_result = CpiHandle::writable_signer(&readonly_view);
	assert!(matches!(
		signer_result,
		Err(ProgramError::InvalidAccountData)
	));
}

fn assert_noncanonical_allocation_rejected(lamports: u64) {
	let owner = Address::new_from_array([5u8; 32]);
	let seeds: &[&[u8]] = &[b"state"];
	let (expected, bump) =
		try_find_program_address(seeds, &owner).unwrap_or_else(|| panic!("expected state PDA"));
	let wrong_address = if expected == Address::new_from_array([7u8; 32]) {
		Address::new_from_array([8u8; 32])
	} else {
		Address::new_from_array([7u8; 32])
	};
	let mut target = TestAccount::<0>::new(wrong_address, true, true);
	target.header.lamports = lamports;
	let mut payer = TestAccount::<0>::new(Address::new_from_array([6u8; 32]), true, true);
	let target_view = target.view();
	let payer_view = payer.view();

	let result = AllocateAccountWithBump {
		account: &target_view,
		payer: &payer_view,
		space: 8,
		owner: &owner,
		seeds,
		bump,
	}
	.invoke();
	assert!(matches!(result, Err(ProgramError::InvalidSeeds)));
}

#[test]
fn allocation_rejects_noncanonical_zero_balance_target_before_cpi() {
	assert_noncanonical_allocation_rejected(0);
}

#[test]
fn allocation_rejects_noncanonical_prefunded_target_before_cpi() {
	assert_noncanonical_allocation_rejected(1_000_000);
}

#[test]
fn allocation_accepts_canonical_target_before_rent_lookup() {
	let owner = Address::new_from_array([5u8; 32]);
	let seeds: &[&[u8]] = &[b"state"];
	let (address, bump) =
		try_find_program_address(seeds, &owner).unwrap_or_else(|| panic!("expected state PDA"));
	let mut target = TestAccount::<0>::new(address, false, true);
	target.header.lamports = 0;
	let mut payer = TestAccount::<0>::new(Address::new_from_array([6u8; 32]), true, true);
	let target_view = target.view();
	let payer_view = payer.view();

	let result = AllocateAccountWithBump {
		account: &target_view,
		payer: &payer_view,
		space: 8,
		owner: &owner,
		seeds,
		bump,
	}
	.invoke();
	assert_eq!(result, Err(ProgramError::UnsupportedSysvar));
}

#[test]
fn canonical_allocation_builder_reaches_rent_lookup() {
	let owner = Address::new_from_array([5u8; 32]);
	let seeds: &[&[u8]] = &[b"state"];
	let (address, _) =
		try_find_program_address(seeds, &owner).unwrap_or_else(|| panic!("expected state PDA"));
	let mut target = TestAccount::<0>::new(address, false, true);
	target.header.lamports = 0;
	let mut payer = TestAccount::<0>::new(Address::new_from_array([6u8; 32]), true, true);
	let target_view = target.view();
	let payer_view = payer.view();

	let result = AllocateAccount {
		account: &target_view,
		payer: &payer_view,
		space: 8,
		owner: &owner,
		seeds,
	}
	.invoke();

	assert_eq!(result, Err(ProgramError::UnsupportedSysvar));
}

#[test]
fn canonical_builders_reject_seed_lists_that_cannot_form_a_pda() {
	let owner = Address::new_from_array([5u8; 32]);
	let seed = [1u8];
	let seeds = [&seed[..]; MAX_SEEDS];
	let mut stored_target = TestAccount::<0>::new(Address::new_from_array([7u8; 32]), false, true);
	let mut stored_payer = TestAccount::<0>::new(Address::new_from_array([6u8; 32]), true, true);
	let mut target = stored_target.view();
	let payer = stored_payer.view();

	let create_result = CreateProgramAccount {
		account: &mut target,
		payer: &payer,
		owner: &owner,
		seeds: &seeds,
	}
	.invoke::<BuilderState>();
	assert_eq!(create_result, Err(ProgramError::InvalidSeeds));

	let allocate_result = AllocateAccount {
		account: &target,
		payer: &payer,
		space: 8,
		owner: &owner,
		seeds: &seeds,
	}
	.invoke();
	assert_eq!(allocate_result, Err(ProgramError::InvalidSeeds));
}

#[test]
fn allocation_rejects_more_signers_than_the_runtime_accepts() {
	let owner = Address::new_from_array([5u8; 32]);
	let seeds: &[&[u8]] = &[b"state"];
	let mut target = TestAccount::<0>::new(Address::new_from_array([7u8; 32]), false, true);
	let mut payer = TestAccount::<0>::new(Address::new_from_array([6u8; 32]), true, true);
	let target_view = target.view();
	let payer_view = payer.view();
	let empty_seeds: [Seed<'_>; 0] = [];
	let signer = Signer::from(&empty_seeds);
	let signers: [Signer<'_, '_>; 16] = core::array::from_fn(|_| signer.clone());

	let result = AllocateAccountWithBump {
		account: &target_view,
		payer: &payer_view,
		space: 8,
		owner: &owner,
		seeds,
		bump: 0,
	}
	.invoke_signed(&signers);

	assert_eq!(result, Err(ProgramError::InvalidArgument));
}

#[test]
fn cpi_context_accepts_typed_account_structs() {
	let mut first = TestAccount::<8>::new(Address::new_from_array([4u8; 32]), true, true);
	let mut second = TestAccount::<8>::new(Address::new_from_array([5u8; 32]), false, false);
	let first_view = first.view();
	let second_view = second.view();
	let accounts = ExampleAccounts {
		first: CpiHandle::writable(&first_view).unwrap_or_else(|e| panic!("first handle: {e:?}")),
		second: CpiHandle::readonly(&second_view),
	};
	let mut stored_program =
		TestAccount::<8>::new_with_executable(ExampleProgram::ID, false, false, true);
	let program_view = stored_program.view();
	let program = Program::<ExampleProgram>::new(&program_view)
		.unwrap_or_else(|e| panic!("program validation: {e:?}"));
	let context = CpiContext::new(program, accounts);
	let ordered = context.accounts.to_cpi_handles();

	assert_eq!(ordered[0].address(), first_view.address());
	assert!(ordered[0].is_writable());
	assert_eq!(ordered[1].address(), second_view.address());
	assert!(!ordered[1].is_writable());
	assert_eq!(context.invoke(&[]), Ok(()));
	assert_eq!(context.invoke_signed(&[], &[]), Ok(()));
}

#[test]
fn program_wrapper_validates_executable_program_address() {
	let mut stored_program =
		TestAccount::<8>::new_with_executable(ExampleProgram::ID, false, false, true);
	let program_view = stored_program.view();
	let program = Program::<ExampleProgram>::new(&program_view)
		.unwrap_or_else(|e| panic!("program validation: {e:?}"));
	let copied_program = program;
	let cloned_program = program.clone();
	let debug_output = format!("{program:?}");

	assert_eq!(program.address(), &ExampleProgram::ID);
	assert_eq!(program.account().address(), &ExampleProgram::ID);
	assert_eq!(copied_program.address(), &ExampleProgram::ID);
	assert_eq!(cloned_program.address(), &ExampleProgram::ID);
	assert!(debug_output.contains("Program"));
}

#[test]
fn program_wrapper_rejects_wrong_address() {
	let wrong_address = Address::new_from_array([7u8; 32]);
	let mut stored_program =
		TestAccount::<8>::new_with_executable(wrong_address, false, false, true);
	let program_view = stored_program.view();
	let error = Program::<ExampleProgram>::new(&program_view)
		.expect_err("wrong program address should fail validation");

	assert_eq!(error, ProgramError::InvalidAccountData);
}
