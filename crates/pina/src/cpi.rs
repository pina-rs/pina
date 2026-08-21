//! CPI and account-allocation helpers used by on-chain instruction handlers.
//!
//! These utilities wrap common system-program patterns (create, allocate,
//! assign, close) with consistent `ProgramError` behavior and PDA signing.
//! All APIs in this module are designed for on-chain determinism and return
//! `ProgramError` values for caller-side propagation with `?` instead of
//! panicking.
//!
//! Seed-based helpers require deterministic seed ordering and consistent
//! program IDs across derivation and verification.

use pinocchio::AccountView;
use pinocchio::Address;
#[cfg(feature = "account-resize")]
use pinocchio::Resize;
use pinocchio::cpi::Seed;
use pinocchio::cpi::Signer;
use pinocchio::error::ProgramError;
use pinocchio::instruction::InstructionAccount;
use pinocchio::instruction::InstructionView;
use pinocchio::sysvars::Sysvar;
use pinocchio::sysvars::rent::Rent;
use pinocchio_system::instructions::Allocate;
use pinocchio_system::instructions::Assign;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_system::instructions::Transfer;

use crate::AccountInfoValidation;
use crate::CloseAccountWithRecipient;
#[cfg(feature = "account-resize")]
use crate::LamportTransfer;
use crate::MAX_SEEDS;
use crate::PinaAccount;
use crate::ProgramResult;

/// Creates a new system account owned by `owner`.
///
/// Calculates the rent-exempt balance for `space`, then issues a single
/// `CreateAccount` CPI from `from` to `to`.
///
/// # Errors
///
/// Returns errors from rent sysvar access, rent minimum-balance computation,
/// or the underlying system-program CPI.
///
/// # Examples
///
/// ```ignore
/// use pina::cpi::create_account;
///
/// // Create a new account with 128 bytes of space owned by `program_id`:
/// create_account(payer, new_account, 128, &program_id)?;
/// ```
#[inline(always)]
pub fn create_account<'a>(
	from: &'a AccountView,
	to: &'a AccountView,
	space: usize,
	owner: &Address,
) -> ProgramResult {
	let lamports = Rent::get()?.try_minimum_balance(space)?;

	CreateAccount {
		from,
		to,
		lamports,
		space: space as u64,
		owner,
	}
	.invoke()
}

/// Creates a new PDA-backed program account and returns `(address, bump)`.
///
/// This helper derives the canonical PDA for `seeds` + `owner`, allocates
/// account storage for `T`, initializes its discriminator, and assigns account
/// ownership to `owner`.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns `InvalidSeeds` when no valid PDA can be derived, plus any errors
/// from allocation/assignment steps.
///
/// # Examples
///
/// ```ignore
/// // Create a PDA-backed escrow account:
/// let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
/// let (address, bump) =
/// 	create_program_account::<EscrowState>(escrow_account, payer, &program_id, seeds)?;
/// ```
#[inline(always)]
pub fn create_program_account<'a, T: PinaAccount>(
	target_account: &'a mut AccountView,
	payer: &'a AccountView,
	owner: &Address,
	seeds: &[&[u8]],
) -> Result<(Address, u8), ProgramError> {
	let Some((address, bump)) = crate::try_find_program_address(seeds, owner) else {
		return Err(ProgramError::InvalidSeeds);
	};

	create_program_account_with_bump::<T>(target_account, payer, owner, seeds, bump)?;

	Ok((address, bump))
}

/// Creates a new PDA-backed program account using a caller-provided `bump` and
/// initializes `T`'s discriminator.
///
/// Prefer [`create_program_account`] when you want canonical bump derivation.
/// Use this function when the bump is instruction data and must be validated.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns any error produced by [`allocate_account_with_bump`], including
/// invalid seed layouts and system-program CPI failures.
///
/// # Examples
///
/// ```ignore
/// // Create a PDA-backed account when you already know the bump:
/// let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
/// create_program_account_with_bump::<EscrowState>(
/// 	escrow_account, payer, &program_id, seeds, bump,
/// )?;
/// ```
#[inline(always)]
pub fn create_program_account_with_bump<'a, T: PinaAccount>(
	target_account: &'a mut AccountView,
	payer: &'a AccountView,
	owner: &Address,
	seeds: &[&[u8]],
	bump: u8,
) -> ProgramResult {
	// Allocate space, then initialize the discriminator. Callers can obtain a
	// typed view immediately when the schema's remaining zeroed fields form a
	// valid zeropod representation.
	allocate_account_with_bump(target_account, payer, T::SIZE, owner, seeds, bump)?;
	{
		let mut data = target_account.try_borrow_mut()?;
		T::write_discriminator(&mut data);
	}

	Ok(())
}

