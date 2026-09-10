//! Pina's lint catalog.
//!
//! Every lint is public and importable:
//!
//! ```rust,ignore
//! use pina_lints::lints::require_consistent_token_program::{
//!     REQUIRE_CONSISTENT_TOKEN_PROGRAM, RequireConsistentTokenProgram,
//! };
//! ```
//!
//! The lints are registered together by [`crate::register_all_lints`].

pub mod deny_account_borrows_across_cpi;
pub mod deny_heap_allocations_in_onchain_instruction_handlers;
pub mod deny_unchecked_remaining_mut;
pub mod deny_unused_account_borrow_guards;
pub mod require_bounded_remaining_accounts;
pub mod require_canonical_bump_before_pda_write;
pub mod require_canonical_instruction_dispatch_for_idl;
pub mod require_checked_asset_arithmetic;
pub mod require_consistent_token_program;
pub mod require_explicit_discriminators_and_seed_namespaces;
pub mod require_explicit_token_2022_extension_policy;
pub mod require_idl_root_to_define_one_program_id;
pub mod require_post_cpi_balance_reload;
pub mod require_program_check_before_cpi;
pub mod require_reason_for_duplicate_remaining_accounts;
pub mod require_sysvar_assert_before_sysvar_use;
pub mod require_type_assert_before_zero_copy_cast;
pub mod require_writable_before_account_resize;
pub mod require_zeroed_before_close;
