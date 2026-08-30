//! Lint-authoring macros for `pina_lints`.
//!
//! The macros mirror the `dylint_linting` API (`declare_late_lint!`,
//! `impl_late_lint!`, `declare_early_lint!`, `impl_early_lint!`,
//! `declare_pre_expansion_lint!`, and `impl_pre_expansion_lint!`) with one
//! difference: a `pina_lints` lint never registers itself. Registration is
//! centralized in [`crate::register_all_lints`], which keeps every lint
//! importable while still producing one loadable catalog.
//!
//! Each macro expands to:
//!
//! - the `extern crate` declarations the lint needs,
//! - a `rustc_session::declare_lint!` call, and
//! - the corresponding `declare_lint_pass!`/`impl_lint_pass!` call.
//!
//! The lint pass structure is named as the camel case of the lint constant
//! (`REQUIRE_OWNER_BEFORE_TOKEN_CAST` becomes
//! `RequireOwnerBeforeTokenCast`), exactly as in Dylint.

/// Declare a late lint with a unit pass structure.
///
/// See the [crate documentation](crate) for the expansion rules.
#[macro_export]
macro_rules! declare_late_lint {
	($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr) => {
		extern crate rustc_lint;
		extern crate rustc_session;

		$crate::paste::paste! {
			rustc_session::declare_lint!($(#[$attr])* $vis $NAME, $Level, $desc);
			rustc_session::declare_lint_pass!([< $NAME:camel >] => [$NAME]);
		}
	};
}

/// Declare an early lint with a unit pass structure.
///
/// See the [crate documentation](crate) for the expansion rules.
#[macro_export]
macro_rules! declare_early_lint {
	($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr) => {
		extern crate rustc_lint;
		extern crate rustc_session;

		$crate::paste::paste! {
			rustc_session::declare_lint!($(#[$attr])* $vis $NAME, $Level, $desc);
			rustc_session::declare_lint_pass!([< $NAME:camel >] => [$NAME]);
		}
	};
}

/// Declare a pre-expansion lint with a unit pass structure.
///
/// See the [crate documentation](crate) for the expansion rules.
#[macro_export]
macro_rules! declare_pre_expansion_lint {
	($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr) => {
		extern crate rustc_lint;
		extern crate rustc_session;

		$crate::paste::paste! {
			rustc_session::declare_lint!($(#[$attr])* $vis $NAME, $Level, $desc);
			rustc_session::declare_lint_pass!([< $NAME:camel >] => [$NAME]);
		}
	};
}

/// Declare a late lint with a user-defined pass structure.
///
/// The pass expression is not used by the macro; registration happens in
/// [`crate::register_all_lints`], which constructs the pass. The argument is
/// kept for API compatibility with `dylint_linting::impl_late_lint!`.
#[macro_export]
macro_rules! impl_late_lint {
	($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr, $pass:expr) => {
		extern crate rustc_lint;
		extern crate rustc_session;

		$crate::paste::paste! {
			rustc_session::declare_lint!($(#[$attr])* $vis $NAME, $Level, $desc);
			rustc_session::impl_lint_pass!([< $NAME:camel >] => [$NAME]);
		}
	};
}

/// Declare an early lint with a user-defined pass structure.
///
/// See [`impl_late_lint!`](crate::impl_late_lint) for the expansion rules.
#[macro_export]
macro_rules! impl_early_lint {
	($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr, $pass:expr) => {
		extern crate rustc_lint;
		extern crate rustc_session;

		$crate::paste::paste! {
			rustc_session::declare_lint!($(#[$attr])* $vis $NAME, $Level, $desc);
			rustc_session::impl_lint_pass!([< $NAME:camel >] => [$NAME]);
		}
	};
}

/// Declare a pre-expansion lint with a user-defined pass structure.
///
/// See [`impl_late_lint!`](crate::impl_late_lint) for the expansion rules.
#[macro_export]
macro_rules! impl_pre_expansion_lint {
	($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr, $pass:expr) => {
		extern crate rustc_lint;
		extern crate rustc_session;

		$crate::paste::paste! {
			rustc_session::declare_lint!($(#[$attr])* $vis $NAME, $Level, $desc);
			rustc_session::impl_lint_pass!([< $NAME:camel >] => [$NAME]);
		}
	};
}