/// Allocates space for a new program account, returning the derived `address`
/// and the canonical `bump`.
///
/// This is the lower-level allocator used by [`create_program_account`] for
/// cases where caller code wants manual discriminator/data initialization.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns `InvalidSeeds` when no canonical PDA can be derived, plus any
/// allocation errors surfaced by [`allocate_account_with_bump`].
///
/// # Examples
///
/// ```ignore
/// // Allocate raw space for manual initialization:
/// let seeds: &[&[u8]] = &[b"vault"];
/// let (address, bump) =
/// 	allocate_account(vault_account, payer, 64, &program_id, seeds)?;
/// ```
#[inline(always)]
pub fn allocate_account<'a>(
	target_account: &'a AccountView,
	payer: &'a AccountView,
	space: usize,
	owner: &Address,
	seeds: &[&[u8]],
) -> Result<(Address, u8), ProgramError> {
	let Some((address, bump)) = crate::try_find_program_address(seeds, owner) else {
		return Err(ProgramError::InvalidSeeds);
	};

	allocate_account_with_bump(target_account, payer, space, owner, seeds, bump)?;

	Ok((address, bump))
}

/// Appends a single-byte bump seed to the provided seeds array, returning
/// a fixed-size `[Seed; MAX_SEEDS]` suitable for PDA signing.
///
/// # Errors
///
/// Returns `ProgramError::InvalidSeeds` if `seeds.len() >= MAX_SEEDS`.
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Examples
///
/// ```ignore
/// let escrow_seeds = EscrowState::seeds(&maker, seed).with_bump(bump);
/// let escrow_signer = escrow_seeds.to_signer();
/// let signers = [escrow_signer.as_signer()];
/// ```
///
/// For untyped seed slices, use this lower-level helper directly:
///
/// ```ignore
/// let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
/// let bump_bytes = [bump];
/// let combined = combine_seeds_with_bump(seeds, &bump_bytes)?;
/// let signer = Signer::from(&combined[..=seeds.len()]);
/// ```
pub fn combine_seeds_with_bump<'a>(
	seeds: &[&'a [u8]],
	bump: &'a [u8; 1],
) -> Result<[Seed<'a>; MAX_SEEDS], ProgramError> {
	if seeds.len() >= MAX_SEEDS {
		return Err(ProgramError::InvalidSeeds);
	}

	// Create our backing storage on the stack, initialized with empty seeds.
	let mut storage: [Seed<'a>; MAX_SEEDS] = core::array::from_fn(|_| Seed::from(&[] as &[u8]));

	// 1. Copy the original seeds into our storage array.
	for (i, seed) in seeds.iter().enumerate() {
		storage[i] = Seed::from(*seed);
	}

	// 2. Add the single-byte bump slice to the end.
	let seeds_len = seeds.len();
	storage[seeds_len] = Seed::from(bump.as_slice());

	Ok(storage)
}

/// Stack-backed PDA signer seeds for Pinocchio CPI calls.
///
/// Pinocchio's [`Signer`] borrows a seed array. This type owns that fixed-size
/// array so generated PDA helpers can return one compact value, and callers can
/// pass [`PdaSigner::as_signer`] into `invoke_signed` APIs without manually
/// assembling temporary seed storage.
#[derive(Clone, Debug)]
#[must_use]
pub struct PdaSigner<'a, const SEEDS: usize> {
	seeds: [Seed<'a>; SEEDS],
}

impl<'a, const SEEDS: usize> PdaSigner<'a, SEEDS> {
	/// Build a PDA signer from byte-slice seeds.
	#[inline(always)]
	pub fn from_slices(seeds: [&'a [u8]; SEEDS]) -> Self {
		Self {
			seeds: seeds.map(Seed::from),
		}
	}

	/// Build a PDA signer from Pinocchio seed values.
	#[inline(always)]
	pub const fn from_seed_array(seeds: [Seed<'a>; SEEDS]) -> Self {
		Self { seeds }
	}

	/// Return the owned seed array.
	#[inline(always)]
	pub const fn as_seeds(&self) -> &[Seed<'a>; SEEDS] {
		&self.seeds
	}

	/// Borrow these seeds as a Pinocchio CPI signer.
	#[inline(always)]
	pub fn as_signer(&self) -> Signer<'a, '_> {
		Signer::from(&self.seeds)
	}
}

impl<'a, const SEEDS: usize> From<[Seed<'a>; SEEDS]> for PdaSigner<'a, SEEDS> {
	#[inline(always)]
	fn from(seeds: [Seed<'a>; SEEDS]) -> Self {
		Self::from_seed_array(seeds)
	}
}

impl<'a, const SEEDS: usize> From<[&'a [u8]; SEEDS]> for PdaSigner<'a, SEEDS> {
	#[inline(always)]
	fn from(seeds: [&'a [u8]; SEEDS]) -> Self {
		Self::from_slices(seeds)
	}
}

/// Allocates space for a new program account with user-provided bump.
///
/// Two paths are taken depending on whether the target account already has
/// lamports:
///
/// - **Zero balance** -- a single `CreateAccount` CPI is issued.
/// - **Non-zero balance** -- a `Transfer` (to top up rent), `Allocate`, and
///   `Assign` are issued separately. This covers the case where the account was
///   pre-funded (e.g. by a previous failed transaction).
///
/// <!-- {=pinaPdaSeedContract|trim|linePrefix:"/// ":true} -->
/// Seed-based APIs require deterministic seed ordering.
///
/// Program IDs must stay consistent across derivation and verification.
///
/// When a bump is required, prefer canonical bump derivation.
///
/// Use explicit bumps when needed.<!-- {/pinaPdaSeedContract} -->
///
/// # Errors
///
/// Returns seed-validation errors, rent sysvar access errors, and any
/// system-program CPI failure from `CreateAccount`, `Transfer`, `Allocate`, or
/// `Assign`.
///
/// # Examples
///
/// ```ignore
/// let seeds: &[&[u8]] = &[b"vault"];
/// allocate_account_with_bump(vault_account, payer, 64, &program_id, seeds, bump)?;
/// ```
#[inline(always)]
pub fn allocate_account_with_bump<'a>(
	target_account: &'a AccountView,
	payer: &'a AccountView,
	space: usize,
	owner: &Address,
	seeds: &[&[u8]],
	bump: u8,
) -> ProgramResult {
	// Combine seeds
	let bump_array = [bump];
	let combined_seeds = combine_seeds_with_bump(seeds, &bump_array)?;
	let mut derivation_seeds: [&[u8]; MAX_SEEDS] = [&[]; MAX_SEEDS];
	derivation_seeds[..seeds.len()].copy_from_slice(seeds);
	derivation_seeds[seeds.len()] = bump_array.as_slice();
	let expected_address = crate::create_program_address(&derivation_seeds[..=seeds.len()], owner)?;
	if target_account.address() != &expected_address {
		return Err(ProgramError::InvalidSeeds);
	}
	let seeds_slice = &combined_seeds[..=seeds.len()];
	let signer = Signer::from(seeds_slice);
	let signers = &[signer];

	// Allocate space for account
	let rent = Rent::get()?;

	if target_account.lamports().eq(&0) {
		let lamports = rent.try_minimum_balance(space)?;

		CreateAccount {
			from: payer,
			to: target_account,
			lamports,
			space: space as u64,
			owner,
		}
		.invoke_signed(signers)?;

		return Ok(());
	}

	// Otherwise, if balance is nonzero:

	// 1) transfer sufficient lamports for rent exemption
	let rent_exempt_balance = rent
		.try_minimum_balance(space)?
		.saturating_sub(target_account.lamports());

	if rent_exempt_balance > 0 {
		Transfer {
			from: payer,
			to: target_account,
			lamports: rent_exempt_balance,
		}
		.invoke_signed(signers)?;
	}

	// 2) allocate space for the account
	Allocate {
		account: target_account,
		space: space as u64,
	}
	.invoke_signed(signers)?;

	// 3) assign our program as the owner
	Assign {
		account: target_account,
		owner,
	}
	.invoke_signed(signers)?;

	Ok(())
}

