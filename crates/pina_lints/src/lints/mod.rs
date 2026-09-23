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
pub mod deny_colliding_account_discriminators;
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
pub mod require_guarded_full_balance_drain;
pub mod require_idl_root_to_define_one_program_id;
pub mod require_post_cpi_balance_reload;
pub mod require_program_check_before_cpi;
pub mod require_reason_for_duplicate_remaining_accounts;
pub mod require_sysvar_assert_before_sysvar_use;
pub mod require_type_assert_before_zero_copy_cast;
pub mod require_writable_before_account_resize;
pub mod require_zeroed_before_close;

/// Record tuple-destructure alias provenance into `aliases`.
///
/// The "parse once, capture several fields" idiom wraps the parse in a
/// scoped block (`let (maker, bump) = { let state = account.as_account()?; …
/// };`), so the tuple the pattern destructures is the block's tail
/// expression. Each plain binding is mapped to the identity of the element at
/// its position; non-binding patterns (`_`, nested tuples) contribute
/// nothing, and a non-tuple initializer records nothing (the pattern cannot
/// have destructured it, so no alias would be sound).
///
/// This runs only inside the lint driver, the rustc-glue layer `codecov.yml`
/// classifies: the UI fixtures in
/// `tests/ui/require_canonical_bump_before_pda_write/` exercise both the
/// plain-binding and skipped-element shapes byte for byte.
pub(crate) fn record_tuple_pattern_aliases(
	initializer: &Expr<'_>,
	pattern: &rustc_hir::Pat<'_>,
	aliases: &mut std::collections::HashMap<rustc_hir::HirId, crate::shared::AliasInfo>,
) {
	let rustc_hir::PatKind::Tuple(pat_elements, _) = pattern.kind else {
		return;
	};
	let mut tail = initializer;
	while let ExprKind::Block(block, _) = tail.kind {
		match block.expr {
			Some(next) => tail = next,
			None => return,
		}
	}
	let ExprKind::Tup(init_elements) = tail.kind else {
		return;
	};
	for (pattern, element) in pat_elements.iter().zip(init_elements) {
		let rustc_hir::PatKind::Binding(_, binding, ..) = pattern.kind else {
			continue;
		};
		if let Some(identity) = crate::shared::expression_identity(element) {
			aliases.insert(
				binding,
				crate::shared::AliasInfo {
					identity,
					binding: crate::shared::expression_local_binding(element),
				},
			);
		}
	}
}

extern crate rustc_hir;

use rustc_hir::Expr;
use rustc_hir::ExprKind;
