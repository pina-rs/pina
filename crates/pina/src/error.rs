/// Built-in pina framework errors.
///
/// These occupy the top end of the `u32` range (`0xFFFF_0000..=0xFFFF_FFFF`)
/// to avoid collisions with user-defined program errors. User `#[error]` enums
/// should use discriminant values below `0xFFFF_0000` to prevent overlap.
///
/// <!-- {=pinaPublicResultContract|trim|linePrefix:"/// ":true} -->
/// All APIs in this section are designed for on-chain determinism.
///
/// They return `ProgramError` values for caller-side propagation with `?`.
///
/// No panics needed.<!-- {/pinaPublicResultContract} -->
#[repr(u32)]
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PinaProgramError {
	/// The caller-owned migration workspace cannot hold the transition.
	///
	/// Returned when a generated transition's `WORKING_SIZE` is smaller than
	/// its `CURRENT_SIZE`, or when the workspace passed to
	/// [`crate::normalize_instruction_data`] or [`crate::normalize_event_data`]
	/// is shorter than `WORKING_SIZE`.
	///
	/// # Remedy
	///
	/// Pass a workspace of at least the generated `WORKING_SIZE` bytes, and
	/// keep the generated workspace at or below
	/// [`crate::MAX_MIGRATION_WORKSPACE`] (1,024 bytes). Generated
	/// `with_current_instruction_data` and `with_current_event_data` helpers
	/// already size their stack workspace at the compile-time maximum, so a
	/// manual caller only needs this when it supplies its own buffer.
	MigrationWorkspaceExceeded = 0xFFFF_FFF4,
	/// One inline migration step would grow the account past the runtime
	/// realloc limit.
	///
	/// Returned when the allocation a step needs exceeds the account's size at
	/// the start of the instruction by more than `MAX_PERMITTED_DATA_INCREASE`
	/// (10,240 bytes), the per-instruction growth cap the Solana runtime
	/// enforces.
	///
	/// # Remedy
	///
	/// Keep every released version within 10,240 bytes of the version a stale
	/// account may still hold, and publish intermediate versions so a larger
	/// change grows across separate transactions. No `max_lamports` budget can
	/// raise this limit; it lives in the runtime.
	MigrationAccountGrowthExceeded = 0xFFFF_FFF3,
	/// The rent funding a migration needs exceeds this invocation's lamport
	/// budget.
	///
	/// Returned when the cumulative rent deficit of the steps already planned
	/// exceeds the `max_lamports` field of `MigrateAccount`, or the
	/// instruction-wide cap passed to `MigrateContext`. The account stays
	/// stale until the budget is raised; nothing is half-migrated.
	///
	/// # Remedy
	///
	/// Raise the program's `max_lamports` constant — the migrations example
	/// calls it `MAX_INLINE_MIGRATION_LAMPORTS` — until it covers the quoted
	/// deficit, or pass a larger budget to the sweep. `pina migrations make`
	/// prints the estimated deficit for each growing transition, about 6,960
	/// lamports per grown byte.
	MigrationLamportBudgetExceeded = 0xFFFF_FFF2,
	/// A generated migration exceeds its configured step, growth, or rent budget.
	///
	/// Legacy aggregate code, kept at its original value so published program
	/// binaries stay decodable. Builds before the split returned this variant
	/// for every budget failure; current builds return
	/// [`Self::MigrationWorkspaceExceeded`],
	/// [`Self::MigrationAccountGrowthExceeded`], or
	/// [`Self::MigrationLamportBudgetExceeded`] instead. Clients should decode
	/// all four.
	MigrationBudgetExceeded = 0xFFFF_FFF5,
	/// No generated transition can safely satisfy the requested historical
	/// contract.
	///
	/// Returned when a stale account is more than the account's generated
	/// `MAX_INLINE_STEPS` adjacent transitions behind the current version (the
	/// framework caps a generated ladder at 8 steps), when a generated plan
	/// does not describe exactly the next adjacent step, or when a
	/// reserved-instruction slot index is past the 63-slot bitmask.
	///
	/// # Remedy
	///
	/// Rebalance the version history so no live account falls more than
	/// `MAX_INLINE_STEPS` (at most 8) versions behind, and migrate accounts
	/// before they fall further behind. The reserved `Migrate` instruction
	/// (`MigrateContext`) is the out-of-band route when the touching
	/// instruction cannot carry a payer, but it enforces the same step cap.
	/// Regenerate transitions with `pina migrations make` when a plan's version
	/// or size shape drifted rather than an account being stale.
	MigrationUnavailable = 0xFFFF_FFF6,
	/// A stored version is malformed, unknown, or newer than this program.
	InvalidMigrationVersion = 0xFFFF_FFF7,
	/// The operation needs a dedicated migration or authorized funding first.
	MigrationRequired = 0xFFFF_FFF8,
	/// Two mutable account fields point at the same runtime account.
	DuplicateMutableAccount = 0xFFFF_FFF9,
	/// Account or instruction data is shorter than the expected minimum.
	DataTooShort = 0xFFFF_FFFA,
	/// Account size does not match the expected type size.
	InvalidAccountSize = 0xFFFF_FFFB,
	/// Account is not owned by the expected token program.
	InvalidTokenOwner = 0xFFFF_FFFC,
	/// A verified CPI did not move the balance it was asked to move.
	///
	/// Returned by the [`crate::token::verified`] helpers when the observed
	/// debit differs from the requested amount, when a transfer moved nothing,
	/// or when the requested amount was zero. A CPI that transfers less than
	/// the caller accounted for silently credits the recipient short, so the
	/// reconciliation refuses to guess which side is wrong.
	UnverifiedTransfer = 0xFFFF_FFF1,
	/// Too many PDA seeds were provided.
	SeedsTooMany = 0xFFFF_FFFD,
	/// More account keys were provided than the instruction expects.
	TooManyAccountKeys = 0xFFFF_FFFE,
	/// The discriminator bytes do not match any known variant.
	InvalidDiscriminator = 0xFFFF_FFFF,
}

impl From<PinaProgramError> for crate::ProgramError {
	fn from(error: PinaProgramError) -> Self {
		crate::ProgramError::Custom(error as u32)
	}
}