/// Maximum number of bytes an account may grow by in a single instruction.
///
/// This limit is enforced by the Solana runtime. Attempting to grow an account
/// by more than this amount returns `ProgramError::InvalidRealloc`.
#[cfg(feature = "account-resize")]
pub const MAX_PERMITTED_DATA_INCREASE: usize = pinocchio::account::MAX_PERMITTED_DATA_INCREASE;

/// Reallocates an account to `new_size` bytes, adjusting rent automatically.
///
/// When **growing**, transfers the additional rent-exempt lamports required from
/// `payer` to `account` via a system-program CPI. When **shrinking**, returns
/// excess rent lamports from `account` to `payer` by direct lamport
/// manipulation (the account must be owned by the executing program for this
/// path).
///
/// New bytes are zero-initialized by the Solana runtime.
///
/// # Limits
///
/// The Solana runtime limits account growth to [`MAX_PERMITTED_DATA_INCREASE`]
/// (10 KiB) per top-level instruction. Exceeding this limit returns
/// `ProgramError::InvalidRealloc`.
///
/// This helper rejects a single growth request larger than that limit before
/// moving rent lamports. Pinocchio does not expose the account's original
/// serialized length, so a later growth after an earlier reallocation in the
/// same instruction can still fail during `resize` after rent lamports have
/// moved. Propagate that error instead of catching it to preserve transaction
/// atomicity.
///
/// # Errors
///
/// Returns `ProgramError::InvalidAccountData` if the account is not writable,
/// `ProgramError::InvalidAccountOwner` if the account is not owned by
/// `program_id`, and propagates any errors from rent sysvar access, lamport
/// transfer, or the runtime `resize` call.
#[cfg(feature = "account-resize")]
#[inline(always)]
pub fn realloc_account(
	account: &mut AccountView,
	new_size: usize,
	payer: &mut AccountView,
	program_id: &Address,
) -> ProgramResult {
	realloc_account_inner(account, new_size, payer, program_id)
}

