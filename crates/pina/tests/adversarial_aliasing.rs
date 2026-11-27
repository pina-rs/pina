#![allow(unsafe_code, missing_docs)]

//! Adversarial security and parity suite for order-independent account aliasing,
//! `#[pina(unique)]`, Anchor v2 sentinel-bug immunity, nested/non-static layouts,
//! and `dispatch_entrypoint!` duplicate-witness fast paths.

use core::cell::RefCell;
use core::mem::size_of;
use std::vec::Vec;

use pina::*;
use pinocchio::SUCCESS;
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE;
use pinocchio::entrypoint::NON_DUP_MARKER;

declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator(entrypoint)]
pub enum AliasInstruction {
	MutPair = 0,
	MutThenRo = 1,
	RoThenMut = 2,
	ThreeWayRoMutMut = 3,
	MutRoMut = 4,
	UniqueRoFirst = 5,
	UniqueRoSecond = 6,
	UniqueMutFirst = 7,
	UniqueMutSecond = 8,
	OptionalMutPair = 9,
	UniqueOptionalPair = 10,
	UniqueOptionalWithRo = 11,
	MutWithImmutableRemaining = 12,
	UniqueMutWithImmutableRemaining = 13,
	UniqueMutWithPassthroughRemaining = 14,
	NestedMutPair = 15,
	NestedMutRo = 16,
	NestedUniqueChild = 17,
	NestedRemainingMut = 18,
	DynamicWithManualParser = 19,
	BorrowGuardCheck = 20,
	ManualTryFrom = 21,
	MutTriple = 22,
	MutQuad = 23,
	ReqAndOptionalMut = 24,
	TwoReqAndOptMut = 25,
	#[dispatch(accounts = MutPairAccounts)]
	SharedMutPairA = 26,
	#[dispatch(accounts = MutPairAccounts)]
	SharedMutPairB = 27,
	UniqueQuad = 28,
}

#[discriminator]
pub enum TestAccountType {
	VaultState = 1,
}

#[account(discriminator = TestAccountType)]
pub struct VaultState {
	pub amount: u64,
}

#[derive(Accounts)]
pub struct MutPairAccounts<'a> {
	pub first: &'a mut AccountView,
	pub second: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct MutTripleAccounts<'a> {
	pub first: &'a mut AccountView,
	pub second: &'a mut AccountView,
	pub third: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct MutQuadAccounts<'a> {
	pub observer: &'a AccountView,
	pub first: &'a mut AccountView,
	pub second: &'a mut AccountView,
	pub third: &'a mut AccountView,
	pub fourth: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct MutThenRoAccounts<'a> {
	pub mutated: &'a mut AccountView,
	pub observed: &'a AccountView,
}

#[derive(Accounts)]
pub struct RoThenMutAccounts<'a> {
	pub observed: &'a AccountView,
	pub mutated: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct ThreeWayRoMutMutAccounts<'a> {
	pub observed: &'a AccountView,
	pub first: &'a mut AccountView,
	pub second: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct MutRoMutAccounts<'a> {
	pub first: &'a mut AccountView,
	pub observed: &'a AccountView,
	pub second: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct UniqueRoFirstAccounts<'a> {
	#[pina(unique)]
	pub first: &'a AccountView,
	pub second: &'a AccountView,
}

#[derive(Accounts)]
pub struct UniqueRoSecondAccounts<'a> {
	pub first: &'a AccountView,
	#[pina(unique)]
	pub second: &'a AccountView,
}

#[derive(Accounts)]
pub struct UniqueMutFirstAccounts<'a> {
	#[pina(unique)]
	pub first: &'a mut AccountView,
	pub second: &'a AccountView,
}

#[derive(Accounts)]
pub struct UniqueMutSecondAccounts<'a> {
	pub first: &'a AccountView,
	#[pina(unique)]
	pub second: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct OptionalMutPairAccounts<'a> {
	pub authority: &'a AccountView,
	pub opt_a: Option<&'a mut AccountView>,
	pub opt_b: Option<&'a mut AccountView>,
}

#[derive(Accounts)]
pub struct ReqAndOptionalMutAccounts<'a> {
	pub authority: &'a AccountView,
	pub req_mut: &'a mut AccountView,
	pub opt_a: Option<&'a mut AccountView>,
	pub opt_b: Option<&'a mut AccountView>,
}

#[derive(Accounts)]
pub struct TwoReqAndOptMutAccounts<'a> {
	pub req_a: &'a mut AccountView,
	pub req_b: &'a mut AccountView,
	pub opt_c: Option<&'a mut AccountView>,
}

#[derive(Accounts)]
pub struct UniqueQuadAccounts<'a> {
	pub ro_slot: &'a AccountView,
	#[pina(unique)]
	pub unique_ro: &'a AccountView,
	pub mut_a: &'a mut AccountView,
	pub mut_b: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct UniqueOptionalPairAccounts<'a> {
	pub authority: &'a AccountView,
	#[pina(unique)]
	pub opt_a: Option<&'a mut AccountView>,
	#[pina(unique)]
	pub opt_b: Option<&'a AccountView>,
}

#[derive(Accounts)]
pub struct UniqueOptionalWithRoAccounts<'a> {
	pub program_slot: &'a AccountView,
	#[pina(unique)]
	pub opt: Option<&'a AccountView>,
}

#[derive(Accounts)]
pub struct MutWithImmutableRemainingAccounts<'a> {
	pub target: &'a mut AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
}

#[derive(Accounts)]
pub struct UniqueMutWithImmutableRemainingAccounts<'a> {
	#[pina(unique)]
	pub target: &'a mut AccountView,
	#[pina(remaining)]
	pub rest: &'a [AccountView],
}

#[derive(Accounts)]
pub struct UniqueMutWithPassthroughRemainingAccounts<'a> {
	#[pina(unique)]
	pub target: &'a mut AccountView,
	#[pina(remaining, distinct = false)]
	pub rest: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct NestedMutChild<'a> {
	pub inner: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct NestedMutPairAccounts<'a> {
	pub outer: &'a mut AccountView,
	pub child: NestedMutChild<'a>,
}

#[derive(Accounts)]
pub struct NestedRoChild<'a> {
	pub inner: &'a AccountView,
}

#[derive(Accounts)]
pub struct NestedMutRoAccounts<'a> {
	pub outer: &'a mut AccountView,
	pub child: NestedRoChild<'a>,
}

#[derive(Accounts)]
pub struct UniqueRoChild<'a> {
	#[pina(unique)]
	pub inner: &'a AccountView,
}

#[derive(Accounts)]
pub struct NestedUniqueChildAccounts<'a> {
	pub outer: &'a AccountView,
	pub child: UniqueRoChild<'a>,
}

#[derive(Accounts)]
pub struct NestedRemainingChild<'a> {
	#[pina(remaining)]
	pub rest: &'a mut [AccountView],
}

#[derive(Accounts)]
pub struct NestedRemainingMutAccounts<'a> {
	pub outer: &'a mut AccountView,
	pub child: NestedRemainingChild<'a>,
}

/// Hand-written `ParseAccounts` implementation that inherits
/// `__AliasLayout::DYNAMIC` (fail-closed).
pub struct ManualMutableSlot<'a> {
	pub slot: &'a mut AccountView,
}

impl<'a> ParseAccounts<'a> for ManualMutableSlot<'a> {
	fn parse_accounts(cursor: &mut AccountsCursor<'a>) -> Result<Self, ProgramError> {
		Ok(Self {
			slot: cursor.next_mut()?,
		})
	}
}

#[derive(Accounts)]
pub struct DynamicWithManualParserAccounts<'a> {
	pub manual: ManualMutableSlot<'a>,
	pub second: &'a mut AccountView,
}

#[derive(Accounts)]
pub struct BorrowGuardCheckAccounts<'a> {
	pub mutated: &'a mut AccountView,
	pub observed: &'a AccountView,
}

std::thread_local! {
	static LAST_SEEN: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

#[expect(
	clippy::unnecessary_wraps,
	reason = "test helper matching ProcessAccountInfos::process"
)]
fn record(accounts: &[&AccountView]) -> ProgramResult {
	LAST_SEEN.set(Some(
		accounts
			.iter()
			.map(|account| account.address().as_ref()[0])
			.collect(),
	));
	Ok(())
}

impl<'a> ProcessAccountInfos<'a> for MutPairAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for MutTripleAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.second, self.third])
	}
}

impl<'a> ProcessAccountInfos<'a> for MutQuadAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[
			self.observer,
			self.first,
			self.second,
			self.third,
			self.fourth,
		])
	}
}

impl<'a> ProcessAccountInfos<'a> for MutThenRoAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.mutated, self.observed])
	}
}

impl<'a> ProcessAccountInfos<'a> for RoThenMutAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.observed, self.mutated])
	}
}

impl<'a> ProcessAccountInfos<'a> for ThreeWayRoMutMutAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.observed, self.first, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for MutRoMutAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.observed, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueRoFirstAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueRoSecondAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueMutFirstAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueMutSecondAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for OptionalMutPairAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![self.authority];
		if let Some(a) = self.opt_a {
			out.push(&*a);
		}
		if let Some(b) = self.opt_b {
			out.push(&*b);
		}
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for ReqAndOptionalMutAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![self.authority, &*self.req_mut];
		if let Some(a) = self.opt_a {
			out.push(&*a);
		}
		if let Some(b) = self.opt_b {
			out.push(&*b);
		}
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for TwoReqAndOptMutAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![&*self.req_a, &*self.req_b];
		if let Some(c) = self.opt_c {
			out.push(&*c);
		}
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueQuadAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.ro_slot, self.unique_ro, &*self.mut_a, &*self.mut_b])
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueOptionalPairAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![self.authority];
		if let Some(a) = self.opt_a {
			out.push(&*a);
		}
		if let Some(b) = self.opt_b {
			out.push(b);
		}
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueOptionalWithRoAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![self.program_slot];
		if let Some(opt) = self.opt {
			out.push(opt);
		}
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for MutWithImmutableRemainingAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![&*self.target];
		out.extend(self.rest);
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueMutWithImmutableRemainingAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![&*self.target];
		out.extend(self.rest);
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for UniqueMutWithPassthroughRemainingAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![&*self.target];
		for view in &*self.rest {
			out.push(view);
		}
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for NestedMutPairAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.outer, self.child.inner])
	}
}

impl<'a> ProcessAccountInfos<'a> for NestedMutRoAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.outer, self.child.inner])
	}
}

impl<'a> ProcessAccountInfos<'a> for NestedUniqueChildAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.outer, self.child.inner])
	}
}

impl<'a> ProcessAccountInfos<'a> for NestedRemainingMutAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		let mut out = std::vec![&*self.outer];
		for view in &*self.child.rest {
			out.push(view);
		}
		record(&out)
	}
}

impl<'a> ProcessAccountInfos<'a> for DynamicWithManualParserAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.manual.slot, self.second])
	}
}

impl<'a> ProcessAccountInfos<'a> for BorrowGuardCheckAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		// While a mutable typed guard is alive, attempting an immutable borrow
		// through the aliased readonly view MUST fail with `AccountBorrowFailed`.
		{
			let mut vault = self.mutated.as_account_mut::<VaultState>(&ID)?;
			assert_eq!(
				self.observed.as_account::<VaultState>(&ID).err(),
				Some(ProgramError::AccountBorrowFailed)
			);
			vault.amount.set(99);
		}

		// Once the mutable guard is dropped, the aliased readonly view reads the
		// updated state safely.
		let observed_vault = self.observed.as_account::<VaultState>(&ID)?;
		assert_eq!(observed_vault.amount.get(), 99);
		record(&[self.mutated, self.observed])
	}
}

/// Manual `TryFromAccountInfos` implementation that inherits the default
/// fail-closed `try_from_route` fallback.
pub struct ManualTryFromAccounts<'a> {
	pub first: &'a mut AccountView,
	pub second: &'a mut AccountView,
}

