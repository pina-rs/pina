//! Remedy text shared by migration cost warnings and the on-chain executor
//! errors.
//!
//! `pina migrations make` quotes these strings when it warns about a growing
//! account, and the cost preview (#341) reuses them, so a pre-deploy warning
//! and the `PinaProgramError` a transaction fails with name the same fix. Each
//! string mirrors the rustdoc on the matching `pina::PinaProgramError`
//! variant; keep the two in step when a constant changes.

/// Remedy for `pina::PinaProgramError::MigrationAccountGrowthExceeded`.
///
/// The executor returns that code when one instruction would grow an account
/// by more than the runtime's `MAX_PERMITTED_DATA_INCREASE` (10,240 bytes).
pub(crate) const ACCOUNT_GROWTH_REMEDY: &str =
	"keep every released version within `MAX_PERMITTED_DATA_INCREASE` (10,240 bytes) of the \
	 version a stale account may hold and publish intermediate versions for larger changes";

/// Remedy for `pina::PinaProgramError::MigrationLamportBudgetExceeded`.
///
/// The executor returns that code when the rent deficit exceeds the
/// `max_lamports` budget the program passes to `MigrateAccount` or
/// `MigrateContext`. The `make` growth warning quotes this string so the
/// printed estimate and the on-chain failure name the same constant.
pub(crate) const LAMPORT_BUDGET_REMEDY: &str =
	"raise the program's lamport budget (`max_lamports` passed to `MigrateAccount` or \
	 `MigrateContext`, a constant such as `MAX_INLINE_MIGRATION_LAMPORTS`) to cover that deficit";
