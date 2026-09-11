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

/// Largest caller-owned migration workspace generated on the SBF stack.
///
/// `with_current_instruction_data` and `with_current_event_data` place the
/// historical-normalization workspace in their own stack frame, and the
/// caller's handler runs inside that frame. Generated code refuses to compile
/// beyond this bound so a migration-aware payload can never silently exhaust
/// the 4 KiB SBF stack budget.
pub const MAX_MIGRATION_WORKSPACE: usize = 1024;

/// All-ones one-byte discriminator reserved for the framework `Migrate`
/// instruction.
///
/// `#[discriminator]` rejects user variants that claim the reserved value, so
/// a program can route the reserved instruction before parsing its own
/// instruction enum:
///
/// ```rust,ignore
/// if data == [pina::migration::MIGRATE_DISCRIMINATOR_U8] {
///     return process_migrate(program_id, accounts);
/// }
/// ```
///
/// Programs with wider instruction discriminators compare against the matching
/// constant, and [`is_migrate_instruction`] covers the one-byte case.
pub const MIGRATE_DISCRIMINATOR_U8: u8 = u8::MAX;

/// Two-byte reserved discriminator; see [`MIGRATE_DISCRIMINATOR_U8`].
pub const MIGRATE_DISCRIMINATOR_U16: u16 = u16::MAX;

/// Four-byte reserved discriminator; see [`MIGRATE_DISCRIMINATOR_U8`].
pub const MIGRATE_DISCRIMINATOR_U32: u32 = u32::MAX;

/// Eight-byte reserved discriminator; see [`MIGRATE_DISCRIMINATOR_U8`].
pub const MIGRATE_DISCRIMINATOR_U64: u64 = u64::MAX;

/// Whether `data` carries exactly the reserved `Migrate` discriminator for a
/// one-byte instruction space.
#[must_use]
pub fn is_migrate_instruction(data: &[u8]) -> bool {
	data == [MIGRATE_DISCRIMINATOR_U8]
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
				u32::from(self)
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
		Self::write_migration_version(Self::CURRENT_VERSION, data)
	}

	/// Write one generated historical or current version into an initialized
	/// envelope.
	///
	/// Account migrations use this only after the destination for that exact
	/// adjacent version has validated. Application code should write only the
	/// current version through [`Self::write_current_migration_version`].
	#[inline(always)]
	fn write_migration_version(version: Self::Version, data: &mut [u8]) -> ProgramResult {
		let bytes = data
			.get_mut(Self::VERSION_OFFSET..Self::MIGRATION_HEADER_SIZE)
			.ok_or(PinaProgramError::DataTooShort)?;

		version.write_le(bytes)
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
	#[allow(
		clippy::match_same_arms,
		reason = "the variants carry independent lifetimes"
	)]
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

/// A current event representation paired with the version found in the log.
///
/// A field initialized by migration can be distinguished from a field that was
/// actually emitted by checking [`Self::source_version`]. Event bytes are
/// immutable, so normalization always writes historical projections into the
/// caller-owned workspace and never changes the original log record.
#[derive(Debug, PartialEq, Eq)]
pub struct CurrentEventData<'source, 'workspace, V> {
	data: CurrentInstructionData<'source, 'workspace>,
	source_version: V,
}

impl<V: Copy> CurrentEventData<'_, '_, V> {
	/// Return the exact current event representation.
	#[must_use]
	pub const fn as_bytes(&self) -> &[u8] {
		self.data.as_bytes()
	}

	/// Return the version carried by the immutable historical event.
	#[must_use]
	pub const fn source_version(&self) -> V {
		self.source_version
	}

	/// Return whether a historical projection ran.
	#[must_use]
	pub const fn was_migrated(&self) -> bool {
		self.data.was_migrated()
	}
}

/// Generated conversion contract for one migratable event payload.
///
/// Event transitions are pure historical projections. They never rewrite a
/// transaction log or account and retain the source version as provenance.
pub trait MigratableEvent: HasMigrationVersion {
	/// Exact current event length.
	const CURRENT_SIZE: usize;

	/// Largest temporary byte region needed by any supported transition path.
	const WORKING_SIZE: usize;

	/// Maximum adjacent transitions accepted by one projection.
	const MAX_INLINE_STEPS: u16;

	/// Rewrite one exact historical event into caller-owned workspace.
	fn migrate_stale_event(data: &[u8], workspace: &mut [u8]) -> ProgramResult;

	/// Validate the current event representation.
	fn validate_current_event(data: &[u8]) -> ProgramResult;
}

/// Project current or historical event bytes into the current representation.
///
/// Unknown versions, future versions, non-exact historical lengths, and
/// insufficient workspaces fail closed. The current hot path borrows the source
/// directly and does not touch the workspace.
pub fn normalize_event_data<'source, 'workspace, T>(
	data: &'source [u8],
	workspace: &'workspace mut [u8],
) -> Result<CurrentEventData<'source, 'workspace, T::Version>, ProgramError>
where
	T: MigratableEvent,
{
	if !T::matches_discriminator(data) {
		return Err(ProgramError::InvalidInstructionData);
	}

	let source_version = T::read_migration_version(data)?;
	let normalized = match T::inspect_migration_version(data)? {
		StoredVersion::Current(_) => {
			T::validate_current_event(data)?;
			CurrentInstructionData::Current(data)
		}
		StoredVersion::Future { .. } => {
			return Err(PinaProgramError::InvalidMigrationVersion.into());
		}
		StoredVersion::Stale { .. } => {
			if T::WORKING_SIZE < T::CURRENT_SIZE || workspace.len() < T::WORKING_SIZE {
				return Err(PinaProgramError::MigrationBudgetExceeded.into());
			}
			workspace[..T::WORKING_SIZE].fill(0);
			T::migrate_stale_event(data, &mut workspace[..T::WORKING_SIZE])?;
			T::write_current_migration_version(&mut workspace[..T::CURRENT_SIZE])?;
			T::validate_current_event(&workspace[..T::CURRENT_SIZE])?;
			CurrentInstructionData::Migrated(&workspace[..T::CURRENT_SIZE])
		}
	};

	Ok(CurrentEventData {
		data: normalized,
		source_version,
	})
}