impl<'a> ParseAccounts<'a> for ManualTryFromAccounts<'a> {
	fn parse_accounts(cursor: &mut AccountsCursor<'a>) -> Result<Self, ProgramError> {
		let first = cursor.next_mut()?;
		let second = cursor.next_mut()?;
		cursor.finish_exact()?;
		Ok(Self { first, second })
	}
}

impl<'a> TryFromAccountInfos<'a> for ManualTryFromAccounts<'a> {
	fn try_from_account_infos(
		program_id: &Address,
		accounts: &'a mut [AccountView],
	) -> Result<Self, ProgramError> {
		let mut cursor = AccountsCursor::new(*program_id, accounts);
		Self::parse_accounts(&mut cursor)
	}
}

impl<'a> TryFrom<(&'a Address, &'a mut [AccountView])> for ManualTryFromAccounts<'a> {
	type Error = ProgramError;

	fn try_from(
		(program_id, accounts): (&'a Address, &'a mut [AccountView]),
	) -> Result<Self, Self::Error> {
		if accounts
			.first()
			.is_some_and(|first| first.address().as_ref()[0] == 0xFE)
		{
			return Err(ProgramError::Custom(0x7777));
		}
		Self::try_from_account_infos(program_id, accounts)
	}
}

impl<'a> ProcessAccountInfos<'a> for ManualTryFromAccounts<'a> {
	fn process(self, _data: &[u8]) -> ProgramResult {
		record(&[self.first, self.second])
	}
}

dispatch_entrypoint!(AliasInstruction);

#[derive(Clone)]
enum Slot {
	Account {
		address: Address,
		writable: bool,
		data: Vec<u8>,
	},
	Duplicate(u8),
}

fn addr(byte: u8) -> Address {
	let mut bytes = [0_u8; 32];
	bytes[0] = byte;
	Address::new_from_array(bytes)
}

fn rw(byte: u8) -> Slot {
	Slot::Account {
		address: addr(byte),
		writable: true,
		data: Vec::new(),
	}
}

fn ro(byte: u8) -> Slot {
	Slot::Account {
		address: addr(byte),
		writable: false,
		data: Vec::new(),
	}
}

fn program_sentinel(writable: bool) -> Slot {
	Slot::Account {
		address: ID,
		writable,
		data: Vec::new(),
	}
}

fn vault_account(byte: u8, amount: u64) -> Slot {
	let mut data = std::vec![0_u8; size_of::<<VaultState as PinaPodFixed>::Zc>()];
	VaultState::initialize(&mut data, |state| {
		state.amount.set(amount);
		Ok(())
	})
	.unwrap_or_else(|e| panic!("init vault: {e:?}"));
	Slot::Account {
		address: addr(byte),
		writable: true,
		data,
	}
}

fn serialize(slots: &[Slot], data: &[u8], program_id: &Address) -> (Vec<u64>, usize) {
	let mut total_bytes = size_of::<u64>();
	for slot in slots {
		match slot {
			Slot::Account { data, .. } => {
				let padded = (data.len() + MAX_PERMITTED_DATA_INCREASE).next_multiple_of(8);
				total_bytes += 88 + padded + size_of::<u64>();
			}
			Slot::Duplicate(_) => total_bytes += size_of::<u64>(),
		}
	}
	let data_len_offset = total_bytes;
	let data_offset = data_len_offset + size_of::<u64>();
	total_bytes = data_offset + data.len() + size_of::<Address>();

	let mut words = std::vec![0_u64; total_bytes.div_ceil(size_of::<u64>())];
	let base = words.as_mut_ptr().cast::<u8>();
	// SAFETY: `words` is allocated for `total_bytes` aligned to 8 bytes and all
	// offsets below stay strictly within `0..total_bytes`.
	unsafe {
		let write_slice = |cursor: &mut usize, src: &[u8]| {
			core::ptr::copy_nonoverlapping(src.as_ptr(), base.add(*cursor), src.len());
			*cursor += src.len();
		};
		let mut cursor = 0_usize;
		write_slice(&mut cursor, &(slots.len() as u64).to_le_bytes());

		for slot in slots {
			match slot {
				Slot::Account {
					address,
					writable,
					data,
				} => {
					let mut header = [0_u8; 88];
					header[0] = NON_DUP_MARKER;
					header[2] = u8::from(*writable);
					header[8..40].copy_from_slice(address.as_ref());
					header[40..72].copy_from_slice(program_id.as_ref());
					header[80..88].copy_from_slice(&(data.len() as u64).to_le_bytes());
					write_slice(&mut cursor, &header);
					write_slice(&mut cursor, data);
					let padded =
						(data.len() + MAX_PERMITTED_DATA_INCREASE).next_multiple_of(8) - data.len();
					cursor += padded + size_of::<u64>();
				}
				Slot::Duplicate(index) => {
					write_slice(&mut cursor, &[*index, 0, 0, 0, 0, 0, 0, 0]);
				}
			}
		}

		debug_assert_eq!(cursor, data_len_offset);
		write_slice(&mut cursor, &(data.len() as u64).to_le_bytes());
		debug_assert_eq!(cursor, data_offset);
		write_slice(&mut cursor, data);
		write_slice(&mut cursor, program_id.as_ref());
		debug_assert_eq!(cursor, total_bytes);
	}

	(words, data_offset)
}

fn run_dispatch(ix: AliasInstruction, slots: &[Slot]) -> (u64, Option<Vec<u8>>) {
	let data = [ix as u8];
	let (mut input, offset) = serialize(slots, &data, &ID);
	let ptr = input.as_mut_ptr().cast::<u8>();
	// SAFETY: `ptr` holds a complete loader input and `offset` points to `data`.
	let status = unsafe { entrypoint(ptr, ptr.add(offset)) };
	(status, LAST_SEEN.take())
}

