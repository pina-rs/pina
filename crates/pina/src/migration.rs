//! Runtime contracts for generated, on-demand ABI migrations.
//!
//! Migration versions live immediately after an existing discriminator. The
//! framework owns these bytes. Normal generated accessors and patches do not
//! expose them.

use core::fmt::Debug;

use crate::HasDiscriminator;
use crate::IntoDiscriminator;
use crate::PinaProgramError;
use crate::ProgramError;
use crate::ProgramResult;

mod private {
	pub trait Sealed {}

	impl Sealed for u8 {}
	impl Sealed for u16 {}
	impl Sealed for u32 {}
}

/// Integer encoding used by the program-wide migration version envelope.
///
/// Pina implements this trait only for `u8`, `u16`, and `u32`. A program picks
/// one encoding globally in `pina.toml`; generated code selects the matching
/// implementation for every migratable account, instruction, and event.
pub trait MigrationVersion:
	private::Sealed + Copy + Debug + Eq + Ord + Send + Sync + 'static
{
	/// Number of bytes occupied by this encoding.
	const BYTES: usize;

	/// Largest representable version, normalized to `u32`.
	const MAX: u32;

	/// Decode one little-endian version from the start of `bytes`.
	fn read_le(bytes: &[u8]) -> Result<Self, ProgramError>;

	/// Encode this version at the start of `bytes`.
	fn write_le(self, bytes: &mut [u8]) -> ProgramResult;

	/// Normalize this version for generated dispatch tables and diagnostics.
	fn into_u32(self) -> u32;

	/// Convert a generated version number into this encoding.
	fn try_from_u32(value: u32) -> Result<Self, ProgramError>;
}

macro_rules! impl_migration_version {
	($type:ty) => {
		impl MigrationVersion for $type {
			const BYTES: usize = size_of::<Self>();
			const MAX: u32 = <$type>::MAX as u32;

			#[inline(always)]
			fn read_le(bytes: &[u8]) -> Result<Self, ProgramError> {
				let source = bytes
					.get(..<Self as MigrationVersion>::BYTES)
					.ok_or(PinaProgramError::DataTooShort)?;
				let mut encoded = [0_u8; size_of::<Self>()];
				encoded.copy_from_slice(source);

				Ok(Self::from_le_bytes(encoded))
			}

			#[inline(always)]
			fn write_le(self, bytes: &mut [u8]) -> ProgramResult {
				let destination = bytes
					.get_mut(..<Self as MigrationVersion>::BYTES)
					.ok_or(PinaProgramError::DataTooShort)?;
				destination.copy_from_slice(&self.to_le_bytes());

				Ok(())
			}

			#[inline(always)]
			fn into_u32(self) -> u32 {
				self as u32
			}

			#[inline(always)]
			fn try_from_u32(value: u32) -> Result<Self, ProgramError> {
				Self::try_from(value).map_err(|_| PinaProgramError::InvalidMigrationVersion.into())
			}
		}
	};
}

impl_migration_version!(u8);
impl_migration_version!(u16);
impl_migration_version!(u32);

/// Relationship between a stored version and the generated current version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredVersion<V> {
	/// The bytes use the current representation.
	Current(V),

	/// The bytes use a known or potentially supported historical representation.
	Stale {
		/// Version read from the wire envelope.
		stored: V,
		/// Current version compiled into the program.
		current: V,
	},

	/// The bytes were written by a newer or corrupt representation.
	Future {
		/// Version read from the wire envelope.
		stored: V,
		/// Current version compiled into the program.
		current: V,
	},
}

/// Framework-owned version envelope implemented by generated code.
///
/// The version starts immediately after the discriminator. Implementations are
/// generated from the checked-in migration history; application code should not
/// implement this trait manually.
pub trait HasMigrationVersion: HasDiscriminator {
	/// Program-wide integer encoding selected in `pina.toml`.
	type Version: MigrationVersion;

	/// Current schema version generated for this wire contract.
	const CURRENT_VERSION: Self::Version;

	/// Byte offset of the version field.
	const VERSION_OFFSET: usize = <Self::Type as IntoDiscriminator>::BYTES;

	/// Total byte length of the discriminator and version envelope.
	const MIGRATION_HEADER_SIZE: usize =
		Self::VERSION_OFFSET + <Self::Version as MigrationVersion>::BYTES;

	/// Read the version without attempting to decode a payload.
	#[inline(always)]
	fn read_migration_version(data: &[u8]) -> Result<Self::Version, ProgramError> {
		let bytes = data
			.get(Self::VERSION_OFFSET..Self::MIGRATION_HEADER_SIZE)
			.ok_or(PinaProgramError::DataTooShort)?;

		Self::Version::read_le(bytes)
	}

	/// Compare stored and current versions without trying historical decoders.
	#[inline(always)]
	fn inspect_migration_version(
		data: &[u8],
	) -> Result<StoredVersion<Self::Version>, ProgramError> {
		let stored = Self::read_migration_version(data)?;
		let current = Self::CURRENT_VERSION;

		Ok(match stored.cmp(&current) {
			core::cmp::Ordering::Equal => StoredVersion::Current(stored),
			core::cmp::Ordering::Less => StoredVersion::Stale { stored, current },
			core::cmp::Ordering::Greater => StoredVersion::Future { stored, current },
		})
	}