/// Pure plan for one adjacent account migration.
///
/// Generated planning code validates the exact historical source and owns
/// `payload`. The payload cannot borrow account data because this type has no
/// source lifetime. The executor can therefore release every account borrow
/// before rent adjustment or resize. Multi-version histories execute one plan
/// at a time inside one atomic instruction; any failure after the first
/// mutation aborts the instruction and rolls every preceding step back.
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

	/// Adjacent destination version.
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

	/// Validate the selected historical representation and build a detached plan
	/// for its immediate successor.
	fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError>;

	/// Rewrite `destination` according to a successfully preflighted plan.
	fn apply_migration(plan: Self::Plan, destination: &mut [u8]);

	/// Validate an exact generated destination representation without requiring
	/// its version bytes to have been committed yet.
	fn validate_migration_destination(version: u32, data: &[u8]) -> ProgramResult;

	/// Validate the complete current representation, including the committed
	/// version envelope.
	#[inline(always)]
	fn validate_current_migration(data: &[u8]) -> ProgramResult {
		Self::require_current_migration_version(data)?;
		Self::validate_migration_destination(Self::CURRENT_VERSION.into_u32(), data)
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
	#[allow(
		clippy::needless_pass_by_value,
		reason = "the divergent boundary owns the error passed from either return branch"
	)]
	fn abort_after_mutation(error: ProgramError) -> ! {
		panic!("account migration invariant failed after mutation: {error:?}")
	}

	fn finish_after_mutation<T>(result: Result<T, ProgramError>) -> T {
		match result {
			Ok(value) => value,
			Err(error) => abort_after_mutation(error),
		}
	}

	/// Shared state for one in-flight account migration.
	struct MigrationCtx<'account, 'payer, 'signers> {
		account: &'account mut AccountView,
		payer: Option<&'payer AccountView>,
		signers: &'signers [Signer<'signers, 'signers>],
		rent: Option<Rent>,
		max_lamports: u64,
		original_size: usize,
		stored: u32,
		transferred: u64,
		from: u32,
		current: u32,
		completed_steps: u16,
	}

	/// Effects planned for one adjacent transition, validated but not yet
	/// applied.
	struct PlannedStep<P> {
		payload: Option<P>,
		to: u32,
		target_size: usize,
		allocated_working_size: usize,
		working_size: usize,
		funding: u64,
	}

	/// Plan the next adjacent transition without producing any effect.
	///
	/// Every check here is effect-free, so [`Preflight`] may surface failures
	/// as ordinary errors while [`Mutating`] must abort on them instead.
	fn plan_next_step<T>(
		ctx: &mut MigrationCtx<'_, '_, '_>,
	) -> Result<PlannedStep<T::Plan>, ProgramError>
	where
		T: MigratableAccount,
	{
		let plan = {
			let data = ctx.account.try_borrow()?;
			T::plan_migration(&data)?
		};

		// `from < current <= u32::MAX`, so this addition cannot overflow.
		let to = ctx.from + 1;
		if plan.from_version() != ctx.from
			|| plan.to_version() != to
			|| plan.steps() != 1
			|| plan.target_size() < T::MIGRATION_HEADER_SIZE
			|| plan.working_size() < plan.target_size()
		{
			return Err(PinaProgramError::MigrationUnavailable.into());
		}

		let current_size = ctx.account.data_len();
		let target_size = plan.target_size();
		let working_size = plan.working_size();
		let allocated_working_size = current_size.max(working_size);
		if allocated_working_size
			.checked_sub(ctx.original_size)
			.is_some_and(|growth| growth > MAX_PERMITTED_DATA_INCREASE)
		{
			return Err(PinaProgramError::MigrationBudgetExceeded.into());
		}

		let funding = if working_size > current_size {
			let rent = match ctx.rent {
				Some(rent) => rent,
				None => Rent::get()?,
			};
			let target_minimum = rent.try_minimum_balance(working_size)?;
			target_minimum.saturating_sub(ctx.account.lamports())
		} else {
			0
		};
		let next_transferred = ctx.transferred.checked_add(funding);
		if next_transferred.is_none_or(|total| total > ctx.max_lamports) {
			return Err(PinaProgramError::MigrationBudgetExceeded.into());
		}
		// Account for the planned funding up front so the post-mutation tail
		// carries no budget bookkeeping after the transfer call: on the host
		// the transfer CPI can never succeed, and any statement after it would
		// be unreachable in coverage. [`Preflight`] rolls this back when the
		// step fails before its first effect.
		ctx.transferred = next_transferred.unwrap_or(u64::MAX);

		if funding > 0 {
			let payer = ctx
				.payer
				.ok_or_else(|| ProgramError::from(PinaProgramError::MigrationRequired))?;
			validate_funding_payer(payer, ctx.account, !ctx.signers.is_empty())?;
		}

		Ok(PlannedStep {
			payload: Some(plan.into_payload()),
			to,
			target_size,
			allocated_working_size,
			working_size,
			funding,
		})
	}

	impl MigrationCtx<'_, '_, '_> {
		/// Execute one planned rent transfer. Only reachable after the first
		/// effect, so a failure aborts the instruction.
		fn execute_funding_transfer(&mut self, funding: u64) -> ProgramResult {
			let payer = self.payer.expect("planned transfer validated the payer");
			SystemTransfer {
				from: payer,
				to: self.account,
				lamports: funding,
			}
			.invoke_signed(self.signers)
		}

		/// Apply one planned transition. Only reachable after the first
		/// effect, so every failure aborts the instruction.
		fn complete_step_after_effect<T>(&mut self, step: &mut PlannedStep<T::Plan>)
		where
			T: MigratableAccount,
		{
			if step.funding > 0 {
				finish_after_mutation(self.execute_funding_transfer(step.funding));
			}
			if step.working_size > self.account.data_len() {
				finish_after_mutation(self.account.resize(step.working_size));
			}
			if let Some(payload) = step.payload.take() {
				let mut data = finish_after_mutation(self.account.try_borrow_mut());
				T::apply_migration(payload, &mut data);
			}
			if step.target_size < step.allocated_working_size {
				finish_after_mutation(self.account.resize(step.target_size));
			}
			{
				let mut data = finish_after_mutation(self.account.try_borrow_mut());
				finish_after_mutation(T::validate_migration_destination(step.to, &data));
				let version = finish_after_mutation(T::Version::try_from_u32(step.to));
				finish_after_mutation(T::write_migration_version(version, &mut data));
				finish_after_mutation(T::validate_migration_destination(step.to, &data));
			}

			self.from = step.to;
			// The preflight caps total steps to `MAX_INLINE_STEPS: u16`.
			self.completed_steps += 1;
		}
	}

	/// Ladder state before any effect has occurred.
	///
	/// Fallible operations surface as ordinary errors: the caller may catch
	/// them and continue without migrating.
	struct Preflight<'account, 'payer, 'signers> {
		ctx: MigrationCtx<'account, 'payer, 'signers>,
	}

	impl<'account, 'payer, 'signers> Preflight<'account, 'payer, 'signers> {
		/// Attempt the first adjacent transition.
		///
		/// Returns `Err` only when the step failed before producing any
		/// effect. The first effect itself is the boundary: once the funding
		/// transfer, resize, or rewrite has succeeded, every later failure
		/// aborts the instruction through [`Mutating`], so a partial
		/// migration can never be caught and committed.
		fn first_step<T>(mut self) -> Result<Mutating<'account, 'payer, 'signers>, ProgramError>
		where
			T: MigratableAccount,
		{
			let mut step = plan_next_step::<T>(&mut self.ctx)?;
			if step.funding > 0 {
				// The transfer is the first effect; its failure leaves the
				// account untouched and stays catchable, so the planned
				// funding is rolled back. Everything after it runs through
				// the aborting tail.
				if let Err(error) = self.ctx.execute_funding_transfer(step.funding) {
					self.ctx.transferred = self.ctx.transferred.saturating_sub(step.funding);
					return Err(error);
				}
				step.funding = 0;
			} else if step.working_size > self.ctx.account.data_len() {
				// The resize is the first effect; its failure also leaves the
				// account untouched. The tail's own size check then skips it.
				self.ctx.account.resize(step.working_size)?;
			}

			let mut mutating = Mutating { ctx: self.ctx };
			mutating.ctx.complete_step_after_effect::<T>(&mut step);
			Ok(mutating)
		}
	}

	/// Ladder state after at least one effect has occurred.
	///
	/// No method returns `Result`: any failure from here on aborts the whole
	/// instruction, which makes a catchable partial migration unrepresentable.
	struct Mutating<'account, 'payer, 'signers> {
		ctx: MigrationCtx<'account, 'payer, 'signers>,
	}

	impl Mutating<'_, '_, '_> {
		/// Run one adjacent transition to completion, aborting on failure.
		fn step<T>(&mut self)
		where
			T: MigratableAccount,
		{
			let mut planned = finish_after_mutation(plan_next_step::<T>(&mut self.ctx));
			self.ctx.complete_step_after_effect::<T>(&mut planned);
		}

		/// Validate the final representation and report the outcome.
		fn finish<T>(self) -> AccountMigrationOutcome<T::Version>
		where
			T: MigratableAccount,
		{
			let data = finish_after_mutation(self.ctx.account.try_borrow());
			finish_after_mutation(T::validate_current_migration(&data));

			AccountMigrationOutcome::Migrated {
				from: finish_after_mutation(T::Version::try_from_u32(self.ctx.stored)),
				to: T::CURRENT_VERSION,
				steps: self.ctx.completed_steps,
			}
		}
	}

	#[allow(
		clippy::trivially_copy_pass_by_ref,
		reason = "validation consistently receives borrowed account handles"
	)]
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

			let stored = {
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
					StoredVersion::Stale { stored, .. } => stored,
				}
			};

			let current = T::CURRENT_VERSION;
			let total_steps = current.into_u32().saturating_sub(stored.into_u32());
			if total_steps > u32::from(T::MAX_INLINE_STEPS) {
				return Err(PinaProgramError::MigrationUnavailable.into());
			}

			// Preflight the mutable borrow before funding. `Preflight` keeps
			// every failure before the first effect catchable; once the first
			// effect succeeds, `Mutating` aborts on any failure so a partial
			// migration can never be caught and committed. This lets
			// variable-length migrations derive each next allocation from the
			// representation produced by the preceding step.
			self.account.check_borrow_mut()?;
			let original_size = self.account.data_len();
			let ladder = Preflight {
				ctx: MigrationCtx {
					account: self.account,
					payer: self.payer,
					signers,
					rent,
					max_lamports: self.max_lamports,
					original_size,
					stored: stored.into_u32(),
					transferred: 0,
					from: stored.into_u32(),
					current: current.into_u32(),
					completed_steps: 0,
				},
			};

			let mut mutating = ladder.first_step::<T>()?;
			while mutating.ctx.from < mutating.ctx.current {
				mutating.step::<T>();
			}

			Ok(mutating.finish::<T>())
		}
	}

	/// Framework entry point for the reserved `Migrate` instruction.
	///
	/// Account layout: slot 0 is the funding payer — a writable account, or the
	/// executing program's address when this invocation needs no funding —
	/// slot 1 is the system program the rent transfers invoke, and every later
	/// slot is a program-owned migratable account. Each migratable slot is
	/// used at most once. A program wires the reserved instruction by running
	/// the declared accounts in order:
	///
	/// ```rust,ignore
	/// let mut migrate = MigrateContext::new(program_id, accounts, cap)?;
	/// migrate.run_optional::<State>(2)?;
	/// migrate.run_optional::<ManualState>(3)?;
	/// Ok(())
	/// ```
	///
	/// Slots that hold the program address (the placeholder generated clients
	/// write for an omitted optional account) and slots past the end of the
	/// slice are skipped by [`Self::run_optional`], so a client may send only
	/// the accounts it needs to migrate.
	#[must_use]
	pub struct MigrateContext<'account> {
		program_id: &'account Address,
		accounts: &'account mut [AccountView],
		max_lamports: u64,
		migrated: u64,
		rent: Option<Rent>,
	}

	impl<'account> MigrateContext<'account> {
		/// Validate the reserved instruction's account layout.
		///
		/// # Errors
		///
		/// Returns `NotEnoughAccountKeys` when the payer or system program
		/// slot is missing, `InvalidAccountData` when the system program slot
		/// is not the system program, `InvalidAccountOwner` when a migratable
		/// slot is not owned by the program, and `DuplicateMutableAccount`
		/// when one account fills several slots.
		pub fn new(
			program_id: &'account Address,
			accounts: &'account mut [AccountView],
			max_lamports: u64,
		) -> Result<Self, ProgramError> {
			let Some((payer, rest)) = accounts.split_first_mut() else {
				return Err(ProgramError::NotEnoughAccountKeys);
			};
			// Slot 1 is the system program: the rent transfers invoke it, so
			// the runtime requires it in the instruction's account list.
			let Some((system_program, migratable)) = rest.split_first_mut() else {
				return Err(ProgramError::NotEnoughAccountKeys);
			};
			system_program.assert_address(&crate::system::ID)?;
			// The program-address filler marks an absent payer. A present payer
			// must be writable because it is the only account that can fund a
			// rent deficit.
			if payer.address() != program_id {
				payer.assert_writable()?;
			}
			for (position, account) in migratable.iter().enumerate() {
				if account.address() == program_id {
					continue;
				}
				account.assert_owner(program_id)?;
				// The same account may appear once: a second slot would let
				// one invocation mutate shared bytes twice.
				if migratable
					.iter()
					.skip(position + 1)
					.any(|other| other.address() == account.address())
				{
					return Err(PinaProgramError::DuplicateMutableAccount.into());
				}
			}

			Ok(Self {
				program_id,
				accounts,
				max_lamports,
				migrated: 0,
				rent: None,
			})
		}

		/// Same layout validation with an injected rent sysvar for tests.
		#[cfg(test)]
		pub(super) fn with_rent(
			program_id: &'account Address,
			accounts: &'account mut [AccountView],
			max_lamports: u64,
			rent: Rent,
		) -> Result<Self, ProgramError> {
			let mut context = Self::new(program_id, accounts, max_lamports)?;
			context.rent = Some(rent);

			Ok(context)
		}

		/// Migrate one declared account slot.
		///
		/// # Errors
		///
		/// Fails closed when the slot is the payer, past the end of the slice,
		/// already migrated, not owned by the program, or does not carry the
		/// requested contract's discriminator. Planning failures are catchable
		/// before any effect; invariant failures after the first effect abort
		/// the instruction.
		pub fn run<T>(
			&mut self,
			index: usize,
		) -> Result<AccountMigrationOutcome<T::Version>, ProgramError>
		where
			T: MigratableAccount,
		{
			let Some(slot) = index.checked_sub(2) else {
				return Err(ProgramError::NotEnoughAccountKeys);
			};
			if slot >= u64::BITS as usize - 1 {
				return Err(PinaProgramError::MigrationUnavailable.into());
			}
			let bit = 1_u64 << slot;
			if self.migrated & bit != 0 {
				return Err(PinaProgramError::DuplicateMutableAccount.into());
			}

			let (payer_slot, rest) = self
				.accounts
				.split_first_mut()
				.ok_or(ProgramError::NotEnoughAccountKeys)?;
			let (_, migratable) = rest
				.split_first_mut()
				.ok_or(ProgramError::NotEnoughAccountKeys)?;
			let account = migratable
				.get_mut(slot)
				.ok_or(ProgramError::NotEnoughAccountKeys)?;
			account.assert_owner(self.program_id)?;
			{
				let data = account.try_borrow()?;
				if !T::matches_discriminator(&data) {
					return Err(ProgramError::InvalidAccountData);
				}
			}

			let payer = if payer_slot.address() == self.program_id {
				None
			} else {
				Some(&*payer_slot)
			};
			let mut executor = MigrateAccount {
				account,
				payer,
				program_id: self.program_id,
				max_lamports: self.max_lamports,
			};
			let outcome = executor.invoke_signed_inner::<T>(&[], self.rent)?;
			self.migrated |= bit;

			Ok(outcome)
		}

		/// Migrate one declared account slot, treating an omitted slot as
		/// nothing to do.
		///
		/// A slot holding the program address, or an index past the end of the
		/// slice, is reported as absent instead of failing.
		///
		/// # Errors
		///
		/// Inherits [`Self::run`]'s failures for a present slot.
		pub fn run_optional<T>(
			&mut self,
			index: usize,
		) -> Result<Option<AccountMigrationOutcome<T::Version>>, ProgramError>
		where
			T: MigratableAccount,
		{
			if self.slot_is_absent(index) {
				return Ok(None);
			}
			self.run::<T>(index).map(Some)
		}

		/// Whether this context migrated at least one account.
		#[must_use]
		pub const fn migrated_any(&self) -> bool {
			self.migrated != 0
		}

		fn slot_is_absent(&self, index: usize) -> bool {
			self.accounts
				.get(index)
				.is_none_or(|account| account.address() == self.program_id)
		}
	}
}