fn run_direct(ix: AliasInstruction, slots: &[Slot]) -> Result<Option<Vec<u8>>, ProgramError> {
	let data = [ix as u8];
	let (mut input, _offset) = serialize(slots, &data, &ID);
	let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 16];
	// SAFETY: `input` is a valid loader input with at most 16 accounts.
	let (program_id, count, ix_data) =
		unsafe { entrypoint::deserialize::<16>(input.as_mut_ptr().cast(), &mut views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let accounts =
		unsafe { core::slice::from_raw_parts_mut(views.as_mut_ptr().cast::<AccountView>(), count) };
	AliasInstruction::process_instruction(program_id, accounts, ix_data)?;
	Ok(LAST_SEEN.take())
}

fn dup_err() -> u64 {
	u64::from(ProgramError::from(
		PinaProgramError::DuplicateMutableAccount,
	))
}

fn invalid_data_err() -> u64 {
	u64::from(ProgramError::InvalidAccountData)
}

fn assert_both_reject(ix: AliasInstruction, slots: &[Slot], expected: &ProgramError) {
	assert_eq!(run_dispatch(ix, slots), (u64::from(expected.clone()), None));
	assert_eq!(run_direct(ix, slots), Err(expected.clone()));
}

fn assert_both_accept(ix: AliasInstruction, slots: &[Slot], expected: Vec<u8>) {
	assert_eq!(run_dispatch(ix, slots), (SUCCESS, Some(expected.clone())));
	assert_eq!(run_direct(ix, slots), Ok(Some(expected)));
}

#[test]
fn symmetric_mut_mut_duplicates_are_rejected_in_every_position() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	// Distinct mutable pair succeeds.
	assert_both_accept(AliasInstruction::MutPair, &[rw(1), rw(2)], std::vec![1, 2]);
	// Aliased mutable pair fails on both dispatch and direct paths.
	assert_both_reject(
		AliasInstruction::MutPair,
		&[rw(1), Slot::Duplicate(0)],
		&dup,
	);

	// Three-way: ro + mut + mut with mutable alias fails.
	assert_both_reject(
		AliasInstruction::ThreeWayRoMutMut,
		&[ro(1), rw(2), Slot::Duplicate(1)],
		&dup,
	);
	// Three-way: ro + mut + mut where ro aliases only one mut succeeds.
	assert_both_accept(
		AliasInstruction::ThreeWayRoMutMut,
		&[rw(1), Slot::Duplicate(0), rw(2)],
		std::vec![1, 1, 2],
	);
	// Three-way: mut + ro + mut with non-adjacent mutable alias fails, while
	// distinct accounts succeed.
	assert_both_accept(
		AliasInstruction::MutRoMut,
		&[rw(1), ro(2), rw(3)],
		std::vec![1, 2, 3],
	);
	assert_both_reject(
		AliasInstruction::MutRoMut,
		&[rw(1), ro(2), Slot::Duplicate(0)],
		&dup,
	);
	// Truncated slice on 2-mut fast path reports `NotEnoughAccountKeys`, and
	// read-only aliased 2-mut reports `InvalidAccountData` before duplicate.
	assert_both_reject(
		AliasInstruction::MutPair,
		&[rw(1)],
		&ProgramError::NotEnoughAccountKeys,
	);
	assert_both_reject(
		AliasInstruction::MutPair,
		&[ro(1), Slot::Duplicate(0)],
		&ProgramError::InvalidAccountData,
	);

	// 3-mut inline fast path: distinct succeeds; every duplicate pair rejects;
	// non-writable precedence holds even when later slots alias.
	assert_both_accept(
		AliasInstruction::MutTriple,
		&[rw(1), rw(2), rw(3)],
		std::vec![1, 2, 3],
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[rw(1), Slot::Duplicate(0), rw(3)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[rw(1), rw(2), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[rw(1), rw(2), Slot::Duplicate(1)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[ro(1), Slot::Duplicate(0), rw(3)],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[ro(1), rw(2), Slot::Duplicate(1)],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[rw(1), ro(2), Slot::Duplicate(1)],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[rw(1), rw(2)],
		&ProgramError::NotEnoughAccountKeys,
	);
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[rw(1)],
		&ProgramError::NotEnoughAccountKeys,
	);

	// 4-mut path (`validate_mut_aliasing_64`) with a leading readonly slot:
	// distinct succeeds, duplicate rejects, and non-writable precedence rejects
	// with `InvalidAccountData`.
	assert_both_accept(
		AliasInstruction::MutQuad,
		&[ro(9), rw(1), rw(2), rw(3), rw(4)],
		std::vec![9, 1, 2, 3, 4],
	);
	assert_both_reject(
		AliasInstruction::MutQuad,
		&[ro(9), rw(1), rw(2), rw(3), Slot::Duplicate(2)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::MutQuad,
		&[ro(9), rw(1), ro(2), rw(3), Slot::Duplicate(2)],
		&ProgramError::InvalidAccountData,
	);
}

#[test]
fn readonly_and_mutable_alias_is_symmetric_and_protected_by_borrow_guards() {
	// Both field orders (`mut + ro` and `ro + mut`) accept an aliased account.
	assert_both_accept(
		AliasInstruction::MutThenRo,
		&[rw(1), Slot::Duplicate(0)],
		std::vec![1, 1],
	);
	assert_both_accept(
		AliasInstruction::RoThenMut,
		&[rw(1), Slot::Duplicate(0)],
		std::vec![1, 1],
	);

	// Mutable + immutable remaining accepts an aliased account symmetrically.
	assert_both_accept(
		AliasInstruction::MutWithImmutableRemaining,
		&[rw(1), ro(2), Slot::Duplicate(0)],
		std::vec![1, 2, 1],
	);

	// Attempting simultaneous mutable + immutable data borrows through aliased
	// views triggers Pinocchio's runtime `AccountBorrowFailed`, then succeeds
	// once the mutable guard drops.
	assert_both_accept(
		AliasInstruction::BorrowGuardCheck,
		&[vault_account(7, 42), Slot::Duplicate(0)],
		std::vec![7, 7],
	);
}

#[test]
fn pina_unique_rejects_every_aliasing_combination_regardless_of_order() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	assert_both_accept(
		AliasInstruction::UniqueRoFirst,
		&[ro(1), ro(2)],
		std::vec![1, 2],
	);
	assert_both_reject(
		AliasInstruction::UniqueRoFirst,
		&[ro(1), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_accept(
		AliasInstruction::UniqueRoSecond,
		&[ro(1), ro(2)],
		std::vec![1, 2],
	);
	assert_both_reject(
		AliasInstruction::UniqueRoSecond,
		&[ro(1), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_accept(
		AliasInstruction::UniqueMutFirst,
		&[rw(1), ro(2)],
		std::vec![1, 2],
	);
	assert_both_reject(
		AliasInstruction::UniqueMutFirst,
		&[rw(1), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_accept(
		AliasInstruction::UniqueMutSecond,
		&[ro(1), rw(2)],
		std::vec![1, 2],
	);
	assert_both_reject(
		AliasInstruction::UniqueMutSecond,
		&[rw(1), Slot::Duplicate(0)],
		&dup,
	);

	// `#[pina(unique)]` rejects aliases inside immutable remaining slices, while
	// distinct accounts succeed.
	assert_both_accept(
		AliasInstruction::UniqueMutWithImmutableRemaining,
		&[rw(1), ro(2), ro(3)],
		std::vec![1, 2, 3],
	);
	assert_both_reject(
		AliasInstruction::UniqueMutWithImmutableRemaining,
		&[rw(1), ro(2), Slot::Duplicate(0)],
		&dup,
	);

	// `#[pina(unique)]` rejects aliases inside `remaining(distinct = false)` too,
	// while still allowing intra-slice duplicates among remaining accounts.
	assert_both_reject(
		AliasInstruction::UniqueMutWithPassthroughRemaining,
		&[rw(1), rw(2), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_accept(
		AliasInstruction::UniqueMutWithPassthroughRemaining,
		&[rw(1), rw(2), Slot::Duplicate(1)],
		std::vec![1, 2, 2],
	);
}

#[test]
fn anchor_v2_sentinel_bug_immunity_omitted_optionals_never_conflict() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	// Present distinct optional accounts succeed on all optional routes.
	assert_both_accept(
		AliasInstruction::OptionalMutPair,
		&[ro(1), rw(2), rw(3)],
		std::vec![1, 2, 3],
	);
	assert_both_accept(
		AliasInstruction::UniqueOptionalPair,
		&[ro(1), rw(2), ro(3)],
		std::vec![1, 2, 3],
	);
	assert_both_accept(
		AliasInstruction::UniqueOptionalWithRo,
		&[ro(1), ro(2)],
		std::vec![1, 2],
	);

	// Two omitted `Option<&mut AccountView>` slots passed via SVM duplicate
	// marker on `program_id` MUST succeed as `(None, None)`.
	assert_both_accept(
		AliasInstruction::OptionalMutPair,
		&[ro(1), program_sentinel(false), Slot::Duplicate(1)],
		std::vec![1],
	);

	// Same even when the sentinel slot happens to be marked writable.
	assert_both_accept(
		AliasInstruction::OptionalMutPair,
		&[ro(1), program_sentinel(true), Slot::Duplicate(1)],
		std::vec![1],
	);

	// Two omitted `#[pina(unique)] Option<&mut>` / `Option<&>` slots MUST also
	// succeed as `(None, None)`.
	assert_both_accept(
		AliasInstruction::UniqueOptionalPair,
		&[ro(1), program_sentinel(false), Slot::Duplicate(1)],
		std::vec![1],
	);

	// An omitted `#[pina(unique)] Option<&>` (`program_id`) does NOT conflict
	// with a non-optional readonly account whose address is `program_id`.
	assert_both_accept(
		AliasInstruction::UniqueOptionalWithRo,
		&[program_sentinel(false), Slot::Duplicate(0)],
		std::vec![ID.as_ref()[0]],
	);

	// Present optional accounts still enforce mut+mut and unique rules.
	assert_both_reject(
		AliasInstruction::OptionalMutPair,
		&[ro(1), rw(2), Slot::Duplicate(1)],
		&dup,
	);
	// Read-only duplicates on present optional mutable slots skip static
	// aliasing checks (`DUP_WRITABLE_MASK == 0`) and still fail with
	// `InvalidAccountData` during `next_mut_opt` writability enforcement.
	assert_both_reject(
		AliasInstruction::OptionalMutPair,
		&[ro(1), ro(2), Slot::Duplicate(1)],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::UniqueOptionalPair,
		&[ro(1), rw(2), Slot::Duplicate(1)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::UniqueOptionalPair,
		&[rw(1), Slot::Duplicate(0), rw(2)],
		&dup,
	);

	// Adversarial spoofing: passing `program_id` twice to non-optional `&mut`
	// slots MUST be rejected (sentinel only applies to `Option` fields).
	assert_both_reject(
		AliasInstruction::MutPair,
		&[program_sentinel(true), Slot::Duplicate(0)],
		&dup,
	);
}

#[test]
fn nested_structs_compose_aliasing_and_unique_masks_correctly() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	assert_both_reject(
		AliasInstruction::NestedMutPair,
		&[rw(1), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_accept(
		AliasInstruction::NestedMutPair,
		&[rw(1), rw(2)],
		std::vec![1, 2],
	);

	// Nested `mut + ro` is allowed symmetrically.
	assert_both_accept(
		AliasInstruction::NestedMutRo,
		&[rw(1), Slot::Duplicate(0)],
		std::vec![1, 1],
	);

	// Nested `#[pina(unique)]` propagates to the outer struct.
	assert_both_reject(
		AliasInstruction::NestedUniqueChild,
		&[ro(1), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_accept(
		AliasInstruction::NestedUniqueChild,
		&[ro(1), ro(2)],
		std::vec![1, 2],
	);

	// Nested trailing mutable remaining rejects both cross-boundary and
	// intra-slice duplicates.
	assert_both_reject(
		AliasInstruction::NestedRemainingMut,
		&[rw(1), rw(2), Slot::Duplicate(0)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::NestedRemainingMut,
		&[rw(1), rw(2), Slot::Duplicate(1)],
		&dup,
	);
	assert_both_accept(
		AliasInstruction::NestedRemainingMut,
		&[rw(1), rw(2), rw(3)],
		std::vec![1, 2, 3],
	);
}

#[test]
fn non_static_dynamic_layout_fails_closed_on_duplicates_and_passes_on_unique_inputs() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	const {
		assert!(!__AliasLayout::DYNAMIC.needs_alias_check());
		assert!(
			!<DynamicWithManualParserAccounts<'static> as ParseAccounts<'static>>::ALIAS_LAYOUT
				.static_layout
		);
	}

	// Unique accounts succeed through both dispatch fast path and direct path.
	assert_both_accept(
		AliasInstruction::DynamicWithManualParser,
		&[rw(1), rw(2)],
		std::vec![1, 2],
	);

	// Aliased mutable accounts fail closed on both paths.
	assert_both_reject(
		AliasInstruction::DynamicWithManualParser,
		&[rw(1), Slot::Duplicate(0)],
		&dup,
	);

	// Read-only duplicates on dynamic mutable slots still fail with
	// `InvalidAccountData` on both paths.
	assert_both_reject(
		AliasInstruction::DynamicWithManualParser,
		&[ro(1), Slot::Duplicate(0)],
		&ProgramError::InvalidAccountData,
	);

	// Manual `TryFromAccountInfos` inherits the default fail-closed
	// `try_from_route` fallback.
	assert_both_accept(
		AliasInstruction::ManualTryFrom,
		&[rw(1), rw(2)],
		std::vec![1, 2],
	);
	assert_both_reject(
		AliasInstruction::ManualTryFrom,
		&[rw(1), Slot::Duplicate(0)],
		&dup,
	);
}

#[test]
fn error_precedence_reports_non_writable_before_duplicate_and_rejects_forward_markers() {
	// Duplicate of a readonly account in two mutable slots reports
	// `InvalidAccountData` (not writable) before `DuplicateMutableAccount`.
	assert_both_reject(
		AliasInstruction::MutPair,
		&[ro(1), Slot::Duplicate(0)],
		&ProgramError::InvalidAccountData,
	);

	// Readonly account in slot 0 aliased into mutable slot 1 reports
	// `InvalidAccountData`.
	assert_both_reject(
		AliasInstruction::RoThenMut,
		&[ro(1), Slot::Duplicate(0)],
		&ProgramError::InvalidAccountData,
	);

	// Non-writable optional mutable duplicate reports `InvalidAccountData`.
	assert_both_reject(
		AliasInstruction::OptionalMutPair,
		&[ro(1), ro(2), Slot::Duplicate(1)],
		&ProgramError::InvalidAccountData,
	);

	// Malformed forward duplicate marker is rejected during loader walk.
	assert_eq!(
		run_dispatch(AliasInstruction::MutPair, &[rw(1), Slot::Duplicate(1)]),
		(invalid_data_err(), None)
	);

	// Direct `try_from_account_infos` with extra trailing accounts respects
	// `ACCOUNT_LIMIT` during aliasing validation and rejects with
	// `TooManyAccountKeys`.
	let (mut input, _) = serialize(&[rw(1), rw(2), rw(3)], &[0], &ID);
	let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
	// SAFETY: `input` is a valid loader input with 3 accounts.
	let (program_id, count, _) =
		unsafe { entrypoint::deserialize::<4>(input.as_mut_ptr().cast(), &mut views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let accounts =
		unsafe { core::slice::from_raw_parts_mut(views.as_mut_ptr().cast::<AccountView>(), count) };
	assert_eq!(
		MutPairAccounts::try_from_account_infos(program_id, accounts).err(),
		Some(PinaProgramError::TooManyAccountKeys.into())
	);
}

#[test]
fn mask256_helpers_and_direct_cursor_remaining_mut_distinct() {
	let mut mut_mask = [0_u64; 4];
	mut_mask = __mask256_with_bit(mut_mask, 65);
	mut_mask = __mask256_with_bit(mut_mask, 69);
	assert_eq!(__mask256_count_ones(mut_mask), 2);
	assert!(__mask256_any(mut_mask));

	// Shifted composition across word boundary (bit 1 shifted by 64 -> bit 65).
	let shifted = __mask256_or_shifted([0; 4], __mask256_with_bit([0; 4], 1), 64);
	assert_eq!(shifted[1], 1_u64 << 1);

	let layout = __AliasLayout {
		static_layout: true,
		mut_mask,
		optional_mask: [0; 4],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	assert!(layout.needs_alias_check());
	assert!(!layout.is_simple_mut_64());
	assert!(!__AliasLayout::DYNAMIC.needs_alias_check());

	let single_mut = __AliasLayout {
		static_layout: true,
		mut_mask: [0b0001, 0, 0, 0],
		optional_mask: [0; 4],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	assert!(!single_mut.needs_alias_check());

	let unique_only = __AliasLayout {
		mut_mask: [0; 4],
		unique_mask: [0b0001, 0, 0, 0],
		..single_mut
	};
	assert!(unique_only.needs_alias_check());

	let remaining_distinct_only = __AliasLayout {
		mut_mask: [0; 4],
		remaining_mut_start: 0,
		remaining_mut_distinct: true,
		..single_mut
	};
	assert!(remaining_distinct_only.needs_alias_check());

	let remaining_passthrough_with_mut = __AliasLayout {
		remaining_mut_start: 1,
		remaining_mut_distinct: false,
		..single_mut
	};
	assert!(remaining_passthrough_with_mut.needs_alias_check());

	let remaining_passthrough_only = __AliasLayout {
		mut_mask: [0; 4],
		remaining_mut_start: 0,
		remaining_mut_distinct: false,
		..single_mut
	};
	assert!(!remaining_passthrough_only.needs_alias_check());

	// A layout with optional readonly slots below index 64 still qualifies for
	// the simple 64-bit mutable path, whereas an optional mutable slot does not.
	let simple_with_opt_ro = __AliasLayout {
		static_layout: true,
		mut_mask: [0b0011, 0, 0, 0],
		optional_mask: [0b0100, 0, 0, 0],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	assert!(simple_with_opt_ro.is_simple_mut_64());
	let opt_mut_layout = __AliasLayout {
		optional_mask: [0b0010, 0, 0, 0],
		..simple_with_opt_ro
	};
	assert!(!opt_mut_layout.is_simple_mut_64());

	// Out-of-bounds bit helpers are safe no-ops.
	assert_eq!(__mask256_with_bit([0; 4], 300), [0; 4]);
	assert_eq!(
		__mask256_or_shifted([0; 4], __mask256_with_bit([0; 4], 250), 10),
		[0; 4]
	);

	// Confirm duplicate error code helper matches `PinaProgramError`.
	assert_eq!(
		dup_err(),
		u64::from(ProgramError::from(
			PinaProgramError::DuplicateMutableAccount
		))
	);

	// Direct `AccountsCursor::new` (check_duplicates = true) enforces
	// distinctness inside `remaining_mut_distinct`.
	let (mut dup_input, _) = serialize(&[rw(1), rw(2), Slot::Duplicate(1)], &[], &ID);
	let mut dup_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
	// SAFETY: `dup_input` holds 3 serialized accounts.
	let (program_id, count, _) =
		unsafe { entrypoint::deserialize::<4>(dup_input.as_mut_ptr().cast(), &mut dup_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let dup_slice = unsafe {
		core::slice::from_raw_parts_mut(dup_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	let mut cursor = AccountsCursor::new(*program_id, dup_slice);
	assert_eq!(
		cursor.remaining_mut_distinct().err(),
		Some(PinaProgramError::DuplicateMutableAccount.into())
	);

	let (mut ok_input, _) = serialize(&[rw(1), rw(2)], &[], &ID);
	let mut ok_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
	// SAFETY: `ok_input` holds 2 serialized accounts.
	let (program_id, count, _) =
		unsafe { entrypoint::deserialize::<4>(ok_input.as_mut_ptr().cast(), &mut ok_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let ok_slice = unsafe {
		core::slice::from_raw_parts_mut(ok_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	let mut cursor = AccountsCursor::new(*program_id, ok_slice);
	assert_eq!(
		cursor.remaining_mut_distinct().map(|slice| slice.len()),
		Ok(2)
	);
}

#[test]
fn multi_word_256_bit_alias_layout_validates_slots_beyond_64() {
	// Exercise word >= 1 of `__AliasLayout` (`mut_mask[1]`, `unique_mask[1]`,
	// `optional_mask[1]`) directly with 70 accounts.
	let mut slots: Vec<Slot> = (0_u8..69).map(rw).collect();
	slots.push(Slot::Duplicate(65));

	let (mut input, _) = serialize(&slots, &[], &ID);
	let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 70];
	// SAFETY: `input` is a valid loader input with 70 accounts.
	let (program_id, count, _) =
		unsafe { entrypoint::deserialize::<70>(input.as_mut_ptr().cast(), &mut views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let accounts =
		unsafe { core::slice::from_raw_parts_mut(views.as_mut_ptr().cast::<AccountView>(), count) };

	let mut mut_mask = [0_u64; 4];
	mut_mask = __mask256_with_bit(mut_mask, 65);
	mut_mask = __mask256_with_bit(mut_mask, 69);

	let layout = __AliasLayout {
		static_layout: true,
		mut_mask,
		optional_mask: [0; 4],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	// Status 2 == ALIAS_ERR_DUPLICATE.
	assert_eq!(
		validate_account_aliasing(program_id, accounts, 70, &layout),
		2
	);

	// Exercise `is_compact_32()` at runtime across true and false boundaries.
	let compact_ok = __AliasLayout {
		static_layout: true,
		mut_mask: [0b101, 0, 0, 0],
		optional_mask: [0b010, 0, 0, 0],
		unique_mask: [0b001, 0, 0, 0],
		remaining_mut_start: 32,
		remaining_mut_distinct: true,
	};
	assert!(compact_ok.is_compact_32());
	assert!(
		!__AliasLayout {
			remaining_mut_start: 33,
			..compact_ok
		}
		.is_compact_32()
	);
	assert!(
		!__AliasLayout {
			mut_mask: [1_u64 << 32, 0, 0, 0],
			..compact_ok
		}
		.is_compact_32()
	);
	assert!(!layout.is_compact_32());

	// Exercise every branch of the >32-slot fallback `validate_account_aliasing`:
	// omitted optional `i`, omitted optional `j`, non-writable `mut_i`, unique
	// conflict, mutable remaining `distinct = false` vs `distinct = true`, and
	// clean pass.
	let mut wide_slots: Vec<Slot> = (0_u8..36).map(rw).collect();
	wide_slots[0] = program_sentinel(false);
	wide_slots[1] = program_sentinel(false); // non-optional slot holding program_id
	wide_slots[2] = Slot::Duplicate(1); // optional slot holding program_id -> omitted!
	wide_slots[34] = Slot::Duplicate(33); // two slots inside trailing mutable remaining

	let (mut wide_input, _) = serialize(&wide_slots, &[], &ID);
	let mut wide_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 36];
	// SAFETY: `wide_input` holds 36 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<36>(wide_input.as_mut_ptr().cast(), &mut wide_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let wide_accounts = unsafe {
		core::slice::from_raw_parts_mut(wide_views.as_mut_ptr().cast::<AccountView>(), count)
	};

	let mut base_wide = __AliasLayout {
		static_layout: true,
		mut_mask: [0; 4],
		optional_mask: __mask256_or_shifted(
			__mask256_with_bit([0; 4], 0),
			__mask256_with_bit([0; 4], 2),
			0,
		),
		unique_mask: __mask256_with_bit([0; 4], 1),
		remaining_mut_start: 33,
		remaining_mut_distinct: false,
	};
	// Slot 0 omitted, slot 2 omitted (so unique slot 1 does NOT conflict with
	// omitted slot 2), and slots 33/34 are in `remaining_mut_distinct = false`.
	assert_eq!(
		validate_account_aliasing(pid, wide_accounts, 36, &base_wide),
		0
	);

	// Enabling `remaining_mut_distinct = true` catches the 33 == 34 duplicate.
	base_wide.remaining_mut_distinct = true;
	assert_eq!(
		validate_account_aliasing(pid, wide_accounts, 36, &base_wide),
		2
	);

	// Marking slot 33 as unique catches 33 == 34 even with `distinct = false`.
	base_wide.remaining_mut_distinct = false;
	base_wide.unique_mask = __mask256_with_bit(base_wide.unique_mask, 33);
	assert_eq!(
		validate_account_aliasing(pid, wide_accounts, 36, &base_wide),
		2
	);

	// Non-writable mutable slot returns status 1 (`ALIAS_ERR_NOT_WRITABLE`).
	let mut ro_slots: Vec<Slot> = (0_u8..34).map(rw).collect();
	ro_slots[33] = ro(99);
	let (mut ro_input, _) = serialize(&ro_slots, &[], &ID);
	let mut ro_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 34];
	// SAFETY: `ro_input` holds 34 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<34>(ro_input.as_mut_ptr().cast(), &mut ro_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let ro_accounts = unsafe {
		core::slice::from_raw_parts_mut(ro_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	let ro_layout = __AliasLayout {
		static_layout: true,
		mut_mask: __mask256_with_bit([0; 4], 33),
		optional_mask: [0; 4],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	assert_eq!(
		validate_account_aliasing(pid, ro_accounts, 34, &ro_layout),
		1
	);
}

#[allow(dead_code)]
#[derive(Accounts)]
struct FiveMutAccounts<'a> {
	a: &'a mut AccountView,
	b: &'a mut AccountView,
	c: &'a mut AccountView,
	d: &'a mut AccountView,
	e: &'a mut AccountView,
}

#[allow(dead_code)]
#[derive(Accounts)]
struct WideFallbackAccounts<'a> {
	f00: &'a AccountView,
	f01: &'a AccountView,
	f02: &'a AccountView,
	f03: &'a AccountView,
	f04: &'a AccountView,
	f05: &'a AccountView,
	f06: &'a AccountView,
	f07: &'a AccountView,
	f08: &'a AccountView,
	f09: &'a AccountView,
	f10: &'a AccountView,
	f11: &'a AccountView,
	f12: &'a AccountView,
	f13: &'a AccountView,
	f14: &'a AccountView,
	f15: &'a AccountView,
	f16: &'a AccountView,
	f17: &'a AccountView,
	f18: &'a AccountView,
	f19: &'a AccountView,
	f20: &'a AccountView,
	f21: &'a AccountView,
	f22: &'a AccountView,
	f23: &'a AccountView,
	f24: &'a AccountView,
	f25: &'a AccountView,
	f26: &'a AccountView,
	f27: &'a AccountView,
	f28: &'a AccountView,
	f29: &'a AccountView,
	f30: &'a AccountView,
	f31: &'a AccountView,
	#[pina(unique)]
	f32: &'a mut AccountView,
	f33: &'a mut AccountView,
}

#[test]
fn five_mut_and_wide_fallback_derived_structs_validate_correctly() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	// 5-mut derived struct routes through `is_simple_mut_64` ->
	// `validate_mut_aliasing_64`.
	let (mut ok5_in, _) = serialize(&[rw(1), rw(2), rw(3), rw(4), rw(5)], &[], &ID);
	let mut ok5_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 8];
	// SAFETY: `ok5_in` holds 5 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<8>(ok5_in.as_mut_ptr().cast(), &mut ok5_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let ok5_slice = unsafe {
		core::slice::from_raw_parts_mut(ok5_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	assert!(FiveMutAccounts::try_from((pid, ok5_slice)).is_ok());

	let (mut dup5_in, _) = serialize(&[rw(1), rw(2), rw(3), rw(4), Slot::Duplicate(1)], &[], &ID);
	let mut dup5_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 8];
	// SAFETY: `dup5_in` holds 5 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<8>(dup5_in.as_mut_ptr().cast(), &mut dup5_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let dup5_slice = unsafe {
		core::slice::from_raw_parts_mut(dup5_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	assert_eq!(
		FiveMutAccounts::try_from((pid, dup5_slice)).err(),
		Some(dup.clone())
	);

	// 34-field derived struct with `#[pina(unique)]` at slot 32 routes through
	// the general `validate_account_aliasing` fallback (`!is_compact_32()`).
	let mut wide_ok: Vec<Slot> = (0_u8..34).map(rw).collect();
	let (mut wok_in, _) = serialize(&wide_ok, &[], &ID);
	let mut wok_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 36];
	// SAFETY: `wok_in` holds 34 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<36>(wok_in.as_mut_ptr().cast(), &mut wok_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let wok_slice = unsafe {
		core::slice::from_raw_parts_mut(wok_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	assert!(WideFallbackAccounts::try_from((pid, wok_slice)).is_ok());

	wide_ok[33] = Slot::Duplicate(32);
	let (mut wdup_in, _) = serialize(&wide_ok, &[], &ID);
	let mut wdup_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 36];
	// SAFETY: `wdup_in` holds 34 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<36>(wdup_in.as_mut_ptr().cast(), &mut wdup_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let wdup_slice = unsafe {
		core::slice::from_raw_parts_mut(wdup_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	assert_eq!(
		WideFallbackAccounts::try_from((pid, wdup_slice)).err(),
		Some(dup.clone())
	);

	// Readonly slot before 32 aliasing unique mutable slot 32 while slot 33 is
	// distinct MUST still be rejected by the wide fallback validator.
	let mut wide_ro_alias: Vec<Slot> = (0_u8..34).map(rw).collect();
	wide_ro_alias[32] = Slot::Duplicate(5);
	let (mut wro_in, _) = serialize(&wide_ro_alias, &[], &ID);
	let mut wro_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 36];
	// SAFETY: `wro_in` holds 34 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<36>(wro_in.as_mut_ptr().cast(), &mut wro_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let wro_slice = unsafe {
		core::slice::from_raw_parts_mut(wro_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	assert_eq!(
		WideFallbackAccounts::try_from((pid, wro_slice)).err(),
		Some(dup)
	);
}

#[test]
fn validate_mut_aliasing_64_allows_trailing_readonly_duplicate_and_mask256_boundaries() {
	// Mutable slot 0 duplicated by a trailing readonly slot 1 MUST succeed in
	// `validate_mut_aliasing_64`: only mutable-vs-mutable pairs conflict.
	let (mut buf, _) = serialize(&[rw(1), Slot::Duplicate(0), rw(2)], &[], &ID);
	let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
	// SAFETY: `buf` holds 3 serialized accounts.
	let (_, count, _) =
		unsafe { entrypoint::deserialize::<4>(buf.as_mut_ptr().cast(), &mut views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let slice =
		unsafe { core::slice::from_raw_parts(views.as_mut_ptr().cast::<AccountView>(), count) };
	// Bits 0 and 2 mutable; bit 1 readonly (even though slot 1 aliases slot 0).
	assert_eq!(validate_mut_aliasing_64(slice, 0b101), 0);
	// When bit 1 is also marked mutable, the duplicate MUST be rejected.
	assert_eq!(validate_mut_aliasing_64(slice, 0b111), 2);

	// Multi-word 256-bit mask arithmetic and compact-32 boundary checks.
	assert_eq!(__mask256_count_ones([1, 2, 4, 8]), 4);
	assert!(!__mask256_any([0, 0, 0, 0]));
	assert!(__mask256_any([0, 0, 0, 1]));
	assert!(__mask256_any([0, 1, 1, 0]));
	assert!(__mask256_any([1, 1, 0, 0]));
	assert!(__mask256_any([0, 0, 1, 1]));

	let mut layout = __AliasLayout {
		static_layout: true,
		mut_mask: [0, 1, 1, 0],
		optional_mask: [0; 4],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	assert!(!layout.is_compact_32());
	layout.mut_mask = [0, 0, 1, 1];
	assert!(!layout.is_compact_32());
}

#[allow(dead_code)]
#[derive(Accounts)]
struct UniqueRoBeforeRequiredMut<'a> {
	#[pina(unique)]
	unique_ro: &'a AccountView,
	target_mut: &'a mut AccountView,
}

#[test]
fn writability_precedes_duplicate_for_readonly_unique_before_mutable_slot() {
	// Earlier readonly `#[pina(unique)]` slot + later required mutable slot
	// sharing the same readonly account MUST report `InvalidAccountData`
	// (writability precedence) in both compact-32 and wide (>32) validators.
	let (mut compact_in, _) = serialize(&[ro(1), Slot::Duplicate(0)], &[], &ID);
	let mut compact_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
	// SAFETY: `compact_in` holds 2 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<4>(compact_in.as_mut_ptr().cast(), &mut compact_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let compact_slice = unsafe {
		core::slice::from_raw_parts_mut(compact_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	assert_eq!(
		UniqueRoBeforeRequiredMut::try_from((pid, compact_slice)).err(),
		Some(ProgramError::InvalidAccountData)
	);

	// Wide (>32) layout: slot 0 is readonly `unique`, slot 33 is required mut,
	// and both receive the same readonly account.
	let mut wide_slots: Vec<Slot> = (0_u8..34).map(ro).collect();
	wide_slots[33] = Slot::Duplicate(0);
	let (mut wide_in, _) = serialize(&wide_slots, &[], &ID);
	let mut wide_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 36];
	// SAFETY: `wide_in` holds 34 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<36>(wide_in.as_mut_ptr().cast(), &mut wide_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let wide_slice = unsafe {
		core::slice::from_raw_parts(wide_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	let wide_layout = __AliasLayout {
		static_layout: true,
		mut_mask: __mask256_with_bit([0; 4], 33),
		optional_mask: [0; 4],
		unique_mask: __mask256_with_bit([0; 4], 0),
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	assert_eq!(
		validate_account_aliasing(pid, wide_slice, 34, &wide_layout),
		1
	);
}

#[test]
fn manual_parser_delegating_to_derived_unique_child_enforces_uniqueness() {
	#[allow(dead_code)]
	struct ManualWrapperAroundUnique<'a> {
		child: UniqueRoFirstAccounts<'a>,
	}

	impl<'a> ParseAccounts<'a> for ManualWrapperAroundUnique<'a> {
		fn parse_accounts(cursor: &mut AccountsCursor<'a>) -> Result<Self, ProgramError> {
			let child = UniqueRoFirstAccounts::parse_accounts(cursor)?;
			cursor.finish_exact()?;
			Ok(Self { child })
		}
	}

	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	// Aliased readonly slots inside the derived unique child MUST be rejected
	// even when parsed through an unverified manual `AccountsCursor::new`.
	let (mut bad_in, _) = serialize(&[ro(1), Slot::Duplicate(0)], &[], &ID);
	let mut bad_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
	// SAFETY: `bad_in` holds 2 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<4>(bad_in.as_mut_ptr().cast(), &mut bad_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let bad_slice = unsafe {
		core::slice::from_raw_parts_mut(bad_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	let mut bad_cursor = AccountsCursor::new(*pid, bad_slice);
	assert_eq!(
		ManualWrapperAroundUnique::parse_accounts(&mut bad_cursor).err(),
		Some(dup.clone())
	);

	// Distinct accounts succeed cleanly.
	let (mut ok_in, _) = serialize(&[ro(1), ro(2)], &[], &ID);
	let mut ok_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
	// SAFETY: `ok_in` holds 2 serialized accounts.
	let (pid, count, _) =
		unsafe { entrypoint::deserialize::<4>(ok_in.as_mut_ptr().cast(), &mut ok_views) };
	// SAFETY: `deserialize` initialized `count` entries.
	let ok_slice = unsafe {
		core::slice::from_raw_parts_mut(ok_views.as_mut_ptr().cast::<AccountView>(), count)
	};
	let mut ok_cursor = AccountsCursor::new(*pid, ok_slice);
	assert!(ManualWrapperAroundUnique::parse_accounts(&mut ok_cursor).is_ok());

	// A derived outer struct embedding the handwritten wrapper has a dynamic
	// layout (`static_layout == false`). `dispatch_entrypoint!` /
	// `try_from_route` MUST still hand it a checked cursor when a readonly
	// duplicate is present so the child's `#[pina(unique)]` guard rejects it.
	#[allow(dead_code)]
	#[derive(Accounts)]
	struct DerivedOuterAroundManualUnique<'a> {
		manual: ManualWrapperAroundUnique<'a>,
	}

	assert!(!DerivedOuterAroundManualUnique::ALIAS_LAYOUT.static_layout);

	let (mut bad_route_in, bad_offset) = serialize(&[ro(1), Slot::Duplicate(0)], &[], &ID);
	let bad_ptr = bad_route_in.as_mut_ptr().cast::<u8>();
	// SAFETY: `bad_route_in` is a valid serialized entrypoint buffer.
	let bad_status = unsafe {
		__dispatch_entrypoint(
			bad_ptr,
			bad_ptr.add(bad_offset),
			|entry: EntrypointInput, pid: &Address, _data: &[u8]| {
				let mut slots = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
				let route = entry.parse_route::<4, { ROUTE_UNBOUNDED }, true>(&mut slots)?;
				let _ = DerivedOuterAroundManualUnique::try_from_route(pid, route)?;
				Ok(())
			},
		)
	};
	assert_eq!(bad_status, u64::from(dup));

	// Distinct readonly accounts take the duplicate-free fast path and succeed.
	let (mut ok_route_in, ok_offset) = serialize(&[ro(1), ro(2)], &[], &ID);
	let ok_ptr = ok_route_in.as_mut_ptr().cast::<u8>();
	// SAFETY: `ok_route_in` is a valid serialized entrypoint buffer.
	let ok_status = unsafe {
		__dispatch_entrypoint(
			ok_ptr,
			ok_ptr.add(ok_offset),
			|entry: EntrypointInput, pid: &Address, _data: &[u8]| {
				let mut slots = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
				let route = entry.parse_route::<4, { ROUTE_UNBOUNDED }, true>(&mut slots)?;
				let parsed = DerivedOuterAroundManualUnique::try_from_route(pid, route)?;
				assert_eq!(parsed.manual.child.first.address().as_ref()[0], 1);
				assert_eq!(parsed.manual.child.second.address().as_ref()[0], 2);
				Ok(())
			},
		)
	};
	assert_eq!(ok_status, SUCCESS);
}

#[test]
fn handwritten_try_from_conversion_runs_on_both_dispatch_first_and_slice_routers() {
	// `ManualTryFromAccounts::try_from` rejects `0xFE` with `Custom(0x7777)`,
	// whereas `try_from_account_infos` accepts it. Both routers MUST enforce
	// the handwritten `TryFrom` check.
	assert_both_reject(
		AliasInstruction::ManualTryFrom,
		&[rw(0xFE), rw(2)],
		&ProgramError::Custom(0x7777),
	);

	// Also exercise the default `TryFromAccountInfos::try_from_route` fallback
	// directly so manual callers invoking it inherit `try_from_account_infos`.
	let (mut input, offset) = serialize(&[rw(0xFE), rw(2)], &[], &ID);
	let ptr = input.as_mut_ptr().cast::<u8>();
	// SAFETY: `input` is a valid serialized Solana entrypoint buffer and
	// `offset` points at the instruction-data slice.
	let status = unsafe {
		__dispatch_entrypoint(
			ptr,
			ptr.add(offset),
			|entry: EntrypointInput, pid: &Address, _data: &[u8]| {
				let mut slots = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 4];
				let route = entry.parse_route::<4, { ROUTE_UNBOUNDED }, true>(&mut slots)?;
				let parsed = ManualTryFromAccounts::try_from_route(pid, route)?;
				assert_eq!(parsed.first.address().as_ref()[0], 0xFE);
				assert_eq!(parsed.second.address().as_ref()[0], 2);
				Ok(())
			},
		)
	};
	assert_eq!(status, SUCCESS);
}

#[allow(dead_code)]
#[derive(Accounts)]
struct WideOptMutAccounts<'a> {
	f00: &'a AccountView,
	f01: &'a AccountView,
	f02: &'a AccountView,
	f03: &'a AccountView,
	f04: &'a AccountView,
	f05: &'a AccountView,
	f06: &'a AccountView,
	f07: &'a AccountView,
	f08: &'a AccountView,
	f09: &'a AccountView,
	f10: &'a AccountView,
	f11: &'a AccountView,
	f12: &'a AccountView,
	f13: &'a AccountView,
	f14: &'a AccountView,
	f15: &'a AccountView,
	f16: &'a AccountView,
	f17: &'a AccountView,
	f18: &'a AccountView,
	f19: &'a AccountView,
	f20: &'a AccountView,
	f21: &'a AccountView,
	f22: &'a AccountView,
	f23: &'a AccountView,
	f24: &'a AccountView,
	f25: &'a AccountView,
	f26: &'a AccountView,
	f27: &'a AccountView,
	f28: &'a AccountView,
	f29: &'a AccountView,
	f30: &'a AccountView,
	f31: &'a AccountView,
	req_mut: &'a mut AccountView,
	opt_a: Option<&'a mut AccountView>,
	opt_b: Option<&'a mut AccountView>,
}

#[test]
fn opt_mut_aliasing_64_enforces_all_adversarial_cases_and_precedence() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	// Confirm every optional-mutable layout with no `#[pina(unique)]` or mutable
	// remaining routes through `is_mut_only_64()` -> `validate_opt_mut_aliasing_64`.
	const {
		assert!(OptionalMutPairAccounts::ALIAS_LAYOUT.is_mut_only_64());
		assert!(!OptionalMutPairAccounts::ALIAS_LAYOUT.is_simple_mut_64());
		assert!(OptionalMutPairAccounts::ALIAS_LAYOUT.needs_alias_check());

		assert!(ReqAndOptionalMutAccounts::ALIAS_LAYOUT.is_mut_only_64());
		assert!(!ReqAndOptionalMutAccounts::ALIAS_LAYOUT.is_simple_mut_64());
		assert!(ReqAndOptionalMutAccounts::ALIAS_LAYOUT.needs_alias_check());

		assert!(TwoReqAndOptMutAccounts::ALIAS_LAYOUT.is_mut_only_64());
		assert!(!TwoReqAndOptMutAccounts::ALIAS_LAYOUT.is_simple_mut_64());
		assert!(TwoReqAndOptMutAccounts::ALIAS_LAYOUT.needs_alias_check());
	}

	// 1. Distinct required + two present optional mutable accounts succeed.
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), rw(3), rw(4)],
		std::vec![1, 2, 3, 4],
	);

	// 2. Two omitted optionals (both readonly and writable sentinel so
	// `DUP_WRITABLE_MASK != 0` enters `validate_opt_mut_aliasing_64` on dispatch).
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), program_sentinel(false), Slot::Duplicate(2)],
		std::vec![1, 2],
	);
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), program_sentinel(true), Slot::Duplicate(2)],
		std::vec![1, 2],
	);

	// 3. Omitted optional (`program_id` sentinel) aliasing a required `&mut`
	// slot holding `program_id`: optional parses as `None` and must NOT conflict.
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), program_sentinel(true), Slot::Duplicate(1), rw(3)],
		std::vec![1, ID.as_ref()[0], 3],
	);
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[
			ro(1),
			program_sentinel(true),
			Slot::Duplicate(1),
			Slot::Duplicate(1),
		],
		std::vec![1, ID.as_ref()[0]],
	);

	// 4. Required `&mut` slot holding `program_id` is NOT omitted: if readonly,
	// it must be rejected with `InvalidAccountData`, and two required `&mut`
	// slots holding `program_id` must be rejected with `DuplicateMutableAccount`
	// even while an optional slot is omitted.
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[
			ro(1),
			program_sentinel(false),
			Slot::Duplicate(1),
			Slot::Duplicate(1),
		],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::TwoReqAndOptMut,
		&[
			program_sentinel(true),
			Slot::Duplicate(0),
			Slot::Duplicate(0),
		],
		&dup,
	);

	// 5. Present optional aliasing a required mutable slot MUST be rejected.
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), Slot::Duplicate(1), rw(3)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), rw(3), Slot::Duplicate(1)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), program_sentinel(false), Slot::Duplicate(1)],
		&dup,
	);

	// 6. Present optional aliasing another present optional MUST be rejected.
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), rw(3), Slot::Duplicate(2)],
		&dup,
	);

	// 7. Present optional that is not writable MUST be rejected with
	// `InvalidAccountData` (including when duplicated or combined with an
	// earlier writable duplicate — writability precedes duplicate).
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), ro(3), rw(4)],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), ro(3), Slot::Duplicate(2)],
		&ProgramError::InvalidAccountData,
	);
	// Precedence case A (`i < j < k`): earlier writable duplicate + later
	// non-writable present optional must still report `InvalidAccountData`.
	assert_both_reject(
		AliasInstruction::TwoReqAndOptMut,
		&[rw(1), Slot::Duplicate(0), ro(2)],
		&ProgramError::InvalidAccountData,
	);
	// Precedence case B (`i < k < j`): interleaved readonly present optional
	// between two aliasing writable mutable slots must report `InvalidAccountData`.
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(9), rw(1), ro(2), Slot::Duplicate(1)],
		&ProgramError::InvalidAccountData,
	);
	// Omitted readonly optional after an earlier writable duplicate does NOT
	// trigger writability failure; the earlier duplicate reports
	// `DuplicateMutableAccount`.
	assert_both_reject(
		AliasInstruction::TwoReqAndOptMut,
		&[rw(1), Slot::Duplicate(0), program_sentinel(false)],
		&dup,
	);

	// 8. Shortened account slices (omitting one or both trailing optionals,
	// including with `DUP_WRITABLE_MASK != 0` so `validate_opt_mut_aliasing_64`
	// runs with `accounts.len() < ACCOUNT_LIMIT`).
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2)],
		std::vec![1, 2],
	);
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), rw(3)],
		std::vec![1, 2, 3],
	);
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), program_sentinel(true)],
		std::vec![1, 2],
	);
	assert_both_accept(
		AliasInstruction::ReqAndOptionalMut,
		&[rw(1), Slot::Duplicate(0)],
		std::vec![1, 1],
	);
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), rw(2), Slot::Duplicate(1)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1), ro(2)],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::ReqAndOptionalMut,
		&[ro(1)],
		&ProgramError::NotEnoughAccountKeys,
	);
}

