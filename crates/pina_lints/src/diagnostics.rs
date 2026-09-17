//! Lint emission plumbing.
//!
//! `rustc` removed `LintContext::lint`, the shorthand for emitting a lint with
//! no associated span through a decorator closure. Its replacement is
//! `opt_span_lint` plus a `DiagDecorator`, which spells the wrapper out at
//! every call site and buries the message a lint exists to deliver. [`emit`]
//! keeps that wrapper in one place, so each lint body still reads as the
//! diagnostic it builds.

extern crate rustc_errors;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_errors::Diag;
use rustc_errors::DiagDecorator;
use rustc_lint::Lint;
use rustc_lint::LintContext;
use rustc_span::Span;

/// Emit `lint` with no associated span, leaving `decorate` to set the spans
/// and the message.
pub fn emit<C: LintContext>(cx: &C, lint: &'static Lint, decorate: impl FnOnce(&mut Diag<'_, ()>)) {
	cx.opt_span_lint(lint, None::<Span>, DiagDecorator(decorate));
}