#[cfg(feature = "account-resize")]
pub use executor::MigrateAccount;
#[cfg(feature = "account-resize")]
pub use executor::MigrateContext;

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
			if data.len() != <Self as MigratableInstruction>::CURRENT_SIZE
				|| data[0] != Self::VALUE
				|| data[1] != Self::CURRENT_VERSION
				|| data[3] != 99
			{
				return Err(ProgramError::InvalidInstructionData);
			}
			Ok(())
		}
	}

	impl MigratableEvent for VersionedInstruction {
		const CURRENT_SIZE: usize = <VersionedInstruction as MigratableInstruction>::CURRENT_SIZE;
		const MAX_INLINE_STEPS: u16 =
			<VersionedInstruction as MigratableInstruction>::MAX_INLINE_STEPS;
		const WORKING_SIZE: usize = <VersionedInstruction as MigratableInstruction>::WORKING_SIZE;

		fn migrate_stale_event(data: &[u8], workspace: &mut [u8]) -> ProgramResult {
			<Self as MigratableInstruction>::migrate_stale_instruction(data, workspace)
		}

		fn validate_current_event(data: &[u8]) -> ProgramResult {
			<Self as MigratableInstruction>::validate_current_instruction(data)
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

	#[test]
	fn event_projection_preserves_source_version_as_provenance() {
		let mut workspace = [0xaa; 4];
		let projected = normalize_event_data::<VersionedInstruction>(&[7, 0, 42], &mut workspace)
			.unwrap_or_else(|error| panic!("normalize event: {error:?}"));

		assert!(projected.was_migrated());
		assert_eq!(projected.source_version(), 0);
		assert_eq!(projected.as_bytes(), [7, 1, 42, 99]);
	}

	#[test]
	fn current_event_projection_is_zero_copy_and_keeps_provenance() {
		let current = [7, 1, 42, 99];
		let mut workspace = [0xaa; 4];
		let projected = normalize_event_data::<VersionedInstruction>(&current, &mut workspace)
			.unwrap_or_else(|error| panic!("normalize event: {error:?}"));

		assert!(!projected.was_migrated());
		assert_eq!(projected.source_version(), 1);
		assert_eq!(projected.as_bytes(), current);
		assert_eq!(workspace, [0xaa; 4]);
	}

	#[test]
	fn event_projection_rejects_wrong_future_and_underfunded_inputs() {
		for rejected in [&[8, 0, 42][..], &[7, 2, 42, 99][..]] {
			let mut workspace = [0xaa; 4];
			assert!(
				normalize_event_data::<VersionedInstruction>(rejected, &mut workspace).is_err()
			);
		}

		let mut workspace = [0xaa; 3];
		assert_eq!(
			normalize_event_data::<VersionedInstruction>(&[7, 0, 42], &mut workspace),
			Err(PinaProgramError::MigrationBudgetExceeded.into())
		);
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

		fn validate_migration_destination(_: u32, data: &[u8]) -> ProgramResult {
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

		fn validate_migration_destination(_: u32, data: &[u8]) -> ProgramResult {
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

		fn apply_migration((): Self::Plan, _: &mut [u8]) {}

		fn validate_migration_destination(_: u32, _: &[u8]) -> ProgramResult {
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

		fn apply_migration((): Self::Plan, destination: &mut [u8]) {
			destination[2] = 99;
		}

		fn validate_migration_destination(_: u32, data: &[u8]) -> ProgramResult {
			if data != [Self::VALUE, 0, 42] && data != [Self::VALUE, 1, 42] {
				return Err(ProgramError::InvalidAccountData);
			}

			Ok(())
		}
	}

	#[cfg(feature = "account-resize")]
	struct TwoStepAccount;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for TwoStepAccount {
		type Type = u8;

		const VALUE: Self::Type = 11;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for TwoStepAccount {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 2;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for TwoStepAccount {
		type Plan = u32;

		const MAX_INLINE_STEPS: u16 = 2;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			match data {
				[Self::VALUE, 0, 5] => AccountMigrationPlan::try_new(0, 1, 4, 1, 0),
				[Self::VALUE, 1, 5, 9] => AccountMigrationPlan::try_new(1, 2, 5, 1, 1),
				_ => Err(ProgramError::InvalidAccountData),
			}
		}

		fn apply_migration(plan: Self::Plan, destination: &mut [u8]) {
			match plan {
				0 => destination[3] = 9,
				1 => destination[4] = 7,
				_ => {}
			}
		}

		fn validate_migration_destination(version: u32, data: &[u8]) -> ProgramResult {
			match (version, data) {
				(1, [Self::VALUE, _, 5, 9]) | (2, [Self::VALUE, _, 5, 9, 7]) => Ok(()),
				_ => Err(ProgramError::InvalidAccountData),
			}
		}
	}

	/// A two-hop ladder whose second hop grows, so the funding transfer runs
	/// from the post-mutation tail of the executor.
	#[cfg(feature = "account-resize")]
	struct FundedLadderAccount;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for FundedLadderAccount {
		type Type = u8;

		const VALUE: Self::Type = 12;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for FundedLadderAccount {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 2;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for FundedLadderAccount {
		type Plan = u32;

		const MAX_INLINE_STEPS: u16 = 2;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			match data {
				[Self::VALUE, 0, 5] => AccountMigrationPlan::try_new(0, 1, 4, 1, 0),
				[Self::VALUE, 1, 5, 9] => AccountMigrationPlan::try_new(1, 2, 5, 1, 1),
				_ => Err(ProgramError::InvalidAccountData),
			}
		}

		fn apply_migration(plan: Self::Plan, destination: &mut [u8]) {
			match plan {
				0 => destination[3] = 9,
				1 => destination[4] = 7,
				_ => {}
			}
		}

		fn validate_migration_destination(version: u32, data: &[u8]) -> ProgramResult {
			match (version, data) {
				(1, [Self::VALUE, _, 5, 9]) | (2, [Self::VALUE, _, 5, 9, 7]) => Ok(()),
				_ => Err(ProgramError::InvalidAccountData),
			}
		}
	}

	#[cfg(feature = "account-resize")]
	struct StepBudgetAccount;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for StepBudgetAccount {
		type Type = u8;

		const VALUE: Self::Type = 11;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for StepBudgetAccount {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 2;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for StepBudgetAccount {
		type Plan = u32;

		const MAX_INLINE_STEPS: u16 = 1;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			TwoStepAccount::plan_migration(data)
		}

		fn apply_migration(plan: Self::Plan, destination: &mut [u8]) {
			TwoStepAccount::apply_migration(plan, destination);
		}

		fn validate_migration_destination(version: u32, data: &[u8]) -> ProgramResult {
			TwoStepAccount::validate_migration_destination(version, data)
		}
	}

	#[cfg(feature = "account-resize")]
	struct PlannerFailureAfterMutation;

	#[cfg(feature = "account-resize")]
	impl HasDiscriminator for PlannerFailureAfterMutation {
		type Type = u8;

		const VALUE: Self::Type = 12;
	}

	#[cfg(feature = "account-resize")]
	impl HasMigrationVersion for PlannerFailureAfterMutation {
		type Version = u8;

		const CURRENT_VERSION: Self::Version = 2;
	}

	#[cfg(feature = "account-resize")]
	impl MigratableAccount for PlannerFailureAfterMutation {
		type Plan = ();

		const MAX_INLINE_STEPS: u16 = 2;

		fn plan_migration(data: &[u8]) -> Result<AccountMigrationPlan<Self::Plan>, ProgramError> {
			match data {
				[Self::VALUE, 0, 5] => AccountMigrationPlan::try_new(0, 1, 3, 1, ()),
				_ => Err(ProgramError::InvalidAccountData),
			}
		}

		fn apply_migration((): Self::Plan, _: &mut [u8]) {}

		fn validate_migration_destination(_: u32, _: &[u8]) -> ProgramResult {
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
	fn migrate_context_validates_the_funding_slot_and_system_program() {
		let program_id = Address::new_from_array([9; 32]);

		assert_eq!(
			MigrateContext::new(&program_id, &mut [], 0).err(),
			Some(ProgramError::NotEnoughAccountKeys)
		);

		// A payer that is not the program-address placeholder must be writable.
		let mut stored_payer =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), program_id, 500, &[]);
		stored_payer.header.is_writable = 0;
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut stored_target = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			500,
			&[7, 0, 42],
		);
		let mut views = [stored_payer.view(), system.view(), stored_target.view()];
		assert!(matches!(
			MigrateContext::new(&program_id, &mut views, 0),
			Err(ProgramError::InvalidAccountData)
		));

		// The rent transfers invoke the system program, so its slot must name it.
		let mut stored_payer =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), program_id, 500, &[]);
		let mut wrong_system =
			TestAccount::<8>::new(Address::new_from_array([7; 32]), program_id, 0, &[]);
		let mut stored_target = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			500,
			&[7, 0, 42],
		);
		let mut views = [
			stored_payer.view(),
			wrong_system.view(),
			stored_target.view(),
		];
		assert!(MigrateContext::new(&program_id, &mut views, 0).is_err());

		// Every migratable slot must be owned by the executing program.
		let mut stored_payer =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), program_id, 500, &[]);
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut foreign = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			Address::new_from_array([8; 32]),
			500,
			&[7, 0, 42],
		);
		let mut views = [stored_payer.view(), system.view(), foreign.view()];
		assert_eq!(
			MigrateContext::new(&program_id, &mut views, 0).err(),
			Some(ProgramError::InvalidAccountOwner)
		);

		// The program address in the payer slot marks an absent payer.
		let mut placeholder = TestAccount::<8>::new(program_id, program_id, 0, &[]);
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut stored_target = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			500,
			&[7, 0, 42],
		);
		let mut views = [placeholder.view(), system.view(), stored_target.view()];
		let context = MigrateContext::new(&program_id, &mut views, 0)
			.unwrap_or_else(|error| panic!("valid layout: {error:?}"));
		assert!(!context.migrated_any());
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn migrate_context_migrates_declared_slots_exactly_once() {
		let program_id = Address::new_from_array([9; 32]);
		let mut placeholder = TestAccount::<8>::new(program_id, program_id, 0, &[]);
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut stored = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			10_000,
			&[7, 0, 42],
		);
		let mut views = [placeholder.view(), system.view(), stored.view()];
		let mut context = MigrateContext::with_rent(&program_id, &mut views, 0, test_rent())
			.unwrap_or_else(|error| panic!("valid layout: {error:?}"));

		assert_eq!(
			context.run::<GrowingAccount>(0).err(),
			Some(ProgramError::NotEnoughAccountKeys)
		);
		assert_eq!(
			context.run::<GrowingAccount>(1).err(),
			Some(ProgramError::NotEnoughAccountKeys)
		);
		assert_eq!(
			context.run::<GrowingAccount>(3).err(),
			Some(ProgramError::NotEnoughAccountKeys)
		);

		let outcome = context
			.run::<GrowingAccount>(2)
			.unwrap_or_else(|error| panic!("migrate slot: {error:?}"));
		assert_eq!(
			outcome,
			AccountMigrationOutcome::Migrated {
				from: 0,
				to: 1,
				steps: 1,
			}
		);
		assert!(context.migrated_any());
		assert_eq!(
			context.run::<GrowingAccount>(2).err(),
			Some(PinaProgramError::DuplicateMutableAccount.into())
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn migrate_context_rejects_duplicate_account_slots() {
		let program_id = Address::new_from_array([9; 32]);
		let mut placeholder = TestAccount::<8>::new(program_id, program_id, 0, &[]);
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut stored = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			10_000,
			&[7, 0, 42],
		);
		let duplicate = stored.view();
		let mut views = [placeholder.view(), system.view(), stored.view(), duplicate];
		assert_eq!(
			MigrateContext::new(&program_id, &mut views, 0).err(),
			Some(PinaProgramError::DuplicateMutableAccount.into())
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn migrate_context_rejects_slots_with_the_wrong_discriminator() {
		let program_id = Address::new_from_array([9; 32]);
		let mut placeholder = TestAccount::<8>::new(program_id, program_id, 0, &[]);
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut stored = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			10_000,
			&[8, 0, 11, 42],
		);
		let mut views = [placeholder.view(), system.view(), stored.view()];
		let mut context = MigrateContext::with_rent(&program_id, &mut views, 0, test_rent())
			.unwrap_or_else(|error| panic!("valid layout: {error:?}"));

		assert_eq!(
			context.run::<GrowingAccount>(2).err(),
			Some(ProgramError::InvalidAccountData)
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn migrate_context_skips_absent_slots_and_honors_the_lamport_cap() {
		let program_id = Address::new_from_array([9; 32]);
		let mut placeholder = TestAccount::<8>::new(program_id, program_id, 0, &[]);
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut absent = TestAccount::<8>::new(program_id, program_id, 0, &[]);
		let mut stored = TestAccount::<8>::new(
			Address::new_from_array([2; 32]),
			program_id,
			10_000,
			&[7, 0, 42],
		);
		let mut views = [
			placeholder.view(),
			system.view(),
			absent.view(),
			stored.view(),
		];
		let mut context = MigrateContext::with_rent(&program_id, &mut views, 0, test_rent())
			.unwrap_or_else(|error| panic!("valid layout: {error:?}"));

		assert_eq!(context.run_optional::<GrowingAccount>(2).ok(), Some(None));
		assert_eq!(context.run_optional::<GrowingAccount>(9).ok(), Some(None));
		assert!(
			context
				.run_optional::<GrowingAccount>(3)
				.ok()
				.flatten()
				.is_some()
		);

		// A growing step whose rent deficit exceeds the cap fails closed.
		let mut payer =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), program_id, 10_000, &[]);
		let mut system = TestAccount::<8>::new(crate::system::ID, program_id, 0, &[]);
		let mut poor =
			TestAccount::<8>::new(Address::new_from_array([3; 32]), program_id, 0, &[7, 0, 42]);
		let mut views = [payer.view(), system.view(), poor.view()];
		let mut context = MigrateContext::with_rent(&program_id, &mut views, 0, test_rent())
			.unwrap_or_else(|error| panic!("valid layout: {error:?}"));
		assert_eq!(
			context.run::<GrowingAccount>(2).err(),
			Some(PinaProgramError::MigrationBudgetExceeded.into())
		);
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
	fn executor_replans_each_adjacent_variable_length_step_atomically() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored =
			TestAccount::<32>::new(Address::new_from_array([1; 32]), owner, 10_000, &[11, 0, 5]);
		let mut account = stored.view();
		let outcome = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: 0,
		}
		.invoke_with_rent::<TwoStepAccount>(test_rent())
		.unwrap_or_else(|error| panic!("migrate two-step account: {error:?}"));

		assert_eq!(
			outcome,
			AccountMigrationOutcome::Migrated {
				from: 0,
				to: 2,
				steps: 2,
			}
		);
		assert_eq!(account.data_len(), 5);
		assert_eq!(&*account.try_borrow().unwrap(), &[11, 2, 5, 9, 7]);
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
	fn public_executor_entrypoints_return_already_current_without_syscalls() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored = TestAccount::<8>::new(
			Address::new_from_array([1; 32]),
			owner,
			1_000,
			&[7, 1, 42, 99],
		);
		let mut account = stored.view();
		let mut migration = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: 0,
		};
		assert_eq!(
			migration.invoke::<GrowingAccount>(),
			Ok(AccountMigrationOutcome::AlreadyCurrent { version: 1 })
		);
		assert_eq!(
			migration.invoke_signed::<GrowingAccount>(&[]),
			Ok(AccountMigrationOutcome::AlreadyCurrent { version: 1 })
		);
		assert_eq!(
			VersionedInstruction::validate_current_instruction(&[7, 1, 42, 13]),
			Err(ProgramError::InvalidInstructionData)
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn public_executor_propagates_unavailable_rent_before_mutation() {
		let owner = Address::new_from_array([9; 32]);
		let original = [7, 0, 42];
		let mut stored =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 0, &original);
		let mut account = stored.view();
		let result = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: u64::MAX,
		}
		.invoke::<GrowingAccount>();

		assert_eq!(result, Err(ProgramError::UnsupportedSysvar));
		assert_eq!(&*account.try_borrow().unwrap(), &original);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_funds_growth_only_from_a_valid_explicit_payer() {
		let owner = Address::new_from_array([9; 32]);
		let mut stored_account =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 0, &[7, 0, 42]);
		let mut stored_payer =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1_000, &[]);
		let mut account = stored_account.view();
		let payer = stored_payer.view();
		let outcome = MigrateAccount {
			account: &mut account,
			payer: Some(&payer),
			program_id: &owner,
			max_lamports: u64::MAX,
		}
		.invoke_with_rent::<GrowingAccount>(test_rent())
		.unwrap_or_else(|error| panic!("funded migration: {error:?}"));

		assert!(matches!(
			outcome,
			AccountMigrationOutcome::Migrated {
				from: 0,
				to: 1,
				steps: 1
			}
		));
		assert_eq!(&*account.try_borrow().unwrap(), &[7, 1, 42, 99]);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_rejects_invalid_or_borrowed_payers_before_account_mutation() {
		let owner = Address::new_from_array([9; 32]);
		let original = [7, 0, 42];
		let mut stored_account =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 0, &original);
		let mut stored_readonly_payer =
			TestAccount::<0>::new(Address::new_from_array([2; 32]), owner, 1_000, &[]);
		stored_readonly_payer.header.is_writable = 0;
		let mut account = stored_account.view();
		let readonly_payer = stored_readonly_payer.view();
		assert_eq!(
			MigrateAccount {
				account: &mut account,
				payer: Some(&readonly_payer),
				program_id: &owner,
				max_lamports: u64::MAX,
			}
			.invoke_with_rent::<GrowingAccount>(test_rent()),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(&*account.try_borrow().unwrap(), &original);

		let mut stored_account =
			TestAccount::<8>::new(Address::new_from_array([3; 32]), owner, 0, &original);
		let mut stored_borrowed_payer =
			TestAccount::<1>::new(Address::new_from_array([4; 32]), owner, 1_000, &[0]);
		let mut account = stored_account.view();
		let borrowed_payer = stored_borrowed_payer.view();
		let payer_guard = borrowed_payer
			.try_borrow()
			.unwrap_or_else(|error| panic!("borrow payer: {error:?}"));
		assert_eq!(
			MigrateAccount {
				account: &mut account,
				payer: Some(&borrowed_payer),
				program_id: &owner,
				max_lamports: u64::MAX,
			}
			.invoke_with_rent::<GrowingAccount>(test_rent()),
			Err(ProgramError::AccountBorrowFailed)
		);
		drop(payer_guard);
		assert_eq!(&*account.try_borrow().unwrap(), &original);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn invalid_rent_is_rejected_before_account_or_payer_mutation() {
		let owner = Address::new_from_array([9; 32]);
		let original = [7, 0, 42];
		let mut stored =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 0, &original);
		let mut account = stored.view();
		let rent = Rent::from_bytes(&u64::MAX.to_le_bytes())
			.unwrap_or_else(|error| panic!("construct invalid rent: {error:?}"));
		let result = MigrateAccount {
			account: &mut account,
			payer: None,
			program_id: &owner,
			max_lamports: u64::MAX,
		}
		.invoke_with_rent::<GrowingAccount>(rent);

		assert_eq!(result, Err(ProgramError::InvalidArgument));
		assert_eq!(&*account.try_borrow().unwrap(), &original);
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

		let mut stored =
			TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 1_000, &[11, 0, 5]);
		let mut account = stored.view();
		assert_eq!(
			MigrateAccount {
				account: &mut account,
				payer: None,
				program_id: &owner,
				max_lamports: u64::MAX,
			}
			.invoke_with_rent::<StepBudgetAccount>(test_rent()),
			Err(PinaProgramError::MigrationUnavailable.into())
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn adversarial_fixture_contracts_reject_every_unmodeled_shape() {
		assert_eq!(
			ShrinkingAccount::plan_migration(&[8, 0, 1]),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			ShrinkingAccount::validate_migration_destination(1, &[8, 1]),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			AdversarialPlanAccount::plan_migration(&[9, 0, 3]),
			Err(ProgramError::InvalidAccountData)
		);
		AdversarialPlanAccount::apply_migration((), &mut []);
		assert_eq!(
			AdversarialPlanAccount::validate_migration_destination(1, &[]),
			Ok(())
		);
		assert_eq!(
			InvalidDestinationAccount::plan_migration(&[10, 0, 41]),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			InvalidDestinationAccount::validate_migration_destination(1, &[10, 1, 99]),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			InvalidDestinationAccount::validate_migration_destination(1, &[10, 1, 42]),
			Ok(())
		);
		assert_eq!(
			TwoStepAccount::plan_migration(&[11, 2, 5, 9, 7]),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(
			FundedLadderAccount::plan_migration(&[12, 2, 5, 9, 7]),
			Err(ProgramError::InvalidAccountData)
		);
		FundedLadderAccount::apply_migration(9, &mut [0_u8; 1]);
		assert_eq!(
			FundedLadderAccount::validate_migration_destination(2, &[12, 2, 5]),
			Err(ProgramError::InvalidAccountData)
		);
		let mut ignored = [0_u8; 1];
		TwoStepAccount::apply_migration(2, &mut ignored);
		assert_eq!(
			TwoStepAccount::validate_migration_destination(3, &ignored),
			Err(ProgramError::InvalidAccountData)
		);
		let plan = StepBudgetAccount::plan_migration(&[11, 0, 5])
			.unwrap_or_else(|error| panic!("delegate migration plan: {error:?}"));
		let mut destination = [11, 0, 5, 0];
		StepBudgetAccount::apply_migration(plan.into_payload(), &mut destination);
		assert_eq!(
			StepBudgetAccount::validate_migration_destination(1, &[11, 1, 5, 9]),
			Ok(())
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_aborts_instead_of_returning_an_error_after_mutation() {
		const RETURNED_EXIT_CODE: i32 = 86;
		for scenario in ["destination", "planner"] {
			let output = std::process::Command::new(
				std::env::current_exe()
					.unwrap_or_else(|error| panic!("locate test binary: {error}")),
			)
			.arg("--exact")
			.arg("migration::tests::post_mutation_invariant_failure_child")
			.arg("--nocapture")
			.env("PINA_TEST_POST_MUTATION_ABORT_CHILD", scenario)
			.output()
			.unwrap_or_else(|error| panic!("run {scenario} abort probe: {error}"));

			assert!(!output.status.success(), "{scenario} failure was returned");
			assert_ne!(output.status.code(), Some(RETURNED_EXIT_CODE),);
		}
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn executor_aborts_when_a_later_hop_needs_funding() {
		let output = std::process::Command::new(
			std::env::current_exe().unwrap_or_else(|error| panic!("locate test binary: {error}")),
		)
		.arg("--exact")
		.arg("migration::tests::funded_ladder_transfer_failure_child")
		.arg("--nocapture")
		.env("PINA_TEST_FUNDED_LADDER_CHILD", "funded")
		.output()
		.unwrap_or_else(|error| panic!("run funded ladder probe: {error}"));

		assert!(
			!output.status.success(),
			"a second-hop funding failure was returned instead of aborting"
		);
		assert_ne!(output.status.code(), Some(86));
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn funded_ladder_transfer_failure_child() {
		let Some(scenario) = std::env::var_os("PINA_TEST_FUNDED_LADDER_CHILD") else {
			return;
		};
		assert_eq!(scenario, "funded");

		let owner = Address::new_from_array([9; 32]);
		let rent = test_rent();
		// Exactly rent-exempt at the v1 size: hop one resizes for free, and
		// hop two must then fund one more lamport of rent from the payer.
		let start = rent
			.try_minimum_balance(4)
			.unwrap_or_else(|error| panic!("rent for v1: {error}"));
		let mut stored =
			TestAccount::<32>::new(Address::new_from_array([1; 32]), owner, start, &[12, 0, 5]);
		let mut stored_payer =
			TestAccount::<32>::new(Address::new_from_array([2; 32]), owner, 10_000, &[]);
		let mut account = stored.view();
		let mut payer = stored_payer.view();

		let outcome = MigrateAccount {
			account: &mut account,
			payer: Some(&payer),
			program_id: &owner,
			max_lamports: u64::MAX,
		}
		.invoke_with_rent::<FundedLadderAccount>(rent);

		// Reaching this point means the post-mutation transfer was returned
		// as a catchable error instead of aborting the instruction.
		assert!(
			matches!(outcome, Err(ProgramError::InvalidArgument)),
			"the funded ladder must abort at the second transfer: {outcome:?}"
		);
	}

	#[cfg(feature = "account-resize")]
	#[test]
	fn post_mutation_invariant_failure_child() {
		const RETURNED_EXIT_CODE: i32 = 86;
		let Some(scenario) = std::env::var_os("PINA_TEST_POST_MUTATION_ABORT_CHILD") else {
			return;
		};

		let owner = Address::new_from_array([9; 32]);
		if scenario == "destination" {
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
		} else {
			let mut stored =
				TestAccount::<8>::new(Address::new_from_array([1; 32]), owner, 1_000, &[12, 0, 5]);
			let mut account = stored.view();
			let _result = MigrateAccount {
				account: &mut account,
				payer: None,
				program_id: &owner,
				max_lamports: 0,
			}
			.invoke_with_rent::<PlannerFailureAfterMutation>(test_rent());
		}

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
