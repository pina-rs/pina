//! Pina's official security lints.
//!
//! This crate is Pina's self-contained replacement for the Dylint lint setup:
//! every security, performance, and IDL lint that Pina ships lives here, is
//! importable from one place, and is registered by a single
//! [`register_all_lints`] entry point.
//!
//! # Structure
//!
//! The crate mirrors the Dylint library structure:
//!
//! - Each lint lives in its own module under [`lints`] and declares itself with
//!   the Dylint-style [`declare_late_lint!`] or [`declare_pre_expansion_lint!`]
//!   macros.
//! - Unlike Dylint, the lints are compiled into the crate instead of being
//!   distributed as separate cdylib packages. The crate still builds as a
//!   cdylib and exports the Dylint-compatible `register_lints` symbol, so any
//!   Dylint driver can load it as a single "pina" library.
//! - The bundled `pina_lint_driver` binary is a `rustc` wrapper that registers
//!   every lint in this crate without dynamic loading, so `pina lint` needs no
//!   external lint tooling.
//!
//! # Importing lints
//!
//! Every lint constant and pass is public:
//!
//! ```rust,ignore
//! use pina_lints::lints::require_owner_before_token_cast;
//! ```
//!
//! The full catalog is available through [`LINTS`] for tooling that wants to
//! validate lint names or default levels.
//!
//! # Configuration
//!
//! Lint levels are configured in the project's `pina.toml` under the `[lints]`
//! table. The Pina CLI reads that table and passes the result to
//! `pina_lint_driver` through the `PINA_LINT_LEVELS` environment variable; the
//! driver forwards the levels to `rustc` as `--allow`, `--warn`, or `--deny`
//! arguments.

// The lint passes and driver link against the Rust compiler's unstable
// `rustc_private` crates. Enabling that requires nightly features, so the
// workspace-wide `unstable_features = "deny"` lint is waived for this crate.
#![feature(rustc_private)]
#![allow(unstable_features)]
#![allow(clippy::useless_attribute)]

// Linking against `librustc_driver` makes the Rust compiler crates resolved
// through the single driver dylib, which is how rustc_private consumers share
// one copy of the compiler state.
extern crate rustc_driver;
extern crate rustc_lint;
extern crate rustc_session;

pub mod lints;
mod macros;
pub mod shared;

pub use paste;

/// Version of the `pina_lints` driver/library protocol.
///
/// A driver refuses to load a library whose version differs from its own.
pub const PINA_LINTS_VERSION: &str = "1";

/// Dylint protocol version.
///
/// `dylint_version` is kept for compatibility so a Dylint driver can load this
/// crate's cdylib like any other Dylint library.
pub const DYLINT_VERSION: &str = "0.1.0";