#[test]
fn opt_mut_aliasing_64_handles_indices_32_to_63_and_slices_over_64() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	const {
		assert!(WideOptMutAccounts::ALIAS_LAYOUT.is_mut_only_64());
		assert!(!WideOptMutAccounts::ALIAS_LAYOUT.is_simple_mut_64());
		assert!(!WideOptMutAccounts::ALIAS_LAYOUT.is_compact_32());
		assert!(WideOptMutAccounts::ALIAS_LAYOUT.needs_alias_check());
	}

	// Verify `is_mut_only_64()` false boundaries explicitly.
	let mut boundary = __AliasLayout {
		static_layout: true,
		mut_mask: [0b11, 0, 0, 0],
		optional_mask: [0b10, 0, 0, 0],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	assert!(boundary.is_mut_only_64());
	assert!(
		!__AliasLayout {
			mut_mask: [0b11, 1, 0, 0],
			..boundary
		}
		.is_mut_only_64()
	);
	assert!(
		!__AliasLayout {
			unique_mask: [1, 0, 0, 0],
			..boundary
		}
		.is_mut_only_64()
	);
	boundary.remaining_mut_start = 4;
	assert!(!boundary.is_mut_only_64());

	let run_wide_both = |slots: &[Slot]| -> (u64, Result<(), ProgramError>) {
		let (mut direct_in, _) = serialize(slots, &[], &ID);
		let mut direct_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 40];
		// SAFETY: `direct_in` holds at most 35 serialized accounts.
		let (pid, count, _) = unsafe {
			entrypoint::deserialize::<40>(direct_in.as_mut_ptr().cast(), &mut direct_views)
		};
		// SAFETY: `deserialize` initialized `count` entries.
		let direct_slice = unsafe {
			core::slice::from_raw_parts_mut(direct_views.as_mut_ptr().cast::<AccountView>(), count)
		};
		let direct_res = WideOptMutAccounts::try_from((pid, direct_slice)).map(|_| ());

		let (mut route_in, offset) = serialize(slots, &[], &ID);
		let ptr = route_in.as_mut_ptr().cast::<u8>();
		// SAFETY: `route_in` is a valid serialized entrypoint buffer.
		let route_status = unsafe {
			__dispatch_entrypoint(
				ptr,
				ptr.add(offset),
				|entry: EntrypointInput, pid: &Address, _data: &[u8]| {
					let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 35];
					let route = entry.parse_route::<35, { ROUTE_BOUNDED }, true>(&mut views)?;
					let _ = WideOptMutAccounts::try_from_route(pid, route)?;
					Ok(())
				},
			)
		};

		(route_status, direct_res)
	};

	let base_slots = || -> Vec<Slot> {
		let mut slots: Vec<Slot> = (0_u8..32).map(ro).collect();
		slots.push(rw(40)); // slot 32: req_mut
		slots.push(rw(41)); // slot 33: opt_a
		slots.push(rw(42)); // slot 34: opt_b
		slots
	};

	// Distinct accounts at slots 32..35 succeed.
	assert_eq!(run_wide_both(&base_slots()), (SUCCESS, Ok(())));

	// Both optionals omitted via writable `program_id` sentinel succeed.
	let mut two_omitted = base_slots();
	two_omitted[33] = program_sentinel(true);
	two_omitted[34] = Slot::Duplicate(33);
	assert_eq!(run_wide_both(&two_omitted), (SUCCESS, Ok(())));

	// Omitted optional at slot 33 aliasing required `&mut` holding `program_id`
	// at slot 32 succeeds.
	let mut req_prog_omitted = base_slots();
	req_prog_omitted[32] = program_sentinel(true);
	req_prog_omitted[33] = Slot::Duplicate(32);
	assert_eq!(run_wide_both(&req_prog_omitted), (SUCCESS, Ok(())));

	// Present optional at slot 33 aliasing required `&mut` at slot 32 fails.
	let mut opt_aliases_req = base_slots();
	opt_aliases_req[33] = Slot::Duplicate(32);
	assert_eq!(
		run_wide_both(&opt_aliases_req),
		(u64::from(dup.clone()), Err(dup.clone()))
	);

	// Present optional at slot 34 aliasing present optional at slot 33 fails.
	let mut opt_aliases_opt = base_slots();
	opt_aliases_opt[34] = Slot::Duplicate(33);
	assert_eq!(
		run_wide_both(&opt_aliases_opt),
		(u64::from(dup.clone()), Err(dup.clone()))
	);

	// Present optional at slot 33 readonly while slot 32 == slot 34 reports
	// `InvalidAccountData` (writability precedes duplicate even in 32..64).
	let mut precedence_wide = base_slots();
	precedence_wide[33] = ro(99);
	precedence_wide[34] = Slot::Duplicate(32);
	assert_eq!(
		run_wide_both(&precedence_wide),
		(
			u64::from(ProgramError::InvalidAccountData),
			Err(ProgramError::InvalidAccountData)
		)
	);

	// Shortened slice (34 accounts, trailing slot 34 absent): distinct succeeds,
	// duplicate between 32 and 33 fails.
	let mut short_ok = base_slots();
	short_ok.pop();
	assert_eq!(run_wide_both(&short_ok), (SUCCESS, Ok(())));

	let mut short_dup = short_ok;
	short_dup[33] = Slot::Duplicate(32);
	assert_eq!(
		run_wide_both(&short_dup),
		(u64::from(dup.clone()), Err(dup))
	);

	// Bit-63 boundary with `accounts.len() == 66 > 64`: verify bit 63 (`1 << 63`)
	// handles omitted optional, duplicate, non-writable, and ignores >64 slots,
	// matching `validate_account_aliasing` on every case.
	let bit_32_63_layout = __AliasLayout {
		static_layout: true,
		mut_mask: [(1_u64 << 32) | (1_u64 << 63), 0, 0, 0],
		optional_mask: [1_u64 << 63, 0, 0, 0],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};
	let check_bit_63 = |slots: &[Slot], expected: u8| {
		let (mut input, _) = serialize(slots, &[], &ID);
		let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 66];
		// SAFETY: `input` holds 66 serialized accounts.
		let (pid, count, _) =
			unsafe { entrypoint::deserialize::<66>(input.as_mut_ptr().cast(), &mut views) };
		// SAFETY: `deserialize` initialized `count` entries.
		let slice =
			unsafe { core::slice::from_raw_parts(views.as_mut_ptr().cast::<AccountView>(), count) };
		let fast = validate_opt_mut_aliasing_64(
			pid,
			slice,
			bit_32_63_layout.mut_mask[0],
			bit_32_63_layout.optional_mask[0] & bit_32_63_layout.mut_mask[0],
		);
		let general = validate_account_aliasing(pid, slice, 66, &bit_32_63_layout);
		assert_eq!(fast, expected);
		assert_eq!(fast, general);
	};

	let mut over64_ok: Vec<Slot> = (0_u8..66).map(rw).collect();
	// Duplicate at slots 64/65 (outside mut_mask[0]) must be ignored.
	over64_ok[65] = Slot::Duplicate(64);
	check_bit_63(&over64_ok, 0);

	// Slot 63 omitted (`program_id` aliasing slot 32) -> OK (0).
	let mut over64_omitted = over64_ok.clone();
	over64_omitted[32] = program_sentinel(true);
	over64_omitted[63] = Slot::Duplicate(32);
	check_bit_63(&over64_omitted, 0);

	// Slot 63 present aliasing slot 32 -> duplicate (2).
	let mut over64_dup = over64_ok.clone();
	over64_dup[63] = Slot::Duplicate(32);
	check_bit_63(&over64_dup, 2);

	// Slot 63 present readonly -> not writable (1).
	let mut over64_ro = over64_ok;
	over64_ro[63] = ro(120);
	check_bit_63(&over64_ro, 1);
}