	/// Write the generated current version into an initialized envelope.
	#[inline(always)]
	fn write_current_migration_version(data: &mut [u8]) -> ProgramResult {
		let bytes = data
			.get_mut(Self::VERSION_OFFSET..Self::MIGRATION_HEADER_SIZE)
			.ok_or(PinaProgramError::DataTooShort)?;

		Self::CURRENT_VERSION.write_le(bytes)
	}

	/// Require bytes that already use the current representation.
	///
	/// Historical dispatch calls [`Self::inspect_migration_version`] and selects
	/// an exact generated decoder. Ordinary current-only loaders use this method
	/// to avoid interpreting stale bytes as the latest layout.
	#[inline(always)]
	fn require_current_migration_version(data: &[u8]) -> ProgramResult {
		match Self::inspect_migration_version(data)? {
			StoredVersion::Current(_) => Ok(()),
			StoredVersion::Stale { .. } => Err(PinaProgramError::MigrationRequired.into()),
			StoredVersion::Future { .. } => Err(PinaProgramError::InvalidMigrationVersion.into()),
		}
	}
}

/// A current instruction byte slice produced by historical normalization.
#[derive(Debug, PartialEq, Eq)]
pub enum CurrentInstructionData<'source, 'workspace> {
	/// The request already used the current representation.
	Current(&'source [u8]),
	/// A historical request was rewritten into caller-owned workspace.
	Migrated(&'workspace [u8]),
}

impl CurrentInstructionData<'_, '_> {
	/// Return the exact current instruction representation.
	#[must_use]
	pub const fn as_bytes(&self) -> &[u8] {
		match self {
			Self::Current(data) => data,
			Self::Migrated(data) => data,
		}
	}

	/// Return whether a historical transition ran.
	#[must_use]
	pub const fn was_migrated(&self) -> bool {
		matches!(self, Self::Migrated(_))
	}
}

/// Generated conversion contract for one migratable instruction payload.
///
/// The caller owns the workspace so the runtime remains allocator-free.
/// Generated implementations validate one exact historical representation,
/// clear the workspace, run adjacent transitions, validate the destination,
/// and commit the current version marker last.
pub trait MigratableInstruction: HasMigrationVersion {
	/// Exact current instruction length.
	const CURRENT_SIZE: usize;

	/// Largest temporary byte region needed by any supported transition path.
	const WORKING_SIZE: usize;

	/// Maximum adjacent transitions accepted during ordinary dispatch.
	const MAX_INLINE_STEPS: u16;

	/// Rewrite one exact stale request into the workspace.
	fn migrate_stale_instruction(data: &[u8], workspace: &mut [u8]) -> ProgramResult;

	/// Validate the current representation without interpreting a stale layout.
	fn validate_current_instruction(data: &[u8]) -> ProgramResult;
}

/// Normalize current or historical instruction bytes before account parsing
/// and business logic.
///
/// Unknown and future versions fail without consulting a historical decoder.
/// The workspace is cleared before any old bytes are copied into it, preventing
/// caller-controlled residue from becoming a newly added field.
pub fn normalize_instruction_data<'source, 'workspace, T>(
	data: &'source [u8],
	workspace: &'workspace mut [u8],
) -> Result<CurrentInstructionData<'source, 'workspace>, ProgramError>
where
	T: MigratableInstruction,
{
	if !T::matches_discriminator(data) {
		return Err(ProgramError::InvalidInstructionData);
	}

	match T::inspect_migration_version(data)? {
		StoredVersion::Current(_) => {
			T::validate_current_instruction(data)?;
			Ok(CurrentInstructionData::Current(data))
		}
		StoredVersion::Future { .. } => Err(PinaProgramError::InvalidMigrationVersion.into()),
		StoredVersion::Stale { .. } => {
			if T::WORKING_SIZE < T::CURRENT_SIZE || workspace.len() < T::WORKING_SIZE {
				return Err(PinaProgramError::MigrationBudgetExceeded.into());
			}
			workspace[..T::WORKING_SIZE].fill(0);
			T::migrate_stale_instruction(data, &mut workspace[..T::WORKING_SIZE])?;
			T::write_current_migration_version(&mut workspace[..T::CURRENT_SIZE])?;
			T::validate_current_instruction(&workspace[..T::CURRENT_SIZE])?;

			Ok(CurrentInstructionData::Migrated(
				&workspace[..T::CURRENT_SIZE],
			))
		}
	}
}

/// Pure plan for one complete account migration to the current representation.
///
/// Generated planning code validates the exact historical source and owns
/// `payload`. The payload cannot borrow account data because this type has no
/// source lifetime. The executor can therefore release every account borrow
/// before rent adjustment or resize.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountMigrationPlan<P> {
	from_version: u32,
	to_version: u32,
	target_size: usize,
	working_size: usize,
	steps: u16,
	payload: P,
}

impl<P> AccountMigrationPlan<P> {
	/// Construct a generated migration plan.
	///
	/// # Errors
	///
	/// Returns `InvalidMigrationVersion` for a non-monotonic transition or a
	/// zero-step plan.
	pub fn try_new(
		from_version: u32,
		to_version: u32,
		target_size: usize,
		steps: u16,
		payload: P,
	) -> Result<Self, ProgramError> {
		Self::try_with_working_size(
			from_version,
			to_version,
			target_size,
			target_size,
			steps,
			payload,
		)
	}

