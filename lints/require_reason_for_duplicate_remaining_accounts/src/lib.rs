#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_span;

use rustc_ast::Attribute;
use rustc_ast::Item;
use rustc_ast::ItemKind;
use rustc_ast::LitKind;
use rustc_ast::MetaItemKind;
use rustc_ast::VariantData;
use rustc_lint::EarlyContext;
use rustc_lint::EarlyLintPass;
use rustc_lint::LintContext;

dylint_linting::declare_pre_expansion_lint! {
	/// ### What it does
	///
	/// Requires a meaningful field doc comment when a mutable remaining-account
	/// slice explicitly permits duplicate addresses with
	/// `#[pina(remaining, distinct = false)]`.
	///
	/// ### Why is this bad?
	///
	/// Duplicate writable accounts can violate accounting and uniqueness
	/// assumptions. Pina rejects them by default, so every opt-out should state
	/// the invariant that makes aliases safe.
	///
	/// ### Example
	///
	/// ```ignore
	/// /// Duplicate addresses represent repeated votes and are counted once per entry.
	/// #[pina(remaining, distinct = false)]
	/// pub votes: &'a mut [AccountView],
	/// ```
	pub REQUIRE_REASON_FOR_DUPLICATE_REMAINING_ACCOUNTS,
	Deny,
	"duplicate mutable remaining accounts require a doc-comment justification"
}

const MIN_EXPLANATION_WORDS: usize = 5;

fn is_pina_attribute(attribute: &Attribute) -> bool {
	attribute.name().is_some_and(|name| name.as_str() == "pina")
}

fn disables_distinct_accounts(attribute: &Attribute) -> bool {
	if !is_pina_attribute(attribute) {
		return false;
	}

	attribute.meta_item_list().is_some_and(|items| {
		items.iter().any(|item| {
			let Some(meta) = item.meta_item() else {
				return false;
			};
			let is_distinct = meta
				.path
				.segments
				.last()
				.is_some_and(|segment| segment.ident.name.as_str() == "distinct");
			let is_false = matches!(
				&meta.kind,
				MetaItemKind::NameValue(value) if matches!(value.kind, LitKind::Bool(false))
			);

			is_distinct && is_false
		})
	})
}

fn has_meaningful_explanation(attributes: &[Attribute]) -> bool {
	let word_count = attributes
		.iter()
		.filter_map(Attribute::doc_str)
		.map(|doc| doc.as_str().split_whitespace().count())
		.sum::<usize>();

	word_count >= MIN_EXPLANATION_WORDS
}

fn check_fields(cx: &EarlyContext<'_>, data: &VariantData) {
	for field in data.fields() {
		let Some(attribute) = field
			.attrs
			.iter()
			.find(|attribute| disables_distinct_accounts(attribute))
		else {
			continue;
		};

		if has_meaningful_explanation(&field.attrs) {
			continue;
		}

		cx.lint(REQUIRE_REASON_FOR_DUPLICATE_REMAINING_ACCOUNTS, |diag| {
			diag.span(attribute.span);
			diag.primary_message(
				"`distinct = false` permits duplicate writable accounts without explaining why",
			);
			diag.help(
				"add a field doc comment explaining the invariant that makes duplicate addresses \
				 safe",
			);
		});
	}
}

impl EarlyLintPass for RequireReasonForDuplicateRemainingAccounts {
	fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
		if let ItemKind::Struct(_, _, data) = &item.kind {
			check_fields(cx, data);
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn ui() {
		dylint_testing::ui_test(
			env!("CARGO_PKG_NAME"),
			concat!(env!("CARGO_MANIFEST_DIR"), "/ui"),
		);
	}
}