#[allow(dead_code)]
#[derive(Accounts)]
struct LowOptMutWithImmutableRemaining<'a> {
	opt_a: Option<&'a mut AccountView>,
	opt_b: Option<&'a mut AccountView>,
	#[pina(remaining)]
	rest: &'a [AccountView],
}

#[test]
fn opt_mut_aliasing_64_pins_64_slot_clamp_against_wraparound() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	// Confirm the derived shape (`Option<&mut>` in low slots 0/1 + immutable
	// `#[pina(remaining)]`) routes to `validate_opt_mut_aliasing_64` and accepts
	// unbounded slices (`>= 65` accounts).
	const {
		assert!(LowOptMutWithImmutableRemaining::ALIAS_LAYOUT.is_mut_only_64());
		assert!(!LowOptMutWithImmutableRemaining::ALIAS_LAYOUT.is_simple_mut_64());
		assert!(LowOptMutWithImmutableRemaining::ALIAS_LAYOUT.needs_alias_check());
		assert!(LowOptMutWithImmutableRemaining::ACCOUNT_LIMIT == usize::MAX);
		assert!(LowOptMutWithImmutableRemaining::ALIAS_LAYOUT.mut_mask[0] == 0b11);
		assert!(LowOptMutWithImmutableRemaining::ALIAS_LAYOUT.optional_mask[0] == 0b11);
	}

	// Layout with optional-mutable slots at indices 0, 1, 32, and 63 plus a
	// required mutable slot at index 2. Without `.min(64)`, `1_u64.wrapping_shl(i)`
	// wraps index 64 -> bit 0 and index 65 -> bit 1, letting `program_id` in
	// slots 64/65 clear slot 0/1's bit or a readonly account in slot 64 falsely
	// reject a valid transaction.
	let wrap_layout = __AliasLayout {
		static_layout: true,
		mut_mask: [
			(1_u64 << 0) | (1_u64 << 1) | (1_u64 << 2) | (1_u64 << 32) | (1_u64 << 63),
			0,
			0,
			0,
		],
		optional_mask: [
			(1_u64 << 0) | (1_u64 << 1) | (1_u64 << 32) | (1_u64 << 63),
			0,
			0,
			0,
		],
		unique_mask: [0; 4],
		remaining_mut_start: usize::MAX,
		remaining_mut_distinct: true,
	};

	let check_wrap = |slots: &[Slot], expected: u8| {
		let (mut input, _) = serialize(slots, &[], &ID);
		let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 72];
		// SAFETY: `input` holds at most 69 serialized accounts (`<= 72`).
		let (pid, count, _) =
			unsafe { entrypoint::deserialize::<72>(input.as_mut_ptr().cast(), &mut views) };
		assert_eq!(count, slots.len());
		// SAFETY: `deserialize` initialized `count` entries.
		let slice =
			unsafe { core::slice::from_raw_parts(views.as_mut_ptr().cast::<AccountView>(), count) };
		let fast = validate_opt_mut_aliasing_64(
			pid,
			slice,
			wrap_layout.mut_mask[0],
			wrap_layout.optional_mask[0] & wrap_layout.mut_mask[0],
		);
		let general = validate_account_aliasing(pid, slice, usize::MAX, &wrap_layout);
		assert_eq!(fast, expected, "unexpected status for len={}", slots.len());
		assert_eq!(
			fast,
			general,
			"mismatch vs validate_account_aliasing for len={}",
			slots.len()
		);
	};

	let run_derived_both = |slots: &[Slot]| -> (u64, Result<(), ProgramError>) {
		let (mut direct_in, _) = serialize(slots, &[], &ID);
		let mut direct_views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 72];
		// SAFETY: `direct_in` holds at most 69 serialized accounts (`<= 72`).
		let (pid, count, _) = unsafe {
			entrypoint::deserialize::<72>(direct_in.as_mut_ptr().cast(), &mut direct_views)
		};
		assert_eq!(count, slots.len());
		// SAFETY: `deserialize` initialized `count` entries.
		let direct_slice = unsafe {
			core::slice::from_raw_parts_mut(direct_views.as_mut_ptr().cast::<AccountView>(), count)
		};
		let direct_res = LowOptMutWithImmutableRemaining::try_from((pid, direct_slice)).map(|_| ());

		let (mut route_in, offset) = serialize(slots, &[], &ID);
		let ptr = route_in.as_mut_ptr().cast::<u8>();
		// SAFETY: `route_in` is a valid serialized entrypoint buffer.
		let route_status = unsafe {
			__dispatch_entrypoint(
				ptr,
				ptr.add(offset),
				|entry: EntrypointInput, pid: &Address, _data: &[u8]| {
					let mut views = [const { core::mem::MaybeUninit::<AccountView>::uninit() }; 72];
					let route = entry.parse_route::<72, { ROUTE_UNBOUNDED }, true>(&mut views)?;
					let _ = LowOptMutWithImmutableRemaining::try_from_route(pid, route)?;
					Ok(())
				},
			)
		};

		(route_status, direct_res)
	};

	for len in [65_usize, 66, 69] {
		let base = || -> Vec<Slot> { (0..len).map(|idx| rw((idx + 1) as u8)).collect() };

		// 1. Present optional at slot 0 aliases required mutable slot 2 while
		// slot 64 holds `program_id` (both readonly and writable). Must reject
		// as duplicate (`2`).
		for sentinel_writable in [false, true] {
			let mut dup_slot0 = base();
			dup_slot0[2] = Slot::Duplicate(0);
			dup_slot0[64] = program_sentinel(sentinel_writable);
			if len > 65 {
				dup_slot0[65] = Slot::Duplicate(64);
			}
			check_wrap(&dup_slot0, 2);
		}

		// 2. Present optional at slot 1 aliases required mutable slot 2 while
		// slot 65 holds `program_id` (for slices of length `>= 66`). Must reject
		// as duplicate (`2`).
		if len > 65 {
			for sentinel_writable in [false, true] {
				let mut dup_slot1 = base();
				dup_slot1[2] = Slot::Duplicate(1);
				dup_slot1[64] = program_sentinel(false);
				dup_slot1[65] = program_sentinel(sentinel_writable);
				check_wrap(&dup_slot1, 2);
			}
		}

		// 3. Present optional at slot 0 aliases present optional at slot 1 while
		// slots 64 (and 65) hold `program_id`. Must reject as duplicate (`2`).
		let mut dup_0_1 = base();
		dup_0_1[1] = Slot::Duplicate(0);
		dup_0_1[64] = program_sentinel(false);
		if len > 65 {
			dup_0_1[65] = program_sentinel(true);
		}
		check_wrap(&dup_0_1, 2);

		// 4. Non-writable present optional at slot 0 or slot 1 while slots 64/65
		// hold `program_id`. Must reject as not-writable (`1`).
		let mut ro_slot0 = base();
		ro_slot0[0] = ro(100);
		ro_slot0[64] = program_sentinel(true);
		if len > 65 {
			ro_slot0[65] = program_sentinel(false);
		}
		check_wrap(&ro_slot0, 1);

		let mut ro_slot1 = base();
		ro_slot1[1] = ro(101);
		ro_slot1[64] = program_sentinel(false);
		if len > 65 {
			ro_slot1[65] = program_sentinel(true);
		}
		check_wrap(&ro_slot1, 1);

		// Writability-before-duplicate precedence with slot 64 = `program_id`.
		let mut prec_wrap = base();
		prec_wrap[1] = Slot::Duplicate(0);
		prec_wrap[2] = ro(102);
		prec_wrap[64] = program_sentinel(true);
		check_wrap(&prec_wrap, 1);

		// 5. Distinct valid present optionals at 0, 1, 32, 63 with readonly
		// non-`program_id` accounts at slots `64..len` must succeed (`0`).
		let mut ro_tail = base();
		for slot in &mut ro_tail[64..] {
			*slot = ro(150);
		}
		check_wrap(&ro_tail, 0);

		// 6. End-to-end derived `LowOptMutWithImmutableRemaining` (direct +
		// dispatch) across lengths 65, 66, and 69.
		assert_eq!(run_derived_both(&ro_tail), (SUCCESS, Ok(())));
		assert_eq!(
			run_derived_both(&dup_0_1),
			(u64::from(dup.clone()), Err(dup.clone()))
		);
		assert_eq!(
			run_derived_both(&ro_slot0),
			(
				u64::from(ProgramError::InvalidAccountData),
				Err(ProgramError::InvalidAccountData)
			)
		);
	}
}