	/// Construct a plan that needs a larger temporary account-data workspace.
	///
	/// `working_size` must be at least the final `target_size`. The executor grows
	/// to this size before applying the infallible rewrite and shrinks to the
	/// target only after the rewrite completes.
	pub fn try_with_working_size(
		from_version: u32,
		to_version: u32,
		target_size: usize,
		working_size: usize,
		steps: u16,
		payload: P,
	) -> Result<Self, ProgramError> {
		if from_version >= to_version || steps == 0 || working_size < target_size {
			return Err(PinaProgramError::InvalidMigrationVersion.into());
		}

		Ok(Self {
			from_version,
			to_version,
			target_size,
			working_size,
			steps,
			payload,
		})
	}

	/// Stored source version validated by the planner.
	#[must_use]
	pub const fn from_version(&self) -> u32 {
		self.from_version
	}

	/// Destination version. Generated automatic plans target the current version.
	#[must_use]
	pub const fn to_version(&self) -> u32 {
		self.to_version
	}

	/// Final physical account-data length.
	#[must_use]
	pub const fn target_size(&self) -> usize {
		self.target_size
	}

	/// Largest account-data length needed while adjacent transitions run.
	#[must_use]
	pub const fn working_size(&self) -> usize {
		self.working_size
	}

	/// Number of logical adjacent transitions represented by this plan.
	#[must_use]
	pub const fn steps(&self) -> u16 {
		self.steps
	}

	/// Consume the envelope and return its generated payload.
	#[must_use]
	pub fn into_payload(self) -> P {
		self.payload
	}
}

/// Generated conversion contract for one migratable account type.
///
/// Planning is fallible and effect-free. Applying an accepted plan is
/// infallible so application code cannot catch an error after a partial rewrite
/// and continue executing. The executor still validates the final bytes before
/// committing the current version marker.
pub trait MigratableAccount: HasMigrationVersion {
	/// Owned generated plan data. It must not contain account-data references.
	type Plan: 'static;

	/// Maximum number of adjacent transitions allowed during ordinary dispatch.
	const MAX_INLINE_STEPS: u16;

	/// Validate the selected historical representation and build a detached plan.
	fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError>;

	/// Rewrite `destination` according to a successfully preflighted plan.
	fn apply_migration(plan: Self::Plan, destination: &mut [u8]);

	/// Validate the destination representation without requiring its version
	/// bytes to have been committed yet.
	fn validate_migration_destination(data: &[u8]) -> ProgramResult;

	/// Validate the complete current representation, including the committed
	/// version envelope.
	#[inline(always)]
	fn validate_current_migration(data: &[u8]) -> ProgramResult {
		Self::require_current_migration_version(data)?;
		Self::validate_migration_destination(data)
	}
}

/// Result of an on-demand account migration attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountMigrationOutcome<V> {
	/// The account was already current and passed current-layout validation.
	AlreadyCurrent { version: V },

	/// The account reached the generated current representation.
	Migrated {
		/// Historical version read before mutation.
		from: V,
		/// Current version written after destination validation.
		to: V,
		/// Logical transitions represented by the generated plan.
		steps: u16,
	},
}

#[cfg(feature = "account-resize")]
mod executor {
	use pinocchio::Resize;
	use pinocchio::cpi::Signer;
	use pinocchio::sysvars::Sysvar;
	use pinocchio::sysvars::rent::Rent;
	use pinocchio_system::instructions::Transfer as SystemTransfer;

	use super::AccountMigrationOutcome;
	use super::MigratableAccount;
	use super::MigrationVersion;
	use super::StoredVersion;
	use crate::AccountInfoValidation;
	use crate::AccountView;
	use crate::Address;
	use crate::MAX_PERMITTED_DATA_INCREASE;
	use crate::PinaProgramError;
	use crate::ProgramError;
	use crate::ProgramResult;

	#[cold]
	#[inline(never)]
	fn abort_after_mutation(error: ProgramError) -> ! {
		panic!("account migration invariant failed after mutation: {error:?}")
	}

	fn finish_after_mutation<T>(result: Result<T, ProgramError>) -> T {
		match result {
			Ok(value) => value,
			Err(error) => abort_after_mutation(error),
		}
	}

	pub(super) fn validate_funding_payer(
		payer: &AccountView,
		account: &AccountView,
		may_sign_as_pda: bool,
	) -> ProgramResult {
		payer.assert_writable()?;
		if payer.address() == account.address() {
			return Err(PinaProgramError::DuplicateMutableAccount.into());
		}
		if !may_sign_as_pda {
			payer.assert_signer()?;
		}

		Ok(())
	}

	/// Execute a generated account migration without refunding surplus lamports.
	///
	/// The planner validates historical bytes while they are immutably borrowed.
	/// The borrow ends before funding, resize, and rewrite. Growth uses `payer`
	/// only for the rent deficit and never charges more than `max_lamports`.
	/// Shrinkage retains every lamport in the migrated account. Preflight failures
	/// return normally. An invariant failure after the first successful mutation
	/// aborts the instruction so application code cannot catch it and commit a
	/// partial migration.
	#[must_use = "account migration has no effect until invoke or invoke_signed is called"]
	pub struct MigrateAccount<'account, 'payer, 'address> {
		/// Program-owned account to inspect and, when stale, migrate.
		pub account: &'account mut AccountView,

