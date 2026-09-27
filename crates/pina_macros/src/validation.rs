//! Parsing and code generation for value validation annotations.
//!
//! A rule is a comparison over the field's `value` or its `len`, so the
//! annotation reads as the check it generates:
//!
//! ```ignore
//! #[pina(validate(value >= 1 && value <= 10, len <= 64, value != 0))]
//! ```
//!
//! `error = ERROR` overrides the failure every rule in the group raises. The
//! `min`/`max`/`min_len`/`max_len`/`exact_len` parameter spellings predate
//! comparisons; they still parse and generate the identical checks, but they
//! are deprecated and warn at the parameter the author wrote.
//!
//! The grammar is parsed by hand because neither `syn` nor `darling` can read
//! an operator where a named parameter's `=` belongs: `ParseNestedMeta::value`
//! consumes one `=` token, so for `a == b` it takes half the pair and then
//! fails to parse the `= b` that remains.

use darling::Error as DarlingError;
use proc_macro2::Spacing;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
#[cfg(feature = "validation")]
use quote::quote;
use quote::quote_spanned;
use syn::Expr;
use syn::Fields;
use syn::Ident;
use syn::ItemStruct;
use syn::Token;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;

use crate::args::ValidationHook;

/// The validated schema fields of one struct, with any deprecations to warn
/// about.
#[derive(Debug)]
pub(crate) struct SchemaValidations {
	pub(crate) fields: Vec<(Ident, Type, Vec<ValueGroup>)>,
	pub(crate) deprecations: Vec<Deprecation>,
}

/// One `#[pina(validate(...))]` group on a schema field.
#[derive(Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
pub(crate) struct ValueGroup {
	items: Vec<ValueItem>,
	deprecations: Vec<Deprecation>,
}

impl ValueGroup {
	/// The `error = ERROR` override the group's rules raise, if any.
	#[cfg_attr(not(feature = "validation"), allow(dead_code))]
	fn error_override(&self) -> Option<(&Expr, &Span)> {
		self.items.iter().find_map(|item| {
			match item {
				ValueItem::Error(expr, span) => Some((expr, span)),
				ValueItem::Comparison(_) | ValueItem::Legacy(_) => None,
			}
		})
	}
}

/// One rule inside a `validate(...)` group.
#[derive(Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
enum ValueItem {
	/// `value >= 1`, `len == 4`, `100 < value <= u64::MAX`.
	Comparison(ComparisonRule),
	/// A deprecated named bound (`min = 1`) checking what its name says.
	Legacy(LegacyRule),
	/// `error = ERROR`, overriding the failure the group's rules raise.
	Error(Expr, Span),
}

/// The side of a comparison a rule names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueReceiver {
	/// The field's value; only integer fields support value comparisons.
	Value,
	/// The field's length; only bounded strings, vectors, and arrays have one.
	Len,
}

/// The comparison a rule checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComparisonKind {
	Eq,
	Ne,
	Lt,
	Le,
	Gt,
	Ge,
}

/// A comparison operator, with the tokens the author wrote.
#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
struct ComparisonOperator {
	kind: ComparisonKind,
	tokens: TokenStream,
}

impl ComparisonOperator {
	/// How many tokens the operator spans.
	fn width(&self) -> usize {
		match self.kind {
			ComparisonKind::Lt | ComparisonKind::Gt => 1,
			ComparisonKind::Eq | ComparisonKind::Ne | ComparisonKind::Le | ComparisonKind::Ge => 2,
		}
	}

	/// Recognize an operator at this position.
	///
	/// `None` means the token begins a bound operand instead. A punct that
	/// looks like a mistyped operator is an error, so `value = 2` is reported
	/// rather than silently parsed as something else.
	fn at(trees: &[TokenTree], index: usize) -> syn::Result<Option<Self>> {
		let TokenTree::Punct(first) = &trees[index] else {
			return Ok(None);
		};
		let follows = |ch: char| {
			matches!(
				trees.get(index + 1),
				Some(TokenTree::Punct(punct)) if punct.as_char() == ch
			)
		};

		let (kind, width) = match first.as_char() {
			'=' if follows('=') => (ComparisonKind::Eq, 2),
			'=' => {
				return Err(syn::Error::new(
					first.span(),
					"comparison rules use `==`: write `value == 2`",
				));
			}

			'!' if follows('=') => (ComparisonKind::Ne, 2),
			'<' if follows('=') => (ComparisonKind::Le, 2),
			'<' if first.spacing() == Spacing::Joint => {
				return Err(syn::Error::new(
					first.span(),
					"unexpected `<`; wrap a bound containing generics or shifts in parentheses",
				));
			}
			'<' => (ComparisonKind::Lt, 1),
			'>' if follows('=') => (ComparisonKind::Ge, 2),
			'>' if first.spacing() == Spacing::Joint => {
				return Err(syn::Error::new(
					first.span(),
					"unexpected `>`; wrap a bound containing shifts or arrows in parentheses",
				));
			}
			'>' => (ComparisonKind::Gt, 1),
			_ => return Ok(None),
		};

		Ok(Some(Self {
			kind,
			tokens: trees[index..index + width].iter().cloned().collect(),
		}))
	}
}

/// One side of a comparison: the field, or a literal bound.
#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
enum Operand {
	Receiver,
	Bound(TokenStream),
}

/// One `left op right` comparison.
#[derive(Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
struct ComparisonCheck {
	left: Operand,
	operator: ComparisonOperator,
	right: Operand,
}

/// A comparison over the field's `value` or `len`, held as the checks that
/// must all hold.
///
/// A single bound is one check; a chain is two, because Rust has no chained
/// comparison to generate. `100 < value <= u64::MAX` is `100 < value` and
/// `value <= u64::MAX`, which is what `rustc` suggests when handed the chain
/// verbatim.
#[derive(Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
struct ComparisonRule {
	receiver: ValueReceiver,
	checks: Vec<ComparisonCheck>,
	span: Span,
}

/// A deprecated named bound, checking what its name says.
#[derive(Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
struct LegacyRule {
	param: LegacyParam,
	bound: Expr,
}

#[derive(Clone, Copy, Debug)]
enum LegacyParam {
	Min,
	Max,
	MinLen,
	MaxLen,
	ExactLen,
}

impl LegacyParam {
	/// The parameter name as the author wrote it.
	fn name(self) -> &'static str {
		match self {
			LegacyParam::Min => "min",
			LegacyParam::Max => "max",
			LegacyParam::MinLen => "min_len",
			LegacyParam::MaxLen => "max_len",
			LegacyParam::ExactLen => "exact_len",
		}
	}

	/// The receiver the parameter's bound applies to.
	fn receiver(self) -> ValueReceiver {
		match self {
			LegacyParam::Min | LegacyParam::Max => ValueReceiver::Value,
			LegacyParam::MinLen | LegacyParam::MaxLen | LegacyParam::ExactLen => ValueReceiver::Len,
		}
	}

	fn from_ident(ident: &Ident) -> Option<Self> {
		match ident.to_string().as_str() {
			"min" => Some(LegacyParam::Min),
			"max" => Some(LegacyParam::Max),
			"min_len" => Some(LegacyParam::MinLen),
			"max_len" => Some(LegacyParam::MaxLen),
			"exact_len" => Some(LegacyParam::ExactLen),
			_ => None,
		}
	}
}