#[test]
fn validate_opt_mut_aliasing_64_matches_general_across_lengths_0_to_69() {
	use pinocchio::account::NOT_BORROWED;
	use pinocchio::account::RuntimeAccount;

	fn make_runtime(address: Address, writable: bool) -> RuntimeAccount {
		RuntimeAccount {
			borrow_state: NOT_BORROWED,
			is_signer: 0,
			is_writable: u8::from(writable),
			executable: 0,
			padding: [0; 4],
			address,
			owner: ID,
			lamports: 1,
			data_len: 0,
		}
	}

	let mut pool_raw = [
		make_runtime(addr(1), true),  // 0: rw(1)
		make_runtime(addr(2), true),  // 1: rw(2)
		make_runtime(addr(3), false), // 2: ro(3)
		make_runtime(addr(4), false), // 3: ro(4)
		make_runtime(ID, true),       // 4: rw(ID)
		make_runtime(ID, false),      // 5: ro(ID)
	];
	// SAFETY: `pool_raw` remains pinned for the duration of this test.
	let pool: [AccountView; 6] = core::array::from_fn(|idx| unsafe {
		AccountView::new_unchecked(core::ptr::from_mut(&mut pool_raw[idx]))
	});

	let layouts: &[(u64, u64)] = &[
		// Optional-mutable at bits 0 and 1.
		(0b11, 0b11),
		// Optional-mutable at bits 0, 1 + required mutable at bit 2.
		(0b111, 0b011),
		// Required mutable at bit 0 + optional-mutable at bits 1, 2.
		(0b111, 0b110),
		// Mixed readonly (bit 1), required mut (bit 2), optional mut (bits 0, 3).
		(0b1101, 0b1001),
		// Wide layout spanning low bits 0/1, mid bit 32, and bit 63.
		(
			(1_u64 << 0) | (1_u64 << 1) | (1_u64 << 32) | (1_u64 << 63),
			(1_u64 << 0) | (1_u64 << 1) | (1_u64 << 63),
		),
	];

	// Adversarial head prefixes exercising duplicates, non-writable slots,
	// writability-before-duplicate precedence, and `program_id` sentinels.
	let heads: &[[usize; 4]] = &[
		[0, 1, 0, 1], // distinct or aliasing on slot 0/2
		[0, 0, 1, 1], // slot 0 == slot 1 (writable duplicate)
		[0, 0, 2, 1], // earlier writable duplicate + later readonly slot
		[0, 2, 0, 1], // interleaved readonly slot between aliasing mutables
		[2, 1, 0, 1], // readonly slot 0
		[0, 2, 1, 1], // readonly slot 1
		[4, 4, 0, 1], // writable `program_id` in slots 0 and 1
		[5, 0, 1, 1], // readonly `program_id` in slot 0
		[0, 5, 0, 1], // readonly `program_id` in slot 1 + slot 0 == slot 2
		[0, 1, 4, 5], // distinct low mutables + sentinel in slots 2/3
	];

	// Tail fillers applied from index 4..len (covering indices 32, 63, and
	// wrap-around indices 64..=68): readonly sentinel, writable sentinel,
	// readonly non-sentinel, and writable account aliasing `pool[0]`.
	let tail_fillers: &[usize] = &[5, 4, 3, 0];

	let mut buf = [pool[1]; 69];
	for &(mut_mask, opt_mask) in layouts {
		let opt_mut_mask = opt_mask & mut_mask;
		let layout = __AliasLayout {
			static_layout: true,
			mut_mask: [mut_mask, 0, 0, 0],
			optional_mask: [opt_mask, 0, 0, 0],
			unique_mask: [0; 4],
			remaining_mut_start: usize::MAX,
			remaining_mut_distinct: true,
		};
		let fits_compact_32 = ((mut_mask | opt_mask) >> 32) == 0;
		let mut_and_opt =
			(mut_mask & u64::from(u32::MAX)) | ((opt_mask & u64::from(u32::MAX)) << 32);
		let unique_and_rem = 0xFF_u64 << 32;

		for head in heads {
			for &tail in tail_fillers {
				for slot in &mut buf[4..] {
					*slot = pool[tail];
				}
				for (idx, &p) in head.iter().enumerate() {
					buf[idx] = pool[p];
				}
				// Ensure bit 32 is distinct/writable when testing the wide layout
				// so slot 63 and wrap-around slots 64..68 stay reachable.
				if (mut_mask & (1_u64 << 32)) != 0 {
					buf[32] = pool[1];
				}

				for len in 0_usize..=69 {
					let slice = &buf[..len];
					let fast = validate_opt_mut_aliasing_64(&ID, slice, mut_mask, opt_mut_mask);
					let general = validate_account_aliasing(&ID, slice, usize::MAX, &layout);
					assert_eq!(
						fast, general,
						"mismatch vs general for mut={mut_mask:#x} opt={opt_mask:#x} head={head:?} tail={tail} len={len}"
					);
					if fits_compact_32 {
						let compact = validate_account_aliasing_compact(
							&ID,
							slice,
							mut_and_opt,
							unique_and_rem,
						);
						assert_eq!(
							fast, compact,
							"mismatch vs compact_32 for mut={mut_mask:#x} opt={opt_mask:#x} head={head:?} tail={tail} len={len}"
						);
					}
				}
			}
		}
	}
}