		/// Optional signer that may fund a rent deficit during growth.
		pub payer: Option<&'payer AccountView>,

		/// Executing program ID used to validate account ownership.
		pub program_id: &'address Address,

		/// Maximum lamports this invocation may transfer from `payer`.
		pub max_lamports: u64,
	}

	impl MigrateAccount<'_, '_, '_> {
		/// Migrate using transaction-level signatures.
		pub fn invoke<T>(&mut self) -> Result<AccountMigrationOutcome<T::Version>, ProgramError>
		where
			T: MigratableAccount,
		{
			self.invoke_signed::<T>(&[])
		}

		/// Migrate with additional PDA signer seeds for the configured payer.
		pub fn invoke_signed<T>(
			&mut self,
			signers: &[Signer<'_, '_>],
		) -> Result<AccountMigrationOutcome<T::Version>, ProgramError>
		where
			T: MigratableAccount,
		{
			self.invoke_signed_inner::<T>(signers, None)
		}

		#[cfg(test)]
		pub(super) fn invoke_with_rent<T>(
			&mut self,
			rent: Rent,
		) -> Result<AccountMigrationOutcome<T::Version>, ProgramError>
		where
			T: MigratableAccount,
		{
			self.invoke_signed_inner::<T>(&[], Some(rent))
		}

		fn invoke_signed_inner<T>(
			&mut self,
			signers: &[Signer<'_, '_>],
			rent: Option<Rent>,
		) -> Result<AccountMigrationOutcome<T::Version>, ProgramError>
		where
			T: MigratableAccount,
		{
			self.account
				.assert_writable()?
				.assert_owner(self.program_id)?;

			let (stored, plan) = {
				let data = self.account.try_borrow()?;
				if !T::matches_discriminator(&data) {
					return Err(ProgramError::InvalidAccountData);
				}

				match T::inspect_migration_version(&data)? {
					StoredVersion::Current(version) => {
						T::validate_current_migration(&data)?;

						return Ok(AccountMigrationOutcome::AlreadyCurrent { version });
					}
					StoredVersion::Future { .. } => {
						return Err(PinaProgramError::InvalidMigrationVersion.into());
					}
					StoredVersion::Stale { stored, .. } => {
						let plan = T::plan_migration(&data)?;

						(stored, plan)
					}
				}
			};

			let current = T::CURRENT_VERSION;
			if plan.from_version() != stored.into_u32()
				|| plan.to_version() != current.into_u32()
				|| plan.steps() > T::MAX_INLINE_STEPS
				|| plan.target_size() < T::MIGRATION_HEADER_SIZE
				|| plan.working_size() < plan.target_size()
			{
				return Err(PinaProgramError::MigrationUnavailable.into());
			}

			let current_size = self.account.data_len();
			let target_size = plan.target_size();
			let working_size = plan.working_size();
			let allocated_working_size = current_size.max(working_size);
			if working_size
				.checked_sub(current_size)
				.is_some_and(|growth| growth > MAX_PERMITTED_DATA_INCREASE)
			{
				return Err(PinaProgramError::MigrationBudgetExceeded.into());
			}

			let funding = if working_size > current_size {
				let rent = rent.map_or_else(Rent::get, Ok)?;
				let target_minimum = rent.try_minimum_balance(working_size)?;
				target_minimum.saturating_sub(self.account.lamports())
			} else {
				0
			};
			if funding > self.max_lamports {
				return Err(PinaProgramError::MigrationBudgetExceeded.into());
			}

			// Application code can catch a returned `ProgramError` and continue.
			// Preflight the account-data borrow before funding, then abort the whole
			// instruction if any invariant fails after the first successful effect.
			self.account.check_borrow_mut()?;
			let mut mutation_started = false;

			if funding > 0 {
				let payer = self.payer.ok_or(PinaProgramError::MigrationRequired)?;
				validate_funding_payer(payer, self.account, !signers.is_empty())?;

				SystemTransfer {
					from: payer,
					to: self.account,
					lamports: funding,
				}
				.invoke_signed(signers)?;
				mutation_started = true;
			}

			if working_size > current_size {
				match self.account.resize(working_size) {
					Ok(()) => mutation_started = true,
					Err(error) if mutation_started => abort_after_mutation(error),
					Err(error) => return Err(error),
				}
			}

			let steps = plan.steps();
			{
				let mut data = match self.account.try_borrow_mut() {
					Ok(data) => data,
					Err(error) if mutation_started => abort_after_mutation(error),
					Err(error) => return Err(error),
				};
				T::apply_migration(plan.into_payload(), &mut data);
			}

			if target_size < allocated_working_size {
				finish_after_mutation(self.account.resize(target_size));
			}

			{
				let mut data = finish_after_mutation(self.account.try_borrow_mut());
				finish_after_mutation(T::validate_migration_destination(&data));
				finish_after_mutation(T::write_current_migration_version(&mut data));
				finish_after_mutation(T::validate_current_migration(&data));
			}

			Ok(AccountMigrationOutcome::Migrated {
				from: stored,
				to: current,
				steps,
			})
		}
	}
}