/// Every lint in this crate, in catalog (alphabetical) order.
pub static LINTS: &[&'static rustc_lint::Lint] = &[
	lints::deny_account_borrows_across_cpi::DENY_ACCOUNT_BORROWS_ACROSS_CPI,
	lints::deny_heap_allocations_in_onchain_instruction_handlers::DENY_HEAP_ALLOCATIONS_IN_ONCHAIN_INSTRUCTION_HANDLERS,
	lints::require_associated_token_address_before_ata_cast::REQUIRE_ASSOCIATED_TOKEN_ADDRESS_BEFORE_ATA_CAST,
	lints::require_bounded_remaining_accounts::REQUIRE_BOUNDED_REMAINING_ACCOUNTS,
	lints::require_canonical_bump_before_pda_write::REQUIRE_CANONICAL_BUMP_BEFORE_PDA_WRITE,
	lints::require_canonical_instruction_dispatch_for_idl::REQUIRE_CANONICAL_INSTRUCTION_DISPATCH_FOR_IDL,
	lints::require_checked_asset_arithmetic::REQUIRE_CHECKED_ASSET_ARITHMETIC,
	lints::require_consistent_token_program::REQUIRE_CONSISTENT_TOKEN_PROGRAM,
	lints::require_empty_before_init::REQUIRE_EMPTY_BEFORE_INIT,
	lints::require_explicit_discriminators_and_seed_namespaces::REQUIRE_EXPLICIT_DISCRIMINATORS_AND_SEED_NAMESPACES,
	lints::require_explicit_token_2022_extension_policy::REQUIRE_EXPLICIT_TOKEN_2022_EXTENSION_POLICY,
	lints::require_idl_root_to_define_one_program_id::REQUIRE_IDL_ROOT_TO_DEFINE_ONE_PROGRAM_ID,
	lints::require_owner_before_token_cast::REQUIRE_OWNER_BEFORE_TOKEN_CAST,
	lints::require_post_cpi_balance_reload::REQUIRE_POST_CPI_BALANCE_RELOAD,
	lints::require_program_check_before_cpi::REQUIRE_PROGRAM_CHECK_BEFORE_CPI,
	lints::require_program_owned_before_lamport_mutation::REQUIRE_PROGRAM_OWNED_BEFORE_LAMPORT_MUTATION,
	lints::require_reason_for_duplicate_remaining_accounts::REQUIRE_REASON_FOR_DUPLICATE_REMAINING_ACCOUNTS,
	lints::require_sysvar_assert_before_sysvar_use::REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE,
	lints::require_type_assert_before_zero_copy_cast::REQUIRE_TYPE_ASSERT_BEFORE_ZERO_COPY_CAST,
	lints::require_writable_before_account_resize::REQUIRE_WRITABLE_BEFORE_ACCOUNT_RESIZE,
	lints::require_zeroed_before_close::REQUIRE_ZEROED_BEFORE_CLOSE,
];

/// Names of every lint in this crate, in catalog (alphabetical) order.
pub const LINT_NAMES: &[&str] = &[
	"deny_account_borrows_across_cpi",
	"deny_heap_allocations_in_onchain_instruction_handlers",
	"require_associated_token_address_before_ata_cast",
	"require_bounded_remaining_accounts",
	"require_canonical_bump_before_pda_write",
	"require_canonical_instruction_dispatch_for_idl",
	"require_checked_asset_arithmetic",
	"require_consistent_token_program",
	"require_empty_before_init",
	"require_explicit_discriminators_and_seed_namespaces",
	"require_explicit_token_2022_extension_policy",
	"require_idl_root_to_define_one_program_id",
	"require_owner_before_token_cast",
	"require_post_cpi_balance_reload",
	"require_program_check_before_cpi",
	"require_program_owned_before_lamport_mutation",
	"require_reason_for_duplicate_remaining_accounts",
	"require_sysvar_assert_before_sysvar_use",
	"require_type_assert_before_zero_copy_cast",
	"require_writable_before_account_resize",
	"require_zeroed_before_close",
];

/// A catalog entry described with plain Rust types.
///
/// Unlike [`LINTS`], this type is usable without `rustc_private`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LintInfo {
	/// The lint's registered name.
	pub name: &'static str,
	/// The lint's default level.
	pub default_level: &'static str,
	/// The lint's description.
	pub desc: &'static str,
}

/// Iterate the lint catalog with plain Rust types, in catalog order.
#[must_use]
pub fn catalog() -> impl Iterator<Item = LintInfo> {
	LINT_NAMES.iter().zip(LINTS).map(|(name, lint)| {
		LintInfo {
			name,
			default_level: lint.default_level.as_str(),
			desc: lint.desc,
		}
	})
}

/// Register every Pina lint with the given lint store.
///
/// This is the single entry point used by the bundled `pina_lint_driver` and by
/// the Dylint-compatible [`register_lints`] symbol.
pub fn register_all_lints(_sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
	for name in LINT_NAMES {
		register_one_lint(name, lint_store);
	}
}

/// Register the named subset of Pina lints with the given lint store.
///
/// Unknown names are ignored; callers that need validation should check the
/// names against [`LINT_NAMES`] first. The bundled driver uses this to lint
/// with a single lint when `PINA_LINT_ONLY` is set.
pub fn register_selected_lints(
	_sess: &rustc_session::Session,
	lint_store: &mut rustc_lint::LintStore,
	names: &[&str],
) {
	for name in names {
		register_one_lint(name, lint_store);
	}
}