#[test]
fn validate_opt_mut_aliasing_64_matches_general_validators_exhaustively() {
	use pinocchio::account::NOT_BORROWED;
	use pinocchio::account::RuntimeAccount;

	fn make_runtime(address: Address, writable: bool) -> RuntimeAccount {
		RuntimeAccount {
			borrow_state: NOT_BORROWED,
			is_signer: 0,
			is_writable: u8::from(writable),
			executable: 0,
			padding: [0; 4],
			address,
			owner: ID,
			lamports: 1,
			data_len: 0,
		}
	}

	let mut pool_raw = [
		make_runtime(addr(1), true),  // 0: rw(1)
		make_runtime(addr(2), true),  // 1: rw(2)
		make_runtime(addr(3), false), // 2: ro(3)
		make_runtime(addr(4), false), // 3: ro(4)
		make_runtime(ID, true),       // 4: rw(ID)
		make_runtime(ID, false),      // 5: ro(ID)
	];
	// SAFETY: `pool_raw` remains pinned for the duration of this test.
	let pool: [AccountView; 6] = core::array::from_fn(|idx| unsafe {
		AccountView::new_unchecked(core::ptr::from_mut(&mut pool_raw[idx]))
	});

	let assert_all_match = |slots: usize, mut_mask: u64, opt_mask: u64, slice: &[AccountView]| {
		let opt_mut_mask = opt_mask & mut_mask;
		let layout = __AliasLayout {
			static_layout: true,
			mut_mask: [mut_mask, 0, 0, 0],
			optional_mask: [opt_mask, 0, 0, 0],
			unique_mask: [0; 4],
			remaining_mut_start: usize::MAX,
			remaining_mut_distinct: true,
		};
		let mut_and_opt =
			(mut_mask & u64::from(u32::MAX)) | ((opt_mask & u64::from(u32::MAX)) << 32);
		let unique_and_rem = 0xFF_u64 << 32;

		let opt_mut_64 = validate_opt_mut_aliasing_64(&ID, slice, mut_mask, opt_mut_mask);
		let compact_32 = validate_account_aliasing_compact(&ID, slice, mut_and_opt, unique_and_rem);
		let general = validate_account_aliasing(&ID, slice, slots, &layout);

		assert_eq!(
			opt_mut_64,
			compact_32,
			"mismatch vs compact_32 for slots={slots} mut={mut_mask:#b} opt={opt_mask:#b} len={}",
			slice.len()
		);
		assert_eq!(
			opt_mut_64,
			general,
			"mismatch vs general for slots={slots} mut={mut_mask:#b} opt={opt_mask:#b} len={}",
			slice.len()
		);
	};

	// Part A: exhaustive over all 4^3 = 64 3-slot layouts (including optional
	// readonly slots) and all slices of length 0..=3 from 6 accounts (259 slices
	// -> 16,576 combinations; smaller account pool under Miri).
	let pool_a_len = if cfg!(miri) { 4 } else { 6 };
	let pool_a_indices: &[usize] = if cfg!(miri) {
		&[0, 2, 4, 5]
	} else {
		&[0, 1, 2, 3, 4, 5]
	};
	for layout_code in 0_u32..64 {
		let mut mut_mask = 0_u64;
		let mut opt_mask = 0_u64;
		for bit in 0..3 {
			let kind = (layout_code >> (bit * 2)) & 0b11;
			// 0 = ro, 1 = opt_ro, 2 = req_mut, 3 = opt_mut
			if kind == 1 || kind == 3 {
				opt_mask |= 1_u64 << bit;
			}
			if kind == 2 || kind == 3 {
				mut_mask |= 1_u64 << bit;
			}
		}

		// Length 0
		assert_all_match(3, mut_mask, opt_mask, &[]);
		// Length 1
		for &a in &pool_a_indices[..pool_a_len] {
			assert_all_match(3, mut_mask, opt_mask, &[pool[a]]);
			// Length 2
			for &b in &pool_a_indices[..pool_a_len] {
				assert_all_match(3, mut_mask, opt_mask, &[pool[a], pool[b]]);
				// Length 3
				for &c in &pool_a_indices[..pool_a_len] {
					assert_all_match(3, mut_mask, opt_mask, &[pool[a], pool[b], pool[c]]);
				}
			}
		}
	}

	// Part B: exhaustive over all 3^4 = 81 4-slot layouts (`ro`, `req_mut`,
	// `opt_mut`) and all slices of length 0..=4 drawn from 5 accounts
	// (`rw(1)`, `rw(2)`, `ro(3)`, `rw(ID)`, `ro(ID)`) -> 63,261 combinations.
	#[cfg(not(miri))]
	{
		let pool_b = [pool[0], pool[1], pool[2], pool[4], pool[5]];
		for mut code in 0_u32..81 {
			let mut mut_mask = 0_u64;
			let mut opt_mask = 0_u64;
			for bit in 0..4 {
				let kind = code % 3;
				code /= 3;
				// 0 = ro, 1 = req_mut, 2 = opt_mut
				if kind == 1 || kind == 2 {
					mut_mask |= 1_u64 << bit;
				}
				if kind == 2 {
					opt_mask |= 1_u64 << bit;
				}
			}

			assert_all_match(4, mut_mask, opt_mask, &[]);
			for &a in &pool_b {
				assert_all_match(4, mut_mask, opt_mask, &[a]);
				for &b in &pool_b {
					assert_all_match(4, mut_mask, opt_mask, &[a, b]);
					for &c in &pool_b {
						assert_all_match(4, mut_mask, opt_mask, &[a, b, c]);
						for &d in &pool_b {
							assert_all_match(4, mut_mask, opt_mask, &[a, b, c, d]);
						}
					}
				}
			}
		}
	}
}