/// A named parameter a value rule accepts: a deprecated bound or the `error`
/// override. Comparisons name a receiver instead, and never reach here.
enum NamedParameter {
	Bound(LegacyParam),
	Error,
}

impl NamedParameter {
	fn from_ident(ident: &Ident) -> Option<Self> {
		if ident == "error" {
			return Some(Self::Error);
		}

		LegacyParam::from_ident(ident).map(Self::Bound)
	}
}

/// A deprecated parameter, with the diagnostic the author sees.
#[derive(Debug)]
pub(crate) struct Deprecation {
	span: Span,
	note: &'static str,
}

/// Validation target generated by `PinaPod` for a schema.
#[derive(Clone, Copy)]
#[cfg(feature = "validation")]
pub(crate) enum ValueTarget<'a> {
	Fixed(&'a Ident),
	Compact {
		target: &'a Ident,
		tails: &'a [crate::schema::CompactTail],
	},
}

/// Extract, validate, and remove schema-field helper attributes.
pub(crate) fn take_value_validations(item: &mut ItemStruct) -> syn::Result<SchemaValidations> {
	let Fields::Named(fields) = &mut item.fields else {
		// The schema validator owns the diagnostic for unsupported struct
		// shapes. Returning no field rules here preserves that more specific
		// error instead of reporting a validation error for a tuple or unit
		// struct that did not request validation.
		return Ok(SchemaValidations {
			fields: Vec::new(),
			deprecations: Vec::new(),
		});
	};

	let mut rules = Vec::with_capacity(fields.named.len());
	let mut deprecations = Vec::new();
	let mut parse_error = None;

	for field in &mut fields.named {
		// `Fields::Named` guarantees an identifier on every field.
		let ident = field
			.ident
			.clone()
			.expect("internal error: named field without an ident");

		let mut groups = Vec::new();
		field.attrs.retain_mut(|attribute| {
			if !attribute.path().is_ident("pina") {
				return true;
			}
			match attribute.parse_args_with(FieldValidations::parse) {
				Ok(mut parsed) => {
					deprecations.append(&mut parsed.take_deprecations());
					groups.append(&mut parsed.groups);
					false
				}
				Err(error) => {
					parse_error.get_or_insert(invalid_annotation(&error));
					false
				}
			}
		});

		if let Some(error) = parse_error {
			return Err(error);
		}

		rules.push((ident, field.ty.clone(), groups));
	}

	let validations = SchemaValidations {
		fields: rules,
		deprecations,
	};
	validate_rules(&validations)?;
	Ok(validations)
}

impl Parse for ValueGroup {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut items = Vec::new();
		let mut deprecations = Vec::new();

		if input.is_empty() {
			return Err(input.error(
				"empty `validate(...)` annotation; add a comparison such as `value >= 1` or `len \
				 == 4`, or remove the annotation",
			));
		}

		loop {
			let (item, deprecation) = parse_value_item(input)?;
			if let Some(deprecation) = deprecation {
				deprecations.push(deprecation);
			}
			items.push(item);

			if input.is_empty() {
				break;
			}
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			} else if input.peek(Token![&&]) {
				input.parse::<Token![&&]>()?;
			} else {
				return Err(input.error("expected `,` or `&&` between validation rules"));
			}
			if input.is_empty() {
				break;
			}
		}

		Ok(ValueGroup {
			items,
			deprecations,
		})
	}
}

/// The `validate(...)` option of a schema field, one per `#[pina(...)]`
/// attribute and several allowed per attribute.
struct FieldValidations {
	groups: Vec<ValueGroup>,
}

impl FieldValidations {
	fn take_deprecations(&mut self) -> Vec<Deprecation> {
		self.groups
			.iter_mut()
			.flat_map(|group| core::mem::take(&mut group.deprecations))
			.collect()
	}
}

impl Parse for FieldValidations {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut groups = Vec::new();
		while !input.is_empty() {
			let name: Ident = input.parse()?;
			if name != "validate" {
				return Err(syn::Error::new(
					name.span(),
					format!(
						"unknown `#[pina]` option `{name}` on a schema field; expected \
						 `validate(...)`"
					),
				));
			}
			let content;
			syn::parenthesized!(content in input);
			groups.push(content.parse::<ValueGroup>()?);
			if input.is_empty() {
				break;
			}
			input.parse::<Token![,]>()?;
		}

		Ok(Self { groups })
	}
}

/// Parse one comma- or `&&`-separated item.
fn parse_value_item(input: ParseStream) -> syn::Result<(ValueItem, Option<Deprecation>)> {
	// A named parameter is an identifier followed by a lone `=`. A `==` starts
	// a comparison instead, so the spacing has to disambiguate.
	if let Some(parameter) = peek_named_parameter(input)? {
		let ident: Ident = input.parse()?;
		input.parse::<Token![=]>()?;
		let bound: Expr = input.parse()?;
		let span = ident.span();

		return Ok(match parameter {
			NamedParameter::Error => (ValueItem::Error(bound, span), None),
			NamedParameter::Bound(param) => deprecated_bound(span, param, bound),
		});
	}

	Ok((ValueItem::Comparison(parse_comparison(input)?), None))
}

/// The named parameter at the front of `input`, when the `=` that follows is a
/// single token rather than half of a comparison's `==`.
fn peek_named_parameter(input: ParseStream<'_>) -> syn::Result<Option<NamedParameter>> {
	let fork = input.fork();
	let Ok(ident) = fork.parse::<Ident>() else {
		return Ok(None);
	};
	let Some(parameter) = NamedParameter::from_ident(&ident) else {
		return Ok(None);
	};

	fork.step(|cursor| {
		let lone = matches!(
			cursor.punct(),
			Some((punct, _)) if punct.as_char() == '=' && punct.spacing() == Spacing::Alone,
		);
		Ok((lone, *cursor))
	})
	.map(|lone| lone.then_some(parameter))
}

fn deprecated_bound(
	span: Span,
	param: LegacyParam,
	bound: Expr,
) -> (ValueItem, Option<Deprecation>) {
	let note = match param {
		LegacyParam::Min => {
			"`min` is deprecated; write the bound as a comparison, as in `value >= 1`"
		}
		LegacyParam::Max => {
			"`max` is deprecated; write the bound as a comparison, as in `value <= 10`"
		}
		LegacyParam::MinLen => {
			"`min_len` is deprecated; write the bound as a comparison, as in `len >= 2`"
		}
		LegacyParam::MaxLen => {
			"`max_len` is deprecated; write the bound as a comparison, as in `len <= 64`"
		}
		LegacyParam::ExactLen => {
			"`exact_len` is deprecated; write the bound as a comparison, as in `len == 4`"
		}
	};

	(
		ValueItem::Legacy(LegacyRule { param, bound }),
		Some(Deprecation { span, note }),
	)
}

/// Parse one comparison rule, operator chain and all.
fn parse_comparison(input: ParseStream) -> syn::Result<ComparisonRule> {
	let span = input.span();
	let mut trees = Vec::new();
	while !input.is_empty() && !input.peek(Token![,]) && !input.peek(Token![&&]) {
		trees.push(input.parse::<TokenTree>()?);
	}

	ComparisonRule::from_tokens(span, &trees)
}

