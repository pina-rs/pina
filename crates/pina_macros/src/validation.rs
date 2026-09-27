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

/// One side of a comparison: the receiver, an operator, or a bound.
#[derive(Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
enum ComparisonPart {
	Receiver,
	Operator(ComparisonOperator),
	Bound(TokenStream),
}

/// A comparison over the field's `value` or `len`.
#[derive(Debug)]
#[cfg_attr(not(feature = "validation"), allow(dead_code))]
struct ComparisonRule {
	receiver: ValueReceiver,
	parts: Vec<ComparisonPart>,
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
		let ident = field.ident.clone().ok_or_else(|| {
			syn::Error::new_spanned(&*field, "validation annotations require a named field")
		})?;

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
	if input.peek(Ident) && lone_parameter_equals(input)? {
		let ident: Ident = input.parse()?;
		input.parse::<Token![=]>()?;
		let bound: Expr = input.parse()?;
		let span = ident.span();

		return match ident.to_string().as_str() {
			"error" => Ok((ValueItem::Error(bound, span), None)),
			"min" => Ok(deprecated_bound(span, LegacyParam::Min, bound)),
			"max" => Ok(deprecated_bound(span, LegacyParam::Max, bound)),
			"min_len" => Ok(deprecated_bound(span, LegacyParam::MinLen, bound)),
			"max_len" => Ok(deprecated_bound(span, LegacyParam::MaxLen, bound)),
			"exact_len" => Ok(deprecated_bound(span, LegacyParam::ExactLen, bound)),
			name => {
				Err(syn::Error::new(
					span,
					format!(
						"unknown validation parameter `{name}`; rules compare the field's `value` \
						 or its `len`, as in `value >= 1`"
					),
				))
			}
		};
	}

	Ok((ValueItem::Comparison(parse_comparison(input)?), None))
}

/// Whether an identifier at the front of `input` is followed by a single `=`
/// rather than the `==` of a comparison.
fn lone_parameter_equals(input: ParseStream<'_>) -> syn::Result<bool> {
	let fork = input.fork();
	let Ok(ident) = fork.parse::<Ident>() else {
		return Ok(false);
	};
	if LegacyParam::from_ident(&ident).is_none() && ident != "error" {
		return Ok(false);
	}

	fork.step(|cursor| {
		let lone = matches!(
			cursor.punct(),
			Some((punct, _)) if punct.as_char() == '=' && punct.spacing() == Spacing::Alone,
		);
		Ok((lone, *cursor))
	})
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
		let mut parts = vec![ComparisonPart::Bound(TokenStream::new())];
		let mut operators = Vec::new();
		let mut index = 0;

		while index < trees.len() {
			if let Some(operator) = ComparisonOperator::at(trees, index)? {
				index += operator.width();
				operators.push(operator.kind);
				parts.push(ComparisonPart::Operator(operator));
				parts.push(ComparisonPart::Bound(TokenStream::new()));
			} else {
				let ComparisonPart::Bound(bound) = parts.last_mut().unwrap() else {
					unreachable!("chunks alternate bounds and operators");
				};
				bound.extend([trees[index].clone()]);
				index += 1;
			}
		}

		if parts.len() == 1 {
			return Err(syn::Error::new(
				span,
				"expected a comparison operator (`==`, `!=`, `<`, `<=`, `>`, `>=`) in the rule; \
				 rules look like `value >= 1` or `len == 4`",
			));
		}
		if parts.len() > 5 {
			return Err(syn::Error::new(
				span,
				"a comparison rule checks at most one range; separate extra comparisons with `,`",
			));
		}
		for part in &parts {
			let ComparisonPart::Bound(bound) = part else {
				continue;
			};
			if bound.is_empty() {
				return Err(syn::Error::new(
					span,
					"expected a bound on the other side of the comparison operator",
				));
			}
		}

		// Exactly one side of the chain names the receiver.
		let mut receiver = None;
		let mut receiver_index = None;
		for (index, part) in parts.iter().enumerate() {
			let ComparisonPart::Bound(bound) = part else {
				continue;
			};
			let Some(chunk_receiver) = receiver_from_tokens(bound) else {
				continue;
			};
			if receiver.replace(chunk_receiver).is_some() {
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

		// A chain puts the receiver in the middle: `4 < len <= 100`.
		if parts.len() == 5 && receiver_index != Some(2) {
			return Err(syn::Error::new(
				span,
				"the middle of a chained comparison must be the field's `value` or its `len`",
			));
		}
		// An equality is a single fact, not a range to chain.
		if parts.len() == 5
			&& operators
				.iter()
				.any(|operator| matches!(*operator, ComparisonKind::Eq | ComparisonKind::Ne))
		{
			return Err(syn::Error::new(
				span,
				"an `==` or `!=` rule takes a single bound; chain only `<`, `<=`, `>`, or `>=`",
			));
		}

		parts[receiver_index.expect("a receiver was found above")] = ComparisonPart::Receiver;

		Ok(Self {
			receiver,
			parts,
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

	let mut condition = TokenStream::new();
	for part in &rule.parts {
		match part {
			ComparisonPart::Receiver => condition.extend(quote!((#receiver))),
			ComparisonPart::Operator(operator) => condition.extend(operator.tokens.clone()),
			ComparisonPart::Bound(bound) => condition.extend(quote!((#bound))),
		}
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
		syn::parse_str(source).unwrap_or_else(|error| panic!("parse `{source}`: {error}"))
	}

	/// Render a group's rules compactly so structure is assertable.
	fn summarize(group: &ValueGroup) -> String {
		group
			.items
			.iter()
			.map(|item| {
				match item {
					ValueItem::Comparison(rule) => {
						let parts = rule
							.parts
							.iter()
							.map(|part| {
								match part {
									ComparisonPart::Receiver => {
										match rule.receiver {
											ValueReceiver::Value => "value".to_owned(),
											ValueReceiver::Len => "len".to_owned(),
										}
									}
									ComparisonPart::Operator(operator) => {
										format!("{:?}", operator.kind)
									}
									ComparisonPart::Bound(bound) => {
										bound.to_string().replace(' ', "")
									}
								}
							})
							.collect::<Vec<_>>()
							.join(" ");
						format!("{:?}[{parts}]", rule.receiver)
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
		assert_eq!(summarize(&parsed), "Value[100 Lt value Le u64::MAX]");
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
}