#[cfg(feature = "account-resize")]
pub use executor::MigrateAccount;

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
	extern crate std;

	#[cfg(feature = "account-resize")]
	use pinocchio::AccountView;
	#[cfg(feature = "account-resize")]
	use pinocchio::Address;
	#[cfg(feature = "account-resize")]
	use pinocchio::account::NOT_BORROWED;
	#[cfg(feature = "account-resize")]
	use pinocchio::account::RuntimeAccount;
	#[cfg(feature = "account-resize")]
	use pinocchio::sysvars::rent::Rent;

	#[cfg(feature = "account-resize")]
	use super::executor::validate_funding_payer;
	use super::*;
	#[cfg(feature = "account-resize")]
	use crate::MAX_PERMITTED_DATA_INCREASE;

	struct U8Versioned;

	impl HasDiscriminator for U8Versioned {
		type Type = u16;

		const VALUE: Self::Type = 0x1234;
	}

	impl HasMigrationVersion for U8Versioned {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 2;
	}

	struct VersionedInstruction;

	impl HasDiscriminator for VersionedInstruction {
		type Type = u8;

		const VALUE: Self::Type = 7;
	}

	impl HasMigrationVersion for VersionedInstruction {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 1;
	}

	impl MigratableInstruction for VersionedInstruction {
		const CURRENT_SIZE: usize = 4;
		const MAX_INLINE_STEPS: u16 = 1;
		const WORKING_SIZE: usize = 4;

		fn migrate_stale_instruction(data: &[u8], workspace: &mut [u8]) -> ProgramResult {
			if data.len() != 3 || data[0] != Self::VALUE || data[1] != 0 {
				return Err(ProgramError::InvalidInstructionData);
			}
			workspace[..3].copy_from_slice(data);
			workspace[3] = 99;
			Ok(())
		}

		fn validate_current_instruction(data: &[u8]) -> ProgramResult {
			if data.len() != Self::CURRENT_SIZE
				|| data[0] != Self::VALUE
				|| data[1] != Self::CURRENT_VERSION
				|| data[3] != 99
			{
				return Err(ProgramError::InvalidInstructionData);
			}
			Ok(())
		}
	}

	#[test]
	fn version_lives_immediately_after_discriminator() {
		let mut bytes = [0_u8; 5];
		U8Versioned::write_discriminator(&mut bytes);
		U8Versioned::write_current_migration_version(&mut bytes)
			.unwrap_or_else(|error| panic!("write version: {error:?}"));

		assert_eq!(U8Versioned::VERSION_OFFSET, 2);
		assert_eq!(U8Versioned::MIGRATION_HEADER_SIZE, 3);
		assert_eq!(bytes[..3], [0x34, 0x12, 2]);
		assert_eq!(
			U8Versioned::inspect_migration_version(&bytes),
			Ok(StoredVersion::Current(2))
		);
	}

	#[test]
	fn classifies_stale_and_future_versions_without_payload_decode() {
		let stale = [0x34, 0x12, 1];
		let future = [0x34, 0x12, 3];

		assert_eq!(
			U8Versioned::inspect_migration_version(&stale),
			Ok(StoredVersion::Stale {
				stored: 1,
				current: 2,
			})
		);
		assert_eq!(
			U8Versioned::inspect_migration_version(&future),
			Ok(StoredVersion::Future {
				stored: 3,
				current: 2,
			})
		);
		assert_eq!(
			U8Versioned::require_current_migration_version(&stale),
			Err(PinaProgramError::MigrationRequired.into())
		);
		assert_eq!(
			U8Versioned::require_current_migration_version(&future),
			Err(PinaProgramError::InvalidMigrationVersion.into())
		);
	}

	#[test]
	fn every_version_codec_is_little_endian_and_bounds_checked() {
		let mut u8_bytes = [0_u8; 1];
		42_u8
			.write_le(&mut u8_bytes)
			.unwrap_or_else(|error| panic!("write u8: {error:?}"));
		assert_eq!(u8::read_le(&u8_bytes), Ok(42));

		let mut u16_bytes = [0_u8; 2];
		0x1234_u16
			.write_le(&mut u16_bytes)
			.unwrap_or_else(|error| panic!("write u16: {error:?}"));
		assert_eq!(u16_bytes, [0x34, 0x12]);
		assert_eq!(u16::read_le(&u16_bytes), Ok(0x1234));

		let mut u32_bytes = [0_u8; 4];
		0x1234_5678_u32
			.write_le(&mut u32_bytes)
			.unwrap_or_else(|error| panic!("write u32: {error:?}"));
		assert_eq!(u32_bytes, [0x78, 0x56, 0x34, 0x12]);
		assert_eq!(u32::read_le(&u32_bytes), Ok(0x1234_5678));

		assert_eq!(
			u32::read_le(&[1, 2, 3]),
			Err(PinaProgramError::DataTooShort.into())
		);
		assert_eq!(
			u8::try_from_u32(256),
			Err(PinaProgramError::InvalidMigrationVersion.into())
		);
	}

	#[test]
	fn migration_plans_must_be_monotonic_and_nonempty() {
		assert_eq!(
			AccountMigrationPlan::try_new(0, 2, 64, 2, 7_u8),
			Ok(AccountMigrationPlan {
				from_version: 0,
				to_version: 2,
				target_size: 64,
				working_size: 64,
				steps: 2,
				payload: 7,
			})
		);
		assert_eq!(
			AccountMigrationPlan::try_new(2, 2, 64, 1, ()),
			Err(PinaProgramError::InvalidMigrationVersion.into())
		);
		assert_eq!(
			AccountMigrationPlan::try_new(2, 1, 64, 1, ()),
			Err(PinaProgramError::InvalidMigrationVersion.into())
		);
		assert_eq!(
			AccountMigrationPlan::try_new(1, 2, 64, 0, ()),
			Err(PinaProgramError::InvalidMigrationVersion.into())
		);
	}

	#[test]
	fn historical_instruction_normalization_clears_workspace_and_commits_version_last() {
		let mut workspace = [0xaa; 4];
		let normalized =
			normalize_instruction_data::<VersionedInstruction>(&[7, 0, 42], &mut workspace)
				.unwrap_or_else(|error| panic!("normalize instruction: {error:?}"));

		assert!(normalized.was_migrated());
		assert_eq!(normalized.as_bytes(), [7, 1, 42, 99]);
	}

	#[test]
	fn current_instruction_hot_path_does_not_touch_workspace() {
		let current = [7, 1, 42, 99];
		let mut workspace = [0xaa; 4];
		let normalized =
			normalize_instruction_data::<VersionedInstruction>(&current, &mut workspace)
				.unwrap_or_else(|error| panic!("normalize instruction: {error:?}"));

		assert!(!normalized.was_migrated());
		assert_eq!(normalized.as_bytes(), current);
		assert_eq!(workspace, [0xaa; 4]);
	}

	#[test]
	fn instruction_normalization_rejects_forged_versions_lengths_and_workspace() {
		for rejected in [&[7, 2, 42, 99][..], &[7, 0, 42, 13][..], &[8, 0, 42][..]] {
			let mut workspace = [0xaa; 4];
			assert!(
				normalize_instruction_data::<VersionedInstruction>(rejected, &mut workspace)
					.is_err()
			);
		}

		let mut short_workspace = [0xaa; 3];
		assert_eq!(
			normalize_instruction_data::<VersionedInstruction>(&[7, 0, 42], &mut short_workspace,),
			Err(PinaProgramError::MigrationBudgetExceeded.into())
		);
		assert_eq!(short_workspace, [0xaa; 3]);
	}

	#[cfg(feature = "account-resize")]
	#[repr(C)]
	struct TestAccount<const N: usize> {
		header: RuntimeAccount,
		data: [u8; N],
	}

	#[cfg(feature = "account-resize")]
	impl<const N: usize> TestAccount<N> {
		fn new(address: Address, owner: Address, lamports: u64, data: &[u8]) -> Self {
			assert!(data.len() <= N);
			let mut stored = Self {
				header: RuntimeAccount {
					borrow_state: NOT_BORROWED,
					is_signer: 1,
					is_writable: 1,
					executable: 0,
					padding: (data.len() as u32).to_le_bytes(),
					address,
					owner,
					lamports,
					data_len: data.len() as u64,
				},
				data: [0; N],
			};
			stored.data[..data.len()].copy_from_slice(data);

			stored
		}

		fn view(&mut self) -> AccountView {
			unsafe { AccountView::new_unchecked(core::ptr::addr_of_mut!(self.header)) }
		}
	}

	#[cfg(feature = "account-resize")]
	struct GrowingAccount;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for GrowingAccount {
		type Type = u8;

		const VALUE: Self::Type = 7;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for GrowingAccount {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 1;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for GrowingAccount {
		type Plan = u8;

		const MAX_INLINE_STEPS: u16 = 1;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			if data.len() != 3 || data[0] != Self::VALUE || data[1] != 0 {
				return Err(ProgramError::InvalidAccountData);
			}

			AccountMigrationPlan::try_new(0, 1, 4, 1, data[2])
		}

		fn apply_migration(plan: Self::Plan, destination: &mut [u8]) {
			destination[2] = plan;
			destination[3] = 99;
		}

		fn validate_migration_destination(data: &[u8]) -> ProgramResult {
			if data.len() != 4 || data[0] != Self::VALUE || data[3] != 99 {
				return Err(ProgramError::InvalidAccountData);
			}

			Ok(())
		}
	}

	#[cfg(feature = "account-resize")]
	struct ShrinkingAccount;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for ShrinkingAccount {
		type Type = u8;

		const VALUE: Self::Type = 8;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for ShrinkingAccount {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 1;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for ShrinkingAccount {
		type Plan = u8;

		const MAX_INLINE_STEPS: u16 = 1;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			if data.len() != 4 || data[0] != Self::VALUE || data[1] != 0 {
				return Err(ProgramError::InvalidAccountData);
			}

			AccountMigrationPlan::try_new(0, 1, 3, 1, data[3])
		}

		fn apply_migration(plan: Self::Plan, destination: &mut [u8]) {
			destination[2] = plan;
		}

		fn validate_migration_destination(data: &[u8]) -> ProgramResult {
			if data.len() != 3 || data[0] != Self::VALUE {
				return Err(ProgramError::InvalidAccountData);
			}

			Ok(())
		}
	}

	#[cfg(feature = "account-resize")]
	struct AdversarialPlanAccount;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for AdversarialPlanAccount {
		type Type = u8;

		const VALUE: Self::Type = 9;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for AdversarialPlanAccount {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 1;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for AdversarialPlanAccount {
		type Plan = ();

		const MAX_INLINE_STEPS: u16 = 1;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			match data {
				[Self::VALUE, 0, 0] => AccountMigrationPlan::try_new(0, 1, 3, 2, ()),
				[Self::VALUE, 0, 1] => AccountMigrationPlan::try_new(0, 1, 1, 1, ()),
				[Self::VALUE, 0, 2] => {
					AccountMigrationPlan::try_with_working_size(
						0,
						1,
						3,
						MAX_PERMITTED_DATA_INCREASE + 4,
						1,
						(),
					)
				}
				_ => Err(ProgramError::InvalidAccountData),
			}
		}

		fn apply_migration(_: Self::Plan, _: &mut [u8]) {}

		fn validate_migration_destination(_: &[u8]) -> ProgramResult {
			Ok(())
		}
	}

	#[cfg(feature = "account-resize")]
	struct InvalidDestinationAccount;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for InvalidDestinationAccount {
		type Type = u8;

		const VALUE: Self::Type = 10;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for InvalidDestinationAccount {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 1;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for InvalidDestinationAccount {
		type Plan = ();

		const MAX_INLINE_STEPS: u16 = 1;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			if data != [Self::VALUE, 0, 42] {
				return Err(ProgramError::InvalidAccountData);
			}

			AccountMigrationPlan::try_new(0, 1, 3, 1, ())
		}

		fn apply_migration(_: Self::Plan, destination: &mut [u8]) {
			destination[2] = 99;
		}

		fn validate_migration_destination(data: &[u8]) -> ProgramResult {
			if data != [Self::VALUE, 0, 42] && data != [Self::VALUE, 1, 42] {
				return Err(ProgramError::InvalidAccountData);
			}

			Ok(())
		}
	}

	#[cfg(feature = "account-resize")]
	fn test_rent() -> Rent {
		Rent::from_bytes(&1_u64.to_le_bytes())
			.unwrap_or_else(|error| panic!("create test rent: {error:?}"))
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn funding_payer_accepts_transaction_or_pda_authority_without_allowing_aliases() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored_account =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 0, &[7, 0, 42]);
		let mut stored_payer =
			TestAccount::<8>::new(Address::new_from_array([2; 32]), owner, 100, &[]);
		stored_payer.header.is_signer = 0;
		let account = stored_account.view();
		let payer = stored_payer.view();

		assert_eq!(
			validate_funding_payer(&payer, &account, false),
			Err(ProgramError::MissingRequiredSignature)
		);
		assert_eq!(validate_funding_payer(&payer, &account, true), Ok(()));

		stored_payer.header.is_writable = 0;
		let payer = stored_payer.view();
		assert_eq!(
			validate_funding_payer(&payer, &account, true),
			Err(ProgramError::InvalidAccountData)
		);

		let mut duplicate =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 100, &[]);
		let duplicate = duplicate.view();
		assert_eq!(
			validate_funding_payer(&duplicate, &account, true),
			Err(PinaProgramError::DuplicateMutableAccount.into())
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_grows_rewrites_validates_and_commits_version() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored =
			TestAccount::<32>::new(Address::new_from_array([1; 32]), owner, 1_000, &[7, 0, 42]);
		let mut account = stored.view();
		let outcome = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: 0,
		}
		.invoke_with_rent::<GrowingAccount>(test_rent())
		.unwrap_or_else(|error| panic!("migrate account: {error:?}"));

		assert_eq!(
			outcome,
			AccountMigrationOutcome::Migrated {
				from: 0,
				to: 1,
				steps: 1,
			}
		);
		assert_eq!(account.data_len(), 4);
		assert_eq!(
			account
				.try_borrow()
				.unwrap_or_else(|error| panic!("borrow migrated data: {error:?}"))
				.as_ref(),
			[7, 1, 42, 99]
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_shrinks_without_refunding_application_lamports() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored = TestAccount::<32>::new(
			Address::new_from_array([1; 32]),
			owner,
			10_000,
			&[8, 0, 11, 42],
		);
		let mut account = stored.view();
		let before = account.lamports();
		let outcome = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: 0,
		}
		.invoke_with_rent::<ShrinkingAccount>(test_rent())
		.unwrap_or_else(|error| panic!("migrate account: {error:?}"));

		assert_eq!(
			outcome,
			AccountMigrationOutcome::Migrated {
				from: 0,
				to: 1,
				steps: 1,
			}
		);
		assert_eq!(account.data_len(), 3);
		assert_eq!(account.lamports(), before);
		assert_eq!(
			account
				.try_borrow()
				.unwrap_or_else(|error| panic!("borrow migrated data: {error:?}"))
				.as_ref(),
			[8, 1, 42]
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_rejects_future_versions_without_mutation() {
		let owner = Address::new_from_array([9; 32]);
		let original = [7, 2, 42, 99];
		let mut stored =
			TestAccount::<32>::new(Address::new_from_array([1; 32]), owner, 1_000, &original);
		let mut account = stored.view();
		let result = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: 0,
		}
		.invoke_with_rent::<GrowingAccount>(test_rent());

		assert_eq!(
			result,
			Err(PinaProgramError::InvalidMigrationVersion.into())
		);
		assert_eq!(
			account
				.try_borrow()
				.unwrap_or_else(|error| panic!("borrow rejected data: {error:?}"))
				.as_ref(),
			original
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_rejects_untrusted_account_boundaries_before_mutation() {
		let program_id = Address::new_from_array([9; 32]);

		let mut wrong_owner = TestAccount::<8>::new(
			Address::new_from_array([1; 32]),
			Address::new_from_array([8; 32]),
			1_000,
			&[7, 0, 42],
		);
		let mut account = wrong_owner.view();
		assert_eq!(
			MigrateAccount {
				account: &mut account,
				payer: None,
				program_id: &program_id,
				max_lamports: 0,
			}
			.invoke_with_rent::<GrowingAccount>(test_rent()),
			Err(ProgramError::InvalidAccountOwner)
		);
		assert_eq!(&*account.try_borrow().unwrap(), &[7, 0, 42]);

		let mut readonly = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			1_000,
			&[7, 0, 42],
		);
		readonly.header.is_writable = 0;
		let mut account = readonly.view();
		assert_eq!(
			MigrateAccount {
				account: &mut account,
				payer: None,
				program_id: &program_id,
				max_lamports: 0,
			}
			.invoke_with_rent::<GrowingAccount>(test_rent()),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(&*account.try_borrow().unwrap(), &[7, 0, 42]);

		for rejected in [&[8, 0, 42][..], &[7, 0, 42, 13][..], &[7, 1, 42, 13][..]] {
			let mut stored = TestAccount::<8>::new(
				Address::new_from_array([3; 32]),
				program_id,
				1_000,
				rejected,
			);
			let mut account = stored.view();
			assert!(
				MigrateAccount {
					account: &mut account,
					payer: None,
					program_id: &program_id,
					max_lamports: 0,
				}
				.invoke_with_rent::<GrowingAccount>(test_rent())
				.is_err()
			);
			assert_eq!(&*account.try_borrow().unwrap(), rejected);
		}
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_enforces_plan_step_header_and_growth_budgets() {
		let owner = Address::new_from_array([9; 32]);
		for source in [[9, 0, 0], [9, 0, 1], [9, 0, 2]] {
			let mut stored =
				TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 1_000, &source);
			let mut account = stored.view();
			let result = MigrateAccount {
				account: &mut account,
				payer: None,
				program_id: &owner,
				max_lamports: u64::MAX,
			}
			.invoke_with_rent::<AdversarialPlanAccount>(test_rent());

			assert!(matches!(
				result,
				Err(error)
					if error == PinaProgramError::MigrationUnavailable.into()
						|| error == PinaProgramError::MigrationBudgetExceeded.into()
			));
			assert_eq!(&*account.try_borrow().unwrap(), &source);
		}
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_aborts_instead_of_returning_an_error_after_mutation() {
		const RETURNED_EXIT_CODE: i32 = 86;
		let output = std::process::Command::new(
			std::env::current_exe().unwrap_or_else(|error| panic!("locate test binary: {error}")),
		)
		.arg("--exact")
		.arg("migration::tests::post_mutation_invariant_failure_child")
		.arg("--nocapture")
		.env("PINA_TEST_POST_MUTATION_ABORT_CHILD", "1")
		.output()
		.unwrap_or_else(|error| panic!("run abort probe: {error}"));

		assert!(!output.status.success());
		assert_ne!(
			output.status.code(),
			Some(RETURNED_EXIT_CODE),
			"migration returned after mutation: {}",
			std::string::String::from_utf8_lossy(&output.stderr),
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn post_mutation_invariant_failure_child() {
		const RETURNED_EXIT_CODE: i32 = 86;
		if std::env::var_os("PINA_TEST_POST_MUTATION_ABORT_CHILD").is_none() {
			return;
		}

		let owner = Address::new_from_array([9; 32]);
		let mut stored =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 1_000, &[10, 0, 42]);
		let mut account = stored.view();
		let _result = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: 0,
		}
		.invoke_with_rent::<InvalidDestinationAccount>(test_rent());

		std::process::exit(RETURNED_EXIT_CODE);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_requires_explicitly_capped_funding() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 0, &[7, 0, 42]);
		let mut account = stored.view();
		assert_eq!(
			MigrateAccount {
				account: &mut account,
				payer: None,
				program_id: &owner,
				max_lamports: 0,
			}
			.invoke_with_rent::<GrowingAccount>(test_rent()),
			Err(PinaProgramError::MigrationBudgetExceeded.into())
		);

		assert_eq!(account.lamports(), 0);
		assert_eq!(account.data_len(), 3);
		assert_eq!(&*account.try_borrow().unwrap(), &[7, 0, 42]);

		assert_eq!(
			MigrateAccount {
				account: &mut account,
				payer: None,
				program_id: &owner,
				max_lamports: u64::MAX,
			}
			.invoke_with_rent::<GrowingAccount>(test_rent()),
			Err(PinaProgramError::MigrationRequired.into())
		);
		assert_eq!(&*account.try_borrow().unwrap(), &[7, 0, 42]);
	}
}