impl ComparisonRule {
	/// Build a rule from the tokens of one comparison, splitting at operators.
	fn from_tokens(span: Span, trees: &[TokenTree]) -> syn::Result<Self> {
		// The tokens split into operands and the operators between them, so
		// `n` operators require exactly `n + 1` operands.
		let mut operands: Vec<TokenStream> = Vec::new();
		let mut operators: Vec<ComparisonOperator> = Vec::new();
		let mut current = TokenStream::new();

		let mut index = 0;
		while index < trees.len() {
			if let Some(operator) = ComparisonOperator::at(trees, index)? {
				index += operator.width();
				operands.push(core::mem::take(&mut current));
				operators.push(operator);
			} else {
				current.extend([trees[index].clone()]);
				index += 1;
			}
		}
		operands.push(current);

		if operators.is_empty() {
			return Err(syn::Error::new(
				span,
				"expected a comparison operator (`==`, `!=`, `<`, `<=`, `>`, `>=`) in the rule; \
				 rules look like `value >= 1` or `len == 4`",
			));
		}
		// One operator is a single bound; two are one range, written either as
		// `1 <= value <= 10` or with the operator on one side.
		if operators.len() > 2 {
			return Err(syn::Error::new(
				span,
				"a comparison rule checks at most one range; separate extra comparisons with `,`",
			));
		}
		if operands.iter().any(TokenStream::is_empty) {
			return Err(syn::Error::new(
				span,
				"expected a bound on the other side of the comparison operator",
			));
		}

		// Exactly one operand names the field, and a chain puts it in the
		// middle: `4 < len <= 100`.
		let mut receiver = None;
		let mut receiver_index = None;
		for (index, operand) in operands.iter().enumerate() {
			let Some(found) = receiver_from_tokens(operand) else {
				continue;
			};
			if receiver.replace(found).is_some() {
				return Err(syn::Error::new(
					span,
					"compare `value` or `len` against a bound, not against each other",
				));
			}
			receiver_index = Some(index);
		}
		let Some(receiver) = receiver else {
			return Err(syn::Error::new(
				span,
				"every comparison must mention the field's `value` or its `len`",
			));
		};
		let receiver_index =
			receiver_index.unwrap_or_else(|| unreachable!("recorded with the receiver"));
		if operators.len() == 2 && receiver_index != 1 {
			return Err(syn::Error::new(
				span,
				"the middle of a chained comparison must be the field's `value` or its `len`",
			));
		}
		// An equality is a single fact, not a range to chain.
		if operators.len() == 2
			&& operators
				.iter()
				.any(|operator| matches!(operator.kind, ComparisonKind::Eq | ComparisonKind::Ne))
		{
			return Err(syn::Error::new(
				span,
				"an `==` or `!=` rule takes a single bound; chain only `<`, `<=`, `>`, or `>=`",
			));
		}

		// Every operand carries tokens (checked above), and the one naming the
		// field is replaced by the receiver.
		let mut operands: Vec<Operand> = operands.into_iter().map(Operand::Bound).collect();
		operands[receiver_index] = Operand::Receiver;

		// Each operator pairs the operand before it with the one after it. A
		// chained range therefore becomes two checks that share the field.
		let checks = operators
			.into_iter()
			.enumerate()
			.map(|(index, operator)| {
				ComparisonCheck {
					left: operands[index].clone(),
					operator,
					right: operands[index + 1].clone(),
				}
			})
			.collect();

		Ok(Self {
			receiver,
			checks,
			span,
		})
	}
}

/// The receiver a single-ident bound names, if it is one.
fn receiver_from_tokens(tokens: &TokenStream) -> Option<ValueReceiver> {
	let trees: Vec<_> = tokens.clone().into_iter().collect();
	let [TokenTree::Ident(ident)] = trees.as_slice() else {
		return None;
	};
	match ident.to_string().as_str() {
		"value" => Some(ValueReceiver::Value),
		"len" => Some(ValueReceiver::Len),
		_ => None,
	}
}

/// Reject rules the field's type cannot honor, and duplicates that would make
/// the raised failure ambiguous.
fn validate_rules(validations: &SchemaValidations) -> syn::Result<()> {
	for (field, ty, groups) in &validations.fields {
		let integer = is_integer(ty);
		let sized = has_length(ty);
		let mut seen = LegacySeen::default();

		for group in groups {
			let mut saw_error = false;
			for item in &group.items {
				match item {
					ValueItem::Error(_, span) => {
						if saw_error {
							return Err(syn::Error::new(
								*span,
								"duplicate `error` override in `validate(...)`; keep one so the \
								 raised failure is unambiguous",
							));
						}
						saw_error = true;
					}
					ValueItem::Legacy(rule) => {
						// The deprecated spellings keep the diagnostics they
						// have always raised.
						match rule.param.receiver() {
							ValueReceiver::Value if !integer => {
								return Err(syn::Error::new_spanned(
									ty,
									format!(
										"`min` and `max` on field `{field}` require an integer \
										 field; supported types are the fixed-width signed and \
										 unsigned Rust integers and Pina `Pod*` integer types"
									),
								));
							}
							ValueReceiver::Len if !sized => {
								return Err(syn::Error::new_spanned(
									ty,
									format!(
										"length validation on field `{field}` requires \
										 `String<N>`, `PodString<N, PFX>`, `Vec<T, N>`, \
										 `PodVec<T, N, PFX>`, or `[u8; N]`"
									),
								));
							}
							_ => {}
						}

						seen.record(field, rule)?;
					}
					ValueItem::Comparison(rule) => {
						match rule.receiver {
							ValueReceiver::Value if !integer => {
								return Err(syn::Error::new(
									rule.span,
									format!(
										"comparisons on field `{field}` require an integer field; \
										 supported types are the fixed-width signed and unsigned \
										 Rust integers and Pina `Pod*` integer types"
									),
								));
							}
							ValueReceiver::Len if !sized => {
								return Err(syn::Error::new(
									rule.span,
									format!(
										"length comparisons on field `{field}` require \
										 `String<N>`, `PodString<N, PFX>`, `Vec<T, N>`, \
										 `PodVec<T, N, PFX>`, or `[u8; N]`"
									),
								));
							}
							_ => {}
						}
					}
				}
			}
		}

		if seen.exact_len && (seen.min_len || seen.max_len) {
			return Err(syn::Error::new_spanned(
				field,
				format!(
					"`exact_len` on field `{field}` cannot be combined with `min_len` or \
					 `max_len`, even across separate `validate(...)` annotations; use only \
					 `exact_len` for an exact-size rule"
				),
			));
		}
	}

	Ok(())
}

/// The deprecated named bounds a single field declared.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct LegacySeen {
	min: bool,
	max: bool,
	min_len: bool,
	max_len: bool,
	exact_len: bool,
}

impl LegacySeen {
	fn record(&mut self, field: &Ident, rule: &LegacyRule) -> syn::Result<()> {
		let slot = match rule.param {
			LegacyParam::Min => &mut self.min,
			LegacyParam::Max => &mut self.max,
			LegacyParam::MinLen => &mut self.min_len,
			LegacyParam::MaxLen => &mut self.max_len,
			LegacyParam::ExactLen => &mut self.exact_len,
		};

		if *slot {
			return Err(syn::Error::new_spanned(
				field,
				format!(
					"duplicate `{}` validation on field `{field}`; keep one rule so validation \
					 order and its error are unambiguous",
					rule.param.name()
				),
			));
		}

		*slot = true;
		Ok(())
	}
}