/// Reallocates an account to `new_size` bytes with explicit zero-initialization,
/// adjusting rent automatically.
///
/// This function behaves identically to [`realloc_account`]. In the current
/// Solana runtime, new bytes are always zero-initialized regardless of which
/// variant is called. This function exists for API symmetry with the runtime's
/// `realloc(new_len, zero_init)` parameter and to make zero-initialization
/// intent explicit at the call site.
///
/// When **growing**, transfers the additional rent-exempt lamports required from
/// `payer` to `account` via a system-program CPI. When **shrinking**, returns
/// excess rent lamports from `account` to `payer` by direct lamport
/// manipulation (the account must be owned by the executing program for this
/// path).
///
/// # Limits
///
/// The Solana runtime limits account growth to [`MAX_PERMITTED_DATA_INCREASE`]
/// (10 KiB) per top-level instruction. Exceeding this limit returns
/// `ProgramError::InvalidRealloc`.
///
/// This helper rejects a single growth request larger than that limit before
/// moving rent lamports. Pinocchio does not expose the account's original
/// serialized length, so a later growth after an earlier reallocation in the
/// same instruction can still fail during `resize` after rent lamports have
/// moved. Propagate that error instead of catching it to preserve transaction
/// atomicity.
///
/// # Errors
///
/// Returns `ProgramError::InvalidAccountData` if the account is not writable,
/// `ProgramError::InvalidAccountOwner` if the account is not owned by
/// `program_id`, and propagates any errors from rent sysvar access, lamport
/// transfer, or the runtime `resize` call.
#[cfg(feature = "account-resize")]
#[inline(always)]
pub fn realloc_account_zero(
	account: &mut AccountView,
	new_size: usize,
	payer: &mut AccountView,
	program_id: &Address,
) -> ProgramResult {
	realloc_account_inner(account, new_size, payer, program_id)
}

/// Shared implementation for [`realloc_account`] and [`realloc_account_zero`].
///
/// Validates the account, computes the rent delta, performs the lamport
/// transfer, and resizes the account data.
#[cfg(feature = "account-resize")]
#[inline(always)]
fn realloc_account_inner(
	account: &mut AccountView,
	new_size: usize,
	payer: &mut AccountView,
	program_id: &Address,
) -> ProgramResult {
	use crate::AccountInfoValidation;

	// Validate the account is writable and owned by the program.
	account.assert_writable()?.assert_owner(program_id)?;

	let current_size = account.data_len();

	// Early return when the size is unchanged.
	if new_size == current_size {
		return Ok(());
	}

	// `resize` would reject either condition after rent movement. The original
	// serialized length is not exposed, so cumulative growth is still checked by
	// the runtime and its error must be propagated by callers.
	account.check_borrow_mut()?;
	if new_size.saturating_sub(current_size) > MAX_PERMITTED_DATA_INCREASE {
		return Err(ProgramError::InvalidRealloc);
	}

	let rent = Rent::get()?;
	let new_minimum_balance = rent.try_minimum_balance(new_size)?;
	let current_lamports = account.lamports();

	if new_size > current_size {
		// Growing: transfer additional rent from payer to account.
		let required_lamports = new_minimum_balance.saturating_sub(current_lamports);
		if required_lamports > 0 {
			Transfer {
				from: payer,
				to: account,
				lamports: required_lamports,
			}
			.invoke()?;
		}
	} else {
		// Shrinking: return excess rent from account to payer.
		let excess_lamports = current_lamports.saturating_sub(new_minimum_balance);
		if excess_lamports > 0 {
			account.send(excess_lamports, payer)?;
		}
	}

	// Resize the account data. The runtime zero-initializes new bytes.
	account.resize(new_size)
}