/// Register one lint and its pass; unknown names are ignored.
fn register_one_lint(name: &str, lint_store: &mut rustc_lint::LintStore) {
	match name {
		"deny_account_borrows_across_cpi" => {
			lint_store.register_lints(&[
				lints::deny_account_borrows_across_cpi::DENY_ACCOUNT_BORROWS_ACROSS_CPI,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::deny_account_borrows_across_cpi::DenyAccountBorrowsAcrossCpi)
			});
		}
		"deny_heap_allocations_in_onchain_instruction_handlers" => {
			lint_store.register_lints(&[
				lints::deny_heap_allocations_in_onchain_instruction_handlers::DENY_HEAP_ALLOCATIONS_IN_ONCHAIN_INSTRUCTION_HANDLERS,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::deny_heap_allocations_in_onchain_instruction_handlers::DenyHeapAllocationsInOnchainInstructionHandlers,
				)
			});
		}
		"require_associated_token_address_before_ata_cast" => {
			lint_store.register_lints(&[
				lints::require_associated_token_address_before_ata_cast::REQUIRE_ASSOCIATED_TOKEN_ADDRESS_BEFORE_ATA_CAST,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_associated_token_address_before_ata_cast::RequireAssociatedTokenAddressBeforeAtaCast,
				)
			});
		}
		"require_bounded_remaining_accounts" => {
			lint_store.register_lints(&[
				lints::require_bounded_remaining_accounts::REQUIRE_BOUNDED_REMAINING_ACCOUNTS,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_bounded_remaining_accounts::RequireBoundedRemainingAccounts)
			});
		}
		"require_canonical_bump_before_pda_write" => {
			lint_store.register_lints(&[
				lints::require_canonical_bump_before_pda_write::REQUIRE_CANONICAL_BUMP_BEFORE_PDA_WRITE,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_canonical_bump_before_pda_write::RequireCanonicalBumpBeforePdaWrite,
				)
			});
		}
		"require_canonical_instruction_dispatch_for_idl" => {
			lint_store.register_lints(&[
				lints::require_canonical_instruction_dispatch_for_idl::REQUIRE_CANONICAL_INSTRUCTION_DISPATCH_FOR_IDL,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_canonical_instruction_dispatch_for_idl::RequireCanonicalInstructionDispatchForIdl,
				)
			});
		}
		"require_checked_asset_arithmetic" => {
			lint_store.register_lints(&[
				lints::require_checked_asset_arithmetic::REQUIRE_CHECKED_ASSET_ARITHMETIC,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_checked_asset_arithmetic::RequireCheckedAssetArithmetic)
			});
		}
		"require_consistent_token_program" => {
			lint_store.register_lints(&[
				lints::require_consistent_token_program::REQUIRE_CONSISTENT_TOKEN_PROGRAM,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_consistent_token_program::RequireConsistentTokenProgram)
			});
		}
		"require_empty_before_init" => {
			lint_store
				.register_lints(&[lints::require_empty_before_init::REQUIRE_EMPTY_BEFORE_INIT]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_empty_before_init::RequireEmptyBeforeInit)
			});
		}
		"require_explicit_discriminators_and_seed_namespaces" => {
			lint_store.register_lints(&[
				lints::require_explicit_discriminators_and_seed_namespaces::REQUIRE_EXPLICIT_DISCRIMINATORS_AND_SEED_NAMESPACES,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_explicit_discriminators_and_seed_namespaces::RequireExplicitDiscriminatorsAndSeedNamespaces,
				)
			});
		}
		"require_explicit_token_2022_extension_policy" => {
			lint_store.register_lints(&[
				lints::require_explicit_token_2022_extension_policy::REQUIRE_EXPLICIT_TOKEN_2022_EXTENSION_POLICY,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_explicit_token_2022_extension_policy::RequireExplicitToken2022ExtensionPolicy,
				)
			});
		}
		"require_idl_root_to_define_one_program_id" => {
			lint_store.register_lints(&[
				lints::require_idl_root_to_define_one_program_id::REQUIRE_IDL_ROOT_TO_DEFINE_ONE_PROGRAM_ID,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_idl_root_to_define_one_program_id::RequireIdlRootToDefineOneProgramId,
				)
			});
		}
		"require_owner_before_token_cast" => {
			lint_store.register_lints(&[
				lints::require_owner_before_token_cast::REQUIRE_OWNER_BEFORE_TOKEN_CAST,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_owner_before_token_cast::RequireOwnerBeforeTokenCast)
			});
		}
		"require_post_cpi_balance_reload" => {
			lint_store.register_lints(&[
				lints::require_post_cpi_balance_reload::REQUIRE_POST_CPI_BALANCE_RELOAD,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_post_cpi_balance_reload::RequirePostCpiBalanceReload)
			});
		}
		"require_program_check_before_cpi" => {
			lint_store.register_lints(&[
				lints::require_program_check_before_cpi::REQUIRE_PROGRAM_CHECK_BEFORE_CPI,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_program_check_before_cpi::RequireProgramCheckBeforeCpi)
			});
		}
		"require_program_owned_before_lamport_mutation" => {
			lint_store.register_lints(&[
				lints::require_program_owned_before_lamport_mutation::REQUIRE_PROGRAM_OWNED_BEFORE_LAMPORT_MUTATION,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_program_owned_before_lamport_mutation::RequireProgramOwnedBeforeLamportMutation,
				)
			});
		}
		"require_reason_for_duplicate_remaining_accounts" => {
			lint_store.register_lints(&[
				lints::require_reason_for_duplicate_remaining_accounts::REQUIRE_REASON_FOR_DUPLICATE_REMAINING_ACCOUNTS,
			]);
			lint_store.register_pre_expansion_pass(|| {
				Box::new(
					lints::require_reason_for_duplicate_remaining_accounts::RequireReasonForDuplicateRemainingAccounts,
				)
			});
		}
		"require_sysvar_assert_before_sysvar_use" => {
			lint_store.register_lints(&[
				lints::require_sysvar_assert_before_sysvar_use::REQUIRE_SYSVAR_ASSERT_BEFORE_SYSVAR_USE,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_sysvar_assert_before_sysvar_use::RequireSysvarAssertBeforeSysvarUse,
				)
			});
		}
		"require_type_assert_before_zero_copy_cast" => {
			lint_store.register_lints(&[
				lints::require_type_assert_before_zero_copy_cast::REQUIRE_TYPE_ASSERT_BEFORE_ZERO_COPY_CAST,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_type_assert_before_zero_copy_cast::RequireTypeAssertBeforeZeroCopyCast,
				)
			});
		}
		"require_writable_before_account_resize" => {
			lint_store.register_lints(&[
				lints::require_writable_before_account_resize::REQUIRE_WRITABLE_BEFORE_ACCOUNT_RESIZE,
			]);
			lint_store.register_late_pass(|_| {
				Box::new(
					lints::require_writable_before_account_resize::RequireWritableBeforeAccountResize,
				)
			});
		}
		"require_zeroed_before_close" => {
			lint_store
				.register_lints(&[lints::require_zeroed_before_close::REQUIRE_ZEROED_BEFORE_CLOSE]);
			lint_store.register_late_pass(|_| {
				Box::new(lints::require_zeroed_before_close::RequireZeroedBeforeClose)
			});
		}
		_ => {}
	}
}

/// Dylint-compatible registration symbol.
///
/// A Dylint driver (or any other driver that understands Dylint libraries)
/// loads the cdylib built from this crate and calls this symbol. The bundled
/// `pina_lint_driver` instead calls [`register_all_lints`] directly.
#[unsafe(no_mangle)]
pub fn register_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
	register_all_lints(sess, lint_store);
}

/// Protocol version symbol checked by `pina_lint_driver` and compatible tools.
#[unsafe(no_mangle)]
pub extern "C" fn pina_lints_version() -> *mut std::os::raw::c_char {
	std::ffi::CString::new(PINA_LINTS_VERSION)
		.expect("protocol version contains no interior NUL bytes")
		.into_raw()
}

/// Dylint protocol version symbol kept for Dylint driver compatibility.
#[unsafe(no_mangle)]
pub extern "C" fn dylint_version() -> *mut std::os::raw::c_char {
	std::ffi::CString::new(DYLINT_VERSION)
		.expect("protocol version contains no interior NUL bytes")
		.into_raw()
}