/// Emit the feature-gate diagnostic before generated code can mention a
/// missing runtime trait.
#[cfg(not(feature = "validation"))]
pub(crate) fn feature_error(item: &ItemStruct) -> TokenStream {
	syn::Error::new_spanned(
		&item.ident,
		"validation annotations require Pina's `validation` feature; enable it with `pina = { \
		 version = \"...\", features = [\"validation\"] }` (or enable `pina_macros/validation` \
		 when using the proc-macro crate directly)",
	)
	.to_compile_error()
}

/// Whether any field or type-level application validator was declared.
#[cfg(not(feature = "validation"))]
pub(crate) fn validation_requested(
	fields: &[(Ident, Type, Vec<ValueGroup>)],
	hook: Option<&ValidationHook>,
) -> bool {
	hook.is_some() || fields.iter().any(|(_, _, groups)| !groups.is_empty())
}

/// Generate the `PinaValidate` implementation for a fixed or compact view.
#[cfg(feature = "validation")]
pub(crate) fn generate_value_validation(
	crate_path: &syn::Path,
	target: ValueTarget<'_>,
	fields: &[(Ident, Type, Vec<ValueGroup>)],
	hook: Option<&ValidationHook>,
	default_error: &TokenStream,
) -> TokenStream {
	let mut checks = Vec::new();
	for (field, ty, groups) in fields {
		for group in groups {
			let error = group.error_override().map_or_else(
				|| quote!(#default_error),
				|(expr, _)| quote!((#expr).into()),
			);
			for item in &group.items {
				let check = match item {
					ValueItem::Comparison(rule) => {
						generate_comparison(field, ty, target, rule, &error)
					}
					ValueItem::Legacy(rule) => generate_legacy(field, ty, target, rule, &error),
					ValueItem::Error(..) => continue,
				};
				checks.push(check);
			}
		}
	}
	let hook = hook.map(|hook| {
		let with = &hook.with;

		quote! { #with(self)?; }
	});

	match target {
		ValueTarget::Fixed(target) => {
			quote! {
				impl #crate_path::PinaValidate for #target {
					#[inline]
					fn validate(&self) -> #crate_path::ProgramResult {
						#(#checks)*
						#hook

						Ok(())
					}
				}
			}
		}
		ValueTarget::Compact { target, .. } => {
			quote! {
				impl<'__pina_validation> #crate_path::PinaValidate
					for #target<'__pina_validation>
				{
					#[inline]
					fn validate(&self) -> #crate_path::ProgramResult {
						#(#checks)*
						#hook

						Ok(())
					}
				}
			}
		}
	}
}

/// Generate one deprecated named bound, with the check shape it always had.
#[cfg(feature = "validation")]
fn generate_legacy(
	field: &Ident,
	ty: &Type,
	target: ValueTarget<'_>,
	rule: &LegacyRule,
	error: &TokenStream,
) -> TokenStream {
	let bound = &rule.bound;
	match rule.param {
		LegacyParam::Min => {
			let value = value_expression(field, ty, target);
			quote! { if #value < (#bound) { return Err(#error); } }
		}
		LegacyParam::Max => {
			let value = value_expression(field, ty, target);
			quote! { if #value > (#bound) { return Err(#error); } }
		}
		LegacyParam::MinLen => {
			let len = length_expression(field, target);
			quote! { if #len < (#bound) { return Err(#error); } }
		}
		LegacyParam::MaxLen => {
			let len = length_expression(field, target);
			quote! { if #len > (#bound) { return Err(#error); } }
		}
		LegacyParam::ExactLen => {
			let len = length_expression(field, target);
			quote! { if #len != (#bound) { return Err(#error); } }
		}
	}
}

/// Generate one comparison, negated so a violation raises the group's error.
///
/// A chain is emitted as `&&`-joined pairs rather than as the single
/// `100 < value <= u64::MAX` the author wrote: Rust has no chained comparison,
/// so `(100) < (value) <= (u64::MAX)` is `comparison operators cannot be
/// chained`. Each operator re-states the receiver, which the generated code
/// reads once per pair.
#[cfg(feature = "validation")]
fn generate_comparison(
	field: &Ident,
	ty: &Type,
	target: ValueTarget<'_>,
	rule: &ComparisonRule,
	error: &TokenStream,
) -> TokenStream {
	let receiver = match rule.receiver {
		ValueReceiver::Value => value_expression(field, ty, target),
		ValueReceiver::Len => length_expression(field, target),
	};

	// Each check stands alone, joined with `&&`: Rust has no chained
	// comparison, so a range renders as its two pairwise checks.
	let mut condition = TokenStream::new();
	for (index, check) in rule.checks.iter().enumerate() {
		let left = match &check.left {
			Operand::Receiver => quote!((#receiver)),
			Operand::Bound(bound) => quote!((#bound)),
		};
		let right = match &check.right {
			Operand::Receiver => quote!((#receiver)),
			Operand::Bound(bound) => quote!((#bound)),
		};
		let tokens = &check.operator.tokens;

		if index > 0 {
			condition.extend(quote!(&&));
		}
		condition.extend(quote!((#left #tokens #right)));
	}

	quote! {
		if !(#condition) {
			return Err(#error);
		}
	}
}

/// The expression a `value` comparison reads.
#[cfg(feature = "validation")]
fn value_expression(field: &Ident, ty: &Type, target: ValueTarget<'_>) -> TokenStream {
	let name = last_type_name(ty).unwrap_or_default();
	let is_pod = name.starts_with("PodU") || name.starts_with("PodI");

	match target {
		ValueTarget::Fixed(_) if is_pod => quote!(self.#field().get()),
		ValueTarget::Fixed(_) => quote!(self.#field()),
		ValueTarget::Compact { .. } if matches!(name.as_str(), "u8" | "i8") => {
			quote!(self.#field)
		}
		ValueTarget::Compact { .. } => quote!(self.#field.get()),
	}
}

/// The expression a `len` comparison reads.
#[cfg(feature = "validation")]
fn length_expression(field: &Ident, target: ValueTarget<'_>) -> TokenStream {
	match target {
		ValueTarget::Fixed(_) => quote!(self.#field().len()),
		ValueTarget::Compact { tails, .. } if tails.iter().any(|tail| tail.name == *field) => {
			quote!(self.#field().len())
		}
		ValueTarget::Compact { .. } => quote!(self.#field.len()),
	}
}

fn last_type_name(ty: &Type) -> Option<String> {
	let Type::Path(path) = ty else {
		return None;
	};

	path.path
		.segments
		.last()
		.map(|segment| segment.ident.to_string())
}

fn is_integer(ty: &Type) -> bool {
	matches!(
		last_type_name(ty).as_deref(),
		Some(
			"u8" | "u16"
				| "u32" | "u64"
				| "u128" | "i8"
				| "i16" | "i32"
				| "i64" | "i128"
				| "PodU16" | "PodU32"
				| "PodU64" | "PodU128"
				| "PodI16" | "PodI32"
				| "PodI64" | "PodI128"
		)
	)
}

fn has_length(ty: &Type) -> bool {
	if matches!(ty, Type::Array(_)) {
		return true;
	}

	matches!(
		last_type_name(ty).as_deref(),
		Some("String" | "PodString" | "Vec" | "PodVec")
	)
}

/// Add the macro name and complete validation vocabulary to a Darling error.
pub(crate) fn attribute_error(error: &DarlingError, macro_name: &str) -> TokenStream {
	let reason = darling_reason(error);
	let outer_arguments = if macro_name == "account" {
		"`discriminator = path`, `variant = name`, `crate = path`, `compact`, and `validate(with = \
		 function)`"
	} else {
		"`discriminator = path`, `variant = name`, `crate = path`, and `validate(with = function)`"
	};

	syn::Error::new(
		error.span(),
		format!(
			"invalid `#[{macro_name}(...)]` arguments: {reason} Supported outer arguments are \
			 {outer_arguments}. Supported field validation syntax is `#[pina(validate(value >= \
			 MIN && value <= MAX, len <= N, error = ERROR))]`"
		),
	)
	.to_compile_error()
}

/// Format a Darling diagnostic as one sentence before adding corrective help.
pub(crate) fn darling_reason(error: &DarlingError) -> String {
	let mut reason = error.to_string();
	if !reason.ends_with(['.', '?', '!']) {
		reason.push('.');
	}

	reason
}

/// Emit one `deprecated` warning per legacy parameter, spanned at the
/// parameter the author wrote.
pub(crate) fn deprecation_warnings(deprecations: &[Deprecation]) -> TokenStream {
	deprecations
		.iter()
		.map(|deprecation| {
			let note = deprecation.note;
			quote_spanned! {deprecation.span=>
				const _: () = {
					#[deprecated(note = #note)]
					const PINA_DEPRECATED_VALIDATION_BOUND: () = ();
					const _: () = PINA_DEPRECATED_VALIDATION_BOUND;
				};
			}
		})
		.collect()
}

/// Wrap a parse diagnostic in the grammar the annotation supports.
fn invalid_annotation(error: &syn::Error) -> syn::Error {
	syn::Error::new(
		error.span(),
		format!(
			"invalid Pina validation annotation: {error} Rules compare the field's `value` or its \
			 `len`, as in `value >= 1`, `len <= 8`, or `100 < value <= u64::MAX`; `error = ERROR` \
			 overrides the failure the group raises"
		),
	)
}

#[cfg(test)]
mod tests {
	use quote::ToTokens as _;

	use super::*;

	/// Parse the inside of one `validate(...)` group.
	fn group(source: &str) -> ValueGroup {
		syn::parse_str(source).unwrap()
	}

	/// Render a group's rules compactly so structure is assertable.
	fn summarize(group: &ValueGroup) -> String {
		group
			.items
			.iter()
			.map(|item| {
				match item {
					ValueItem::Comparison(rule) => {
						let receiver = match rule.receiver {
							ValueReceiver::Value => "value",
							ValueReceiver::Len => "len",
						};
						let operand = |operand: &Operand| {
							match operand {
								Operand::Receiver => receiver.to_owned(),
								Operand::Bound(bound) => bound.to_string().replace(' ', ""),
							}
						};
						let checks = rule
							.checks
							.iter()
							.map(|check| {
								format!(
									"{} {:?} {}",
									operand(&check.left),
									check.operator.kind,
									operand(&check.right),
								)
							})
							.collect::<Vec<_>>()
							.join(" && ");

						format!("{:?}[{checks}]", rule.receiver)
					}
					ValueItem::Legacy(rule) => {
						format!("legacy{:?}[{}]", rule.param, rule.bound.to_token_stream())
					}
					ValueItem::Error(..) => "error".to_owned(),
				}
			})
			.collect::<Vec<_>>()
			.join("|")
	}

	#[test]
	fn every_operator_is_parsed_for_both_receivers() {
		let parsed = group(
			"value == 1, value != 2, value < 3, value <= 4, value > 5, value >= 6, len == 1, len \
			 != 2, len < 3, len <= 4, len > 5, len >= 6",
		);

		assert_eq!(parsed.items.len(), 12);
		assert_eq!(
			summarize(&parsed),
			"Value[value Eq 1]|Value[value Ne 2]|Value[value Lt 3]|Value[value Le 4]|Value[value \
			 Gt 5]|Value[value Ge 6]|Len[len Eq 1]|Len[len Ne 2]|Len[len Lt 3]|Len[len Le \
			 4]|Len[len Gt 5]|Len[len Ge 6]",
		);
	}

	#[test]
	fn a_chain_keeps_the_receiver_in_the_middle() {
		let parsed = group("100 < value <= u64::MAX");

		assert_eq!(parsed.items.len(), 1);
		// The range is held as two checks, because Rust cannot chain them.
		assert_eq!(
			summarize(&parsed),
			"Value[100 Lt value && value Le u64::MAX]"
		);
	}

	#[test]
	fn both_separators_accept_rules() {
		let comma = group("value >= 1, value <= 10");
		let and_and = group("value >= 1 && value <= 10");

		assert_eq!(summarize(&comma), summarize(&and_and));
	}

	#[test]
	fn the_error_override_is_group_wide() {
		let parsed = group("error = ProgramError::Denied, value >= 1");

		assert!(
			parsed
				.items
				.iter()
				.any(|item| matches!(item, ValueItem::Error(..)))
		);
	}

	#[test]
	fn legacy_bounds_are_deprecated_and_kept() {
		let parsed = group("min = 1, max = 10, min_len = 2, max_len = 64, exact_len = 4");

		assert_eq!(parsed.deprecations.len(), 5);
		assert_eq!(
			summarize(&parsed),
			"legacyMin[1]|legacyMax[10]|legacyMinLen[2]|legacyMaxLen[64]|legacyExactLen[4]",
		);
	}

	#[test]
	fn a_term_without_an_operator_is_rejected() {
		let error = syn::parse_str::<ValueGroup>("value").expect_err("no operator");

		assert!(error.to_string().contains("expected a comparison operator"));
	}

	#[test]
	fn an_unknown_receiver_is_rejected() {
		let error = syn::parse_str::<ValueGroup>("count == 2").expect_err("unknown receiver");

		assert!(
			error
				.to_string()
				.contains("must mention the field's `value` or its `len`"),
			"{error}"
		);
	}

	#[test]
	fn a_chain_needs_its_receiver_in_the_middle() {
		let error = syn::parse_str::<ValueGroup>("value >= 1 < 2").expect_err("receiver first");

		assert!(
			error
				.to_string()
				.contains("the middle of a chained comparison"),
			"{error}"
		);
	}

	#[test]
	fn two_receivers_in_one_rule_are_rejected() {
		let error = syn::parse_str::<ValueGroup>("value >= len").expect_err("two receivers");

		assert!(
			error.to_string().contains("not against each other"),
			"{error}"
		);
	}

	#[test]
	fn an_equality_takes_a_single_bound() {
		let error = syn::parse_str::<ValueGroup>("4 == len == 4").expect_err("chained equality");

		assert!(
			error.to_string().contains("takes a single bound"),
			"{error}"
		);
	}

	#[test]
	fn a_lone_equals_is_a_hinted_error() {
		let error = syn::parse_str::<ValueGroup>("value = 2").expect_err("single equals");

		assert!(
			error.to_string().contains("comparison rules use `==`"),
			"{error}"
		);
	}

	#[test]
	fn deprecation_warnings_are_const_blocks_spanned_at_the_parameter() {
		let parsed = group("min = 1");
		let rendered = deprecation_warnings(&parsed.deprecations)
			.to_string()
			.replace(' ', "");

		assert!(
			rendered.contains("const_:()={#[deprecated(note="),
			"each warning is an anonymous const block: {rendered}"
		);
		assert!(
			rendered.contains("PINA_DEPRECATED_VALIDATION_BOUND"),
			"the block must read the deprecated constant: {rendered}"
		);
	}

	#[test]
	fn value_comparisons_require_an_integer_field() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(value >= 1))]
				enabled: bool,
			}
		};
		let error = take_value_validations(&mut item).expect_err("bool must reject value rules");

		assert!(
			error.to_string().contains("require an integer field"),
			"{error}"
		);
	}

	#[test]
	fn len_comparisons_require_a_sized_field() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(len <= 4))]
				value: u64,
			}
		};
		let error = take_value_validations(&mut item).expect_err("u64 must reject len rules");

		assert!(
			error.to_string().contains("length comparisons on field"),
			"{error}"
		);
	}

	#[test]
	fn duplicate_error_overrides_are_rejected() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(value >= 1, error = A, error = B))]
				value: u64,
			}
		};
		let error = take_value_validations(&mut item).expect_err("two error overrides");

		assert!(
			error.to_string().contains("duplicate `error` override"),
			"{error}"
		);
	}

	/// The legacy spellings keep the diagnostics they have always raised.
	#[test]
	fn legacy_gates_are_preserved() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(min = 1))]
				enabled: bool,
			}
		};
		let error = take_value_validations(&mut item).expect_err("bool must reject min");

		assert!(
			error
				.to_string()
				.contains("`min` and `max` on field `enabled` require an integer field"),
			"{error}"
		);

		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(exact_len = 4))]
				#[pina(validate(max_len = 8))]
				label: String<32>,
			}
		};
		let error =
			take_value_validations(&mut item).expect_err("exact_len with max_len must conflict");

		assert!(
			error
				.to_string()
				.contains("cannot be combined with `min_len` or `max_len`"),
			"{error}"
		);
	}

	/// Every diagnostic the parser can raise, asserted on its own text so a
	/// rewrite that changes one is caught here rather than in a UI fixture.
	#[test]
	fn every_parse_error_names_its_fix() {
		for (source, expected) in [
			("", "empty `validate(...)` annotation"),
			("value", "expected a comparison operator"),
			// A separator is required between two named parameters.
			("min = 1 2", "expected `,` or `&&`"),
			(
				"count == 2",
				"must mention the field's `value` or its `len`",
			),
			("value >= 1 < 2", "the middle of a chained comparison"),
			("value >= len", "not against each other"),
			("4 == len == 4", "takes a single bound"),
			("value = 2", "comparison rules use `==`"),
			("1 < value < 2 < len < 3", "checks at most one range"),
			("value >= ", "expected a bound on the other side"),
			// A generic bound splits at `<`, so it reads as an over-long chain.
			("value >= Vec<u8>", "checks at most one range"),
			("value >= 1 << 2", "wrap a bound containing generics"),
			("value >= 1 >> 2", "wrap a bound containing shifts"),
		] {
			// Each of these must fail; `.err()` keeps the panic in `core`
			// rather than adding an uncoverable branch here.
			let error = syn::parse_str::<ValueGroup>(source).err().unwrap();

			assert!(
				error.to_string().contains(expected),
				"`{source}` should report `{expected}`: {error}"
			);
		}
	}

	/// A name that is neither a receiver nor a named parameter reaches the
	/// comparison parser, whose hint names the operator that is missing.
	#[test]
	fn a_misspelled_parameter_is_reported_as_a_comparison_rule() {
		let error = syn::parse_str::<ValueGroup>("minimum = 1").expect_err("unknown parameter");

		assert!(
			error.to_string().contains("comparison rules use `==`"),
			"`minimum = 1` must fall through to the comparison parser: {error}"
		);
	}

	/// The generated check, rendered compactly, for one rule on one field.
	#[cfg(feature = "validation")]
	fn generated(source: &str, field: &str, ty: &str) -> String {
		let target = syn::parse_quote!(ExampleZc);
		let fields = vec![(
			Ident::new(field, Span::call_site()),
			syn::parse_str::<Type>(ty).unwrap(),
			vec![syn::parse_str::<ValueGroup>(source).unwrap()],
		)];

		generate_value_validation(
			&syn::parse_quote!(::pina),
			ValueTarget::Fixed(&target),
			&fields,
			None,
			&quote!(::pina::ProgramError::InvalidAccountData),
		)
		.to_string()
		.replace(' ', "")
	}

	/// The generated check reads the field through the accessor the view
	/// exposes, and the `Pod*` wrappers through `.get()`.
	#[cfg(feature = "validation")]
	#[test]
	fn comparisons_read_the_view_accessor() {
		let plain = generated("value >= 1", "amount", "u64");
		let pod = generated("value >= 1", "amount", "PodU64");

		assert!(plain.contains("((self.amount())>=(1))"), "{plain}");
		assert!(pod.contains("((self.amount().get())>=(1))"), "{pod}");
		assert!(
			plain.contains("impl::pina::PinaValidateforExampleZc"),
			"{plain}"
		);
	}

	#[cfg(feature = "validation")]
	#[test]
	fn length_comparisons_call_len() {
		let generated = generated("len <= 64", "memo", "String<64>");

		assert!(
			generated.contains("((self.memo().len())<=(64))"),
			"{generated}"
		);
	}

	/// A comparison is negated, so the group's error is raised on violation.
	#[cfg(feature = "validation")]
	#[test]
	fn a_violated_comparison_raises_the_group_error() {
		let generated = generated("value >= 1, error = MyError::TooSmall", "amount", "u64");

		assert!(
			generated.contains("returnErr((MyError::TooSmall).into());"),
			"{generated}"
		);
	}

	/// The default error applies when the group declares no override.
	#[cfg(feature = "validation")]
	#[test]
	fn a_group_without_an_override_uses_the_default_error() {
		let generated = generated("value >= 1", "amount", "u64");

		assert!(
			generated.contains("returnErr(::pina::ProgramError::InvalidAccountData);"),
			"{generated}"
		);
	}

	/// A chain renders every operator in the order the author wrote it.
	#[cfg(feature = "validation")]
	#[test]
	fn a_chain_renders_each_operator_in_place() {
		let chained = generated("100 < value <= u64::MAX", "amount", "u64");

		assert!(
			chained.contains("((100)<(self.amount()))&&((self.amount())<=(u64::MAX))"),
			"{chained}"
		);
	}

	/// Every deprecated spelling keeps the exact check shape it has always had.
	#[cfg(feature = "validation")]
	#[test]
	fn legacy_bounds_generate_their_original_shapes() {
		for (source, expected) in [
			("min = 1", "ifself.amount()<(1){returnErr("),
			("max = 10", "ifself.amount()>(10){returnErr("),
		] {
			let generated = generated(source, "amount", "u64");

			assert!(generated.contains(expected), "`{source}`: {generated}");
		}

		for (source, expected) in [
			("min_len = 2", "ifself.memo().len()<(2){returnErr("),
			("max_len = 64", "ifself.memo().len()>(64){returnErr("),
			("exact_len = 4", "ifself.memo().len()!=(4){returnErr("),
		] {
			let generated = generated(source, "memo", "String<64>");

			assert!(generated.contains(expected), "`{source}`: {generated}");
		}
	}

	/// A compact view reads through the tail accessor, and the short integer
	/// forms dereference directly.
	#[cfg(feature = "validation")]
	#[test]
	fn compact_targets_read_through_the_generated_accessors() {
		let target = syn::parse_quote!(ExampleRef);
		let tails = vec![crate::schema::CompactTail {
			name: Ident::new("memo", Span::call_site()),
			pod: quote!(PodString),
			capacity: quote!(64),
			optional: false,
		}];
		let amount: Type = syn::parse_quote!(u64);
		let memo: Type = syn::parse_quote!(String<64>);
		let fields = vec![
			(
				Ident::new("amount", Span::call_site()),
				amount,
				vec![syn::parse_str::<ValueGroup>("value >= 1").unwrap()],
			),
			(
				Ident::new("memo", Span::call_site()),
				memo,
				vec![syn::parse_str::<ValueGroup>("len <= 64").unwrap()],
			),
		];
		let generated = generate_value_validation(
			&syn::parse_quote!(::pina),
			ValueTarget::Compact {
				target: &target,
				tails: &tails,
			},
			&fields,
			None,
			&quote!(::pina::ProgramError::InvalidAccountData),
		)
		.to_string()
		.replace(' ', "");

		assert!(
			generated.contains("((self.amount.get())>=(1))"),
			"{generated}"
		);
		assert!(
			generated.contains("((self.memo().len())<=(64))"),
			"{generated}"
		);
		assert!(
			generated.contains("impl<'__pina_validation>::pina::PinaValidate"),
			"{generated}"
		);
	}

	/// A `u8` compact field is stored inline, so it is read directly rather
	/// than through `.get()`.
	#[cfg(feature = "validation")]
	#[test]
	fn a_compact_byte_field_is_read_directly() {
		let target = syn::parse_quote!(ExampleRef);
		let bump: Type = syn::parse_quote!(u8);
		let fields = vec![(
			Ident::new("bump", Span::call_site()),
			bump,
			vec![syn::parse_str::<ValueGroup>("value >= 1").unwrap()],
		)];
		let generated = generate_value_validation(
			&syn::parse_quote!(::pina),
			ValueTarget::Compact {
				target: &target,
				tails: &[],
			},
			&fields,
			None,
			&quote!(::pina::ProgramError::InvalidAccountData),
		)
		.to_string()
		.replace(' ', "");

		assert!(generated.contains("((self.bump)>=(1))"), "{generated}");
	}

	/// A compact field with no tail reads its length directly, and an array
	/// field is recognized as sized without naming a wrapper type.
	#[cfg(feature = "validation")]
	#[test]
	fn compact_plain_fields_and_arrays_report_their_length() {
		let target = syn::parse_quote!(ExampleRef);
		let tags: Type = syn::parse_quote!([u8; 4]);
		let fields = vec![
			(
				Ident::new("plain", Span::call_site()),
				syn::parse_quote!(String<8>),
				vec![syn::parse_str::<ValueGroup>("len <= 8").unwrap()],
			),
			(
				Ident::new("tags", Span::call_site()),
				tags.clone(),
				vec![syn::parse_str::<ValueGroup>("len == 4").unwrap()],
			),
		];
		let generated = generate_value_validation(
			&syn::parse_quote!(::pina),
			ValueTarget::Compact {
				target: &target,
				tails: &[],
			},
			&fields,
			None,
			&quote!(::pina::ProgramError::InvalidAccountData),
		)
		.to_string()
		.replace(' ', "");

		// A compact tail reads through its accessor; an array is a plain field
		// and reads `.len` directly.
		assert!(
			generated.contains("((self.plain.len())<=(8))"),
			"{generated}"
		);
		assert!(
			generated.contains("((self.tags.len())==(4))"),
			"{generated}"
		);
		assert!(has_length(&tags), "an array is sized");
	}

	/// A type that is not a path has no name to compare against, so both gates
	/// decline it rather than guessing.
	#[test]
	fn an_unnamed_type_is_neither_integer_nor_sized() {
		let tuple: Type = syn::parse_quote!((u64, u64));
		let reference: Type = syn::parse_quote!(&'static str);

		for ty in [&tuple, &reference] {
			assert!(last_type_name(ty).is_none(), "{ty:?}");
			assert!(!is_integer(ty), "{ty:?}");
			assert!(!has_length(ty), "{ty:?}");
		}
	}

	/// Every integer spelling the gate accepts, and one it does not.
	#[test]
	fn the_integer_gate_names_every_supported_type() {
		for name in [
			"u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "PodU16",
			"PodU32", "PodU64", "PodU128", "PodI16", "PodI32", "PodI64", "PodI128",
		] {
			let ty: Type = syn::parse_str(name).unwrap_or_else(|e| panic!("{name}: {e}"));

			assert!(is_integer(&ty), "`{name}` must be accepted");
		}

		for name in ["bool", "Address", "PodBool", "String<8>"] {
			let ty: Type = syn::parse_str(name).unwrap_or_else(|e| panic!("{name}: {e}"));

			assert!(!is_integer(&ty), "`{name}` must be rejected");
		}
	}

	/// Every wrapper the length gate accepts, and one it does not.
	#[test]
	fn the_length_gate_names_every_supported_type() {
		for name in [
			"String<8>",
			"PodString<8, 1>",
			"Vec<u8, 8>",
			"PodVec<u8, 8, 1>",
			"[u8; 8]",
		] {
			let ty: Type = syn::parse_str(name).unwrap_or_else(|e| panic!("{name}: {e}"));

			assert!(has_length(&ty), "`{name}` must be accepted");
		}

		for name in ["u64", "bool", "Address"] {
			let ty: Type = syn::parse_str(name).unwrap_or_else(|e| panic!("{name}: {e}"));

			assert!(!has_length(&ty), "`{name}` must be rejected");
		}
	}

	/// A field that is not named cannot carry a rule, and a non-`pina`
	/// attribute is left for the rest of the expansion.
	#[test]
	fn only_named_fields_and_pina_attributes_are_considered() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[doc = "kept"]
				value: u64,
			}
		};
		let validations = take_value_validations(&mut item).unwrap();

		assert_eq!(validations.fields.len(), 1);
		assert!(
			item.fields
				.iter()
				.any(|field| field.attrs.iter().any(|a| a.path().is_ident("doc"))),
			"a non-`pina` attribute is retained"
		);

		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(value >= 1))]
				value: u64,
				#[pina(validate(value >= 2))]
				count: u32,
			}
		};
		let validations = take_value_validations(&mut item).unwrap();

		assert_eq!(validations.fields.len(), 2);
		assert!(
			item.fields.iter().all(|field| field.attrs.is_empty()),
			"every `pina` attribute is consumed: {:?}",
			item.fields
		);
	}

	/// A parse failure in any field's attributes surfaces once, with the
	/// grammar hint, rather than being swallowed.
	#[test]
	fn a_malformed_group_fails_the_whole_struct() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(value >= 1))]
				good: u64,
				#[pina(validate(value = 2))]
				bad: u64,
			}
		};
		let error = take_value_validations(&mut item).expect_err("`value = 2` is not valid");

		assert!(
			error.to_string().contains("comparison rules use `==`")
				&& error.to_string().contains("Rules compare the field's"),
			"{error}"
		);
	}

	/// A separator is consumed between rules; a trailing one is accepted so a
	/// vertically formatted annotation can keep its final comma.
	#[test]
	fn a_trailing_separator_is_accepted() {
		for source in ["value >= 1,", "value >= 1 &&", "value >= 1, len == 4,"] {
			let parsed = group(source);

			assert!(!parsed.items.is_empty(), "`{source}` parses");
		}
	}

	/// The struct-level hook runs after every field check.
	#[cfg(feature = "validation")]
	#[test]
	fn the_hook_runs_after_the_field_checks() {
		let target = syn::parse_quote!(ExampleZc);
		let fields = vec![(
			Ident::new("amount", Span::call_site()),
			syn::parse_quote!(u64),
			vec![syn::parse_str::<ValueGroup>("value >= 1").unwrap()],
		)];
		let hook = ValidationHook {
			with: syn::parse_quote!(check_domain),
		};
		let generated = generate_value_validation(
			&syn::parse_quote!(::pina),
			ValueTarget::Fixed(&target),
			&fields,
			Some(&hook),
			&quote!(::pina::ProgramError::InvalidAccountData),
		)
		.to_string()
		.replace(' ', "");

		let check = generated.find("self.amount()").unwrap();
		let call = generated.find("check_domain(self)?;").unwrap();

		assert!(check < call, "the hook follows the checks: {generated}");
	}

	#[test]
	fn field_attributes_reject_a_foreign_option() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(len = 4)]
				value: u64,
			}
		};
		let error = take_value_validations(&mut item).expect_err("`len = 4` is not an option");

		assert!(
			error.to_string().contains("unknown `#[pina]` option"),
			"{error}"
		);
	}

	#[test]
	fn a_tuple_struct_carries_no_field_rules() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example(u64);
		};
		let validations = take_value_validations(&mut item).unwrap();

		assert!(validations.fields.is_empty());
		assert!(validations.deprecations.is_empty());
	}

	#[test]
	fn a_legacy_length_bound_on_an_unsized_field_is_rejected() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(max_len = 8))]
				value: u64,
			}
		};
		let error = take_value_validations(&mut item).expect_err("u64 has no length");

		assert!(
			error.to_string().contains("length validation on field"),
			"{error}"
		);
	}

	#[test]
	fn a_duplicate_legacy_bound_is_rejected() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(min = 1))]
				#[pina(validate(min = 2))]
				value: u64,
			}
		};
		let error = take_value_validations(&mut item).expect_err("two `min` bounds");

		assert!(
			error.to_string().contains("duplicate `min` validation"),
			"{error}"
		);
	}

	#[test]
	fn every_legacy_parameter_names_itself_and_its_receiver() {
		for (ident, name, receiver) in [
			("min", "min", ValueReceiver::Value),
			("max", "max", ValueReceiver::Value),
			("min_len", "min_len", ValueReceiver::Len),
			("max_len", "max_len", ValueReceiver::Len),
			("exact_len", "exact_len", ValueReceiver::Len),
		] {
			let parameter = LegacyParam::from_ident(&Ident::new(ident, Span::call_site())).unwrap();

			assert_eq!(parameter.name(), name);
			assert_eq!(parameter.receiver(), receiver);
		}
		assert!(LegacyParam::from_ident(&Ident::new("value", Span::call_site())).is_none());
	}

	#[test]
	fn a_named_parameter_classifies_error_and_bounds() {
		let span = Span::call_site();
		let error = NamedParameter::from_ident(&Ident::new("error", span));

		assert!(matches!(error, Some(NamedParameter::Error)));
		assert!(matches!(
			NamedParameter::from_ident(&Ident::new("min", span)),
			Some(NamedParameter::Bound(LegacyParam::Min))
		));
		assert!(NamedParameter::from_ident(&Ident::new("value", span)).is_none());
	}

	#[test]
	fn a_foreign_option_message_names_the_supported_grammar() {
		let error = attribute_error(&DarlingError::custom("unexpected argument"), "instruction")
			.to_string();

		assert!(
			error.contains("`discriminator = path`") && error.contains("value >= MIN"),
			"the hint carries the whole vocabulary: {error}"
		);
	}

	#[test]
	fn the_account_vocabulary_adds_compact_and_the_others_do_not() {
		let account = attribute_error(&DarlingError::custom("bad"), "account").to_string();
		let instruction = attribute_error(&DarlingError::custom("bad"), "instruction").to_string();

		assert!(account.contains("`compact`"), "{account}");
		assert!(!instruction.contains("`compact`"), "{instruction}");
	}

	#[test]
	fn darling_reasons_end_in_one_sentence() {
		assert_eq!(
			darling_reason(&DarlingError::custom("bad input")),
			"bad input."
		);
		assert_eq!(darling_reason(&DarlingError::custom("why?")), "why?");
		assert_eq!(darling_reason(&DarlingError::custom("stop!")), "stop!");
	}

	#[test]
	fn a_parse_error_is_wrapped_in_the_grammar_hint() {
		let inner = syn::Error::new(Span::call_site(), "stray token");
		let wrapped = invalid_annotation(&inner).to_string();

		assert!(
			wrapped.starts_with("invalid Pina validation annotation: stray token")
				&& wrapped.contains("100 < value <= u64::MAX"),
			"{wrapped}"
		);
	}

	/// One `#[pina(...)]` attribute may carry several `validate(...)` groups,
	/// which is what the comma between them distinguishes.
	#[test]
	fn one_attribute_may_carry_several_groups() {
		let mut item: ItemStruct = syn::parse_quote! {
			struct Example {
				#[pina(validate(value >= 1), validate(value <= 10))]
				value: u64,
			}
		};
		let validations = take_value_validations(&mut item).unwrap();

		assert_eq!(validations.fields.len(), 1);
		assert_eq!(validations.fields[0].2.len(), 2, "both groups are kept");
	}

	/// Every legacy bound records itself as deprecated on a field its gate
	/// accepts, which is what makes the warning fire per spelling.
	#[test]
	fn every_legacy_bound_is_recorded_as_deprecated() {
		for (source, ty, attribute) in [
			(
				"min = 1",
				"u64",
				syn::parse_quote!(#[pina(validate(min = 1))]),
			),
			(
				"max = 2",
				"u64",
				syn::parse_quote!(#[pina(validate(max = 2))]),
			),
			(
				"min_len = 3",
				"String<8>",
				syn::parse_quote!(#[pina(validate(min_len = 3))]),
			),
			(
				"max_len = 4",
				"String<8>",
				syn::parse_quote!(#[pina(validate(max_len = 4))]),
			),
			(
				"exact_len = 5",
				"String<8>",
				syn::parse_quote!(#[pina(validate(exact_len = 5))]),
			),
		] {
			let ty: Type = syn::parse_str(ty).unwrap_or_else(|e| panic!("type `{ty}`: {e}"));
			let mut item: ItemStruct = syn::parse_quote! {
				struct Example {
					value: #ty,
				}
			};
			item.fields.iter_mut().next().unwrap().attrs.push(attribute);

			let validations =
				take_value_validations(&mut item).unwrap_or_else(|e| panic!("`{source}`: {e}"));

			assert_eq!(validations.deprecations.len(), 1, "`{source}` warns");
		}
	}

	/// The `error` arm of the summary renders distinctly from the rules.
	#[test]
	fn the_summary_marks_an_error_override() {
		let parsed = group("value >= 1, error = E");

		assert!(summarize(&parsed).ends_with("|error"), "{parsed:?}");
	}
}