#[test]
fn cold_route_and_shared_dispatch_accounts_match_direct_path() {
	let dup: ProgramError = PinaProgramError::DuplicateMutableAccount.into();

	// Shared accounts struct across multiple `#[dispatch(accounts = ...)]`
	// variants (`SharedMutPairA`, `SharedMutPairB`) parses and enforces aliasing
	// identically on both dispatch (`__parse_accounts_inlined`) and direct paths.
	for ix in [
		AliasInstruction::SharedMutPairA,
		AliasInstruction::SharedMutPairB,
	] {
		assert_both_accept(ix, &[rw(1), rw(2)], std::vec![1, 2]);
		assert_both_reject(ix, &[rw(1), Slot::Duplicate(0)], &dup);
		assert_both_reject(
			ix,
			&[ro(1), Slot::Duplicate(0)],
			&ProgramError::InvalidAccountData,
		);
		assert_both_reject(ix, &[rw(1)], &ProgramError::NotEnoughAccountKeys);
	}

	// `COLD_ROUTE = true` (`run_dispatch`) matches `COLD_ROUTE = false`
	// (`run_direct`) on 3-mut and 4-mut simple layouts even when an earlier
	// writable duplicate precedes a later readonly slot.
	assert_both_reject(
		AliasInstruction::MutTriple,
		&[rw(1), Slot::Duplicate(0), ro(2)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::MutQuad,
		&[ro(9), rw(1), Slot::Duplicate(1), rw(2), ro(3)],
		&dup,
	);

	// `COLD_ROUTE = true` (`validate_account_aliasing_compact`) matches
	// `COLD_ROUTE = false` (inline <=4 unique fast path) across distinct,
	// allowed ro+mut overlap, aliased unique, aliased mut, writability-before-
	// duplicate precedence, and short-slice inputs.
	const {
		assert!(!UniqueQuadAccounts::ALIAS_LAYOUT.is_mut_only_64());
		assert!(UniqueQuadAccounts::ALIAS_LAYOUT.is_compact_32());
		assert!(UniqueQuadAccounts::ALIAS_LAYOUT.optional_mask[0] == 0);
		assert!(UniqueQuadAccounts::ACCOUNT_LIMIT == 4);
	}
	assert_both_accept(
		AliasInstruction::UniqueQuad,
		&[ro(1), ro(2), rw(3), rw(4)],
		std::vec![1, 2, 3, 4],
	);
	// Allowed overlap between non-unique `ro_slot` (0) and `mut_a` (2) sets
	// `DUP_ANY_MASK | DUP_WRITABLE_MASK`, exercising `COLD_ROUTE = true`.
	assert_both_accept(
		AliasInstruction::UniqueQuad,
		&[rw(1), ro(2), Slot::Duplicate(0), rw(4)],
		std::vec![1, 2, 1, 4],
	);
	assert_both_reject(
		AliasInstruction::UniqueQuad,
		&[ro(1), Slot::Duplicate(0), rw(3), rw(4)],
		&dup,
	);
	// Earlier readonly unique duplicate + later non-writable mut reports
	// `InvalidAccountData` first on both paths.
	assert_both_reject(
		AliasInstruction::UniqueQuad,
		&[ro(1), Slot::Duplicate(0), ro(3), rw(4)],
		&ProgramError::InvalidAccountData,
	);
	assert_both_reject(
		AliasInstruction::UniqueQuad,
		&[ro(1), ro(2), rw(3), Slot::Duplicate(2)],
		&dup,
	);
	assert_both_reject(
		AliasInstruction::UniqueQuad,
		&[ro(1), ro(2), rw(3)],
		&ProgramError::NotEnoughAccountKeys,
	);
}