/// Closes an account and returns the remaining rent lamports to the provided
/// recipient.
///
/// Callers should clear any program-owned account state before closing when
/// stale data reuse matters for their threat model.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->
/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
/// No panics needed.<!-- {/pinaPublicResultContract} -->
///
/// # Errors
///
/// Returns errors from lamport transfer or account close operations.
///
/// # Examples
///
/// ```ignore
/// // Close the escrow account and return rent to the authority:
/// close_account(escrow_account, authority)?;
/// ```
#[inline(always)]
pub fn close_account(account_info: &mut AccountView, recipient: &mut AccountView) -> ProgramResult {
	account_info.close_with_recipient(recipient)
}

/// Closes an account after zeroing its current data bytes in-place.
///
/// This helper clears the raw account data before transferring the remaining
/// lamports and closing the account. It does not implicitly reallocate the
/// account, even when the `account-resize` feature is enabled.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->
/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
/// No panics needed.<!-- {/pinaPublicResultContract} -->
///
/// # Errors
///
/// Returns errors from account borrowing, lamport transfer, or account close
/// operations.
///
/// # Examples
///
/// ```ignore
/// // Zero the raw account bytes, then close the account and return rent:
/// close_account_zeroed(escrow_account, authority)?;
/// ```
#[inline(always)]
pub fn close_account_zeroed(
	account_info: &mut AccountView,
	recipient: &mut AccountView,
) -> ProgramResult {
	account_info.close_account_zeroed(recipient)
}

/// Typed handle for passing validated accounts into CPI builders.
///
/// This is a lightweight, allocator-free wrapper around `&AccountView` plus the
/// writable bit the callee should observe. It is intentionally separate from
/// the raw `AccountView` so callers can build typed CPI account structs without
/// immediately reaching for unchecked runtime APIs.
///
/// This prototype keeps Pina's current architecture constraints intact:
///
/// - no heap allocation in the on-chain CPI path
/// - no `unsafe` in the wrapper layer
/// - const-generic account counts instead of `Vec`
/// - checked `pinocchio::cpi::invoke_signed` as the execution path for now
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct CpiHandle<'a> {
	view: &'a AccountView,
	writable: bool,
	signer: bool,
}

impl<'a> CpiHandle<'a> {
	/// Construct a read-only CPI handle.
	#[inline(always)]
	pub const fn readonly(view: &'a AccountView) -> Self {
		Self {
			view,
			writable: false,
			signer: false,
		}
	}

	/// Construct a read-only handle that the callee requires to sign.
	///
	/// The signer bit describes the target instruction schema, not the outer
	/// instruction's current privileges. This permits [`Signer`] seeds passed to
	/// [`CpiContext::invoke`] to satisfy PDA signer requirements.
	#[inline(always)]
	pub const fn readonly_signer(view: &'a AccountView) -> Self {
		Self {
			view,
			writable: false,
			signer: true,
		}
	}

	/// Construct a writable CPI handle.
	///
	/// Returns `InvalidAccountData` when the source account was not declared
	/// writable in the current instruction.
	#[inline(always)]
	pub fn writable(view: &'a AccountView) -> Result<Self, ProgramError> {
		if !view.is_writable() {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(Self {
			view,
			writable: true,
			signer: false,
		})
	}

	/// Construct a writable handle that the callee requires to sign.
	///
	/// Returns `InvalidAccountData` when the source account was not declared
	/// writable in the current instruction. The runtime verifies the requested
	/// signer privilege during invocation or derives it from supplied PDA seeds.
	#[inline(always)]
	pub fn writable_signer(view: &'a AccountView) -> Result<Self, ProgramError> {
		if !view.is_writable() {
			return Err(ProgramError::InvalidAccountData);
		}

		Ok(Self {
			view,
			writable: true,
			signer: true,
		})
	}

	/// Return the account address with the original borrow lifetime.
	#[inline(always)]
	pub fn address(&self) -> &'a Address {
		self.view.address()
	}

	/// Whether this handle should be passed to the callee as writable.
	#[inline(always)]
	pub const fn is_writable(&self) -> bool {
		self.writable
	}

	/// Whether the target instruction requires this account to sign.
	#[inline(always)]
	pub const fn is_signer(&self) -> bool {
		self.signer
	}

	#[inline(always)]
	fn instruction_account(self) -> InstructionAccount<'a> {
		InstructionAccount::new(self.address(), self.is_writable(), self.is_signer())
	}

	#[inline(always)]
	fn account_view(self) -> &'a AccountView {
		self.view
	}
}

/// Marker trait for a known CPI target program.
///
/// Generated CPI modules should emit a zero-sized program marker that
/// implements this trait. Pair it with [`Program`] to validate the executable
/// account once, before constructing Pinocchio-style instruction builders.
pub trait CpiProgramId {
	/// Canonical program address.
	const ID: Address;
}

/// A validated CPI target program account.
///
/// This wrapper makes program-ID verification structural for generated CPI
/// builders. It stores the original account view so callers can keep the
/// executable account in the same typed accounts struct as the rest of the CPI
/// inputs.
#[must_use]
pub struct Program<'a, T: CpiProgramId> {
	account: &'a AccountView,
	_marker: core::marker::PhantomData<T>,
}

impl<T: CpiProgramId> Clone for Program<'_, T> {
	#[inline(always)]
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: CpiProgramId> Copy for Program<'_, T> {}

impl<T: CpiProgramId> core::fmt::Debug for Program<'_, T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("Program")
			.field("account", &self.account)
			.finish_non_exhaustive()
	}
}

impl<'a, T: CpiProgramId> Program<'a, T> {
	/// Validate `account` as the expected executable program.
	#[inline(always)]
	pub fn new(account: &'a AccountView) -> Result<Self, ProgramError> {
		account.assert_program(&T::ID)?;

		Ok(Self {
			account,
			_marker: core::marker::PhantomData,
		})
	}

	/// The validated program account.
	#[inline(always)]
	pub const fn account(&self) -> &'a AccountView {
		self.account
	}

	/// The validated program address.
	#[inline(always)]
	pub fn address(&self) -> &'a Address {
		self.account.address()
	}
}

/// Convert a typed CPI accounts struct into a fixed-size array of handles.
///
/// This is the no-allocation counterpart to Anchor lang-v2's `ToCpiAccounts`.
/// The const generic keeps the final account count explicit at compile time so
/// Pina can stay within its allocator-free on-chain boundary.
pub trait ToCpiAccounts<'a, const ACCOUNTS: usize> {
	/// Collect the handles in the exact order expected by the callee
	/// instruction.
	fn to_cpi_handles(&self) -> [CpiHandle<'a>; ACCOUNTS];
}

/// Minimal typed CPI context built around [`CpiHandle`] and const generics.
///
/// This prototype intentionally omits heap-backed remaining-account lists.
/// Callers should include every required account in the typed accounts struct
/// for now. A future cursor-based account runtime can extend this with richer
/// remaining-account support while preserving `no_std` compatibility.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct CpiContext<'a, P, T, const ACCOUNTS: usize>
where
	P: CpiProgramId,
	T: ToCpiAccounts<'a, ACCOUNTS>,
{
	pub accounts: T,
	pub program: Program<'a, P>,
}

impl<'a, P, T, const ACCOUNTS: usize> CpiContext<'a, P, T, ACCOUNTS>
where
	P: CpiProgramId,
	T: ToCpiAccounts<'a, ACCOUNTS>,
{
	#[inline(always)]
	pub const fn new(program: Program<'a, P>, accounts: T) -> Self {
		Self { accounts, program }
	}

	/// Invoke the CPI using pinocchio's checked static-array path.
	///
	/// This keeps the prototype allocator-free and avoids introducing unsafe
	/// unchecked invocation until Pina has a stronger typed account runtime for
	/// duplicate-account and alias analysis.
	#[inline(always)]
	pub fn invoke(&self, data: &[u8], signers: &[Signer<'_, '_>]) -> ProgramResult {
		let handles = self.accounts.to_cpi_handles();
		let instruction_accounts = handles.map(CpiHandle::instruction_account);
		let account_views = handles.map(CpiHandle::account_view);
		let instruction = InstructionView {
			program_id: self.program.address(),
			data,
			accounts: &instruction_accounts,
		};

		pinocchio::cpi::invoke_signed::<ACCOUNTS, _>(&instruction, &account_views, signers)
	}
}
