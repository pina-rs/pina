//! A source-location side table for the explained program.
//!
//! The public IR carries no spans, and adding fields to its structs would break
//! the published API. This index is built from the same resolved syntax trees
//! and keeps what a diagnosis reads: each accounts struct's slot layout and
//! rules with their lines, the account calls in each `process()` body, the value
//! rules on instruction arguments, every `#[error]` variant, and the dispatch
//! from instruction to accounts struct.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use heck::ToSnakeCase;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::ToTokens;
use serde::Serialize;
use syn::Item;
use syn::meta::ParseNestedMeta;

use crate::parse::accounts_struct::has_accounts_derive;
use crate::parse::entrypoint;
use crate::parse::error_enum::DeclaredError;
use crate::parse::error_enum::extract_declared_errors;
use crate::parse::module_resolver::ResolvedFile;
use crate::parse::validation::HelperFunctions;
use crate::parse::validation::extract_assertion_sites;

/// A `path:line` position, with the path relative to the project root.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Location {
	path: String,
	line: usize,
}

impl Location {
	fn new(path: &str, span: Span) -> Self {
		Self {
			path: path.to_owned(),
			line: span.start().line,
		}
	}
}

impl fmt::Display for Location {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(formatter, "{}:{}", self.path, self.line)
	}
}

/// One declarative account rule from `#[pina(validate(...))]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuleKind {
	Signer,
	Writable,
	Executable,
	Address,
	Addresses,
	Owner,
	Owners,
	Program,
	Sysvar,
	Empty,
	NotEmpty,
	DataLen,
	DistinctFrom,
}

/// The validation phase a rule runs in. `#[derive(Accounts)]` runs every
/// field's checks of one phase before the next phase starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
	Header,
	Identity,
	Data,
	Relationship,
}

impl RuleKind {
	/// Every rule in the order the generated validator checks them within one
	/// `validate(...)` group.
	pub(crate) const ALL: [Self; 13] = [
		Self::Signer,
		Self::Writable,
		Self::Executable,
		Self::Address,
		Self::Addresses,
		Self::Owner,
		Self::Owners,
		Self::Program,
		Self::Sysvar,
		Self::Empty,
		Self::NotEmpty,
		Self::DataLen,
		Self::DistinctFrom,
	];

	pub(crate) const fn name(self) -> &'static str {
		match self {
			Self::Signer => "signer",
			Self::Writable => "writable",
			Self::Executable => "executable",
			Self::Address => "address",
			Self::Addresses => "addresses",
			Self::Owner => "owner",
			Self::Owners => "owners",
			Self::Program => "program",
			Self::Sysvar => "sysvar",
			Self::Empty => "empty",
			Self::NotEmpty => "not_empty",
			Self::DataLen => "data_len",
			Self::DistinctFrom => "distinct_from",
		}
	}

	pub(crate) const fn phase(self) -> Phase {
		match self {
			Self::Signer | Self::Writable | Self::Executable => Phase::Header,
			Self::Address
			| Self::Addresses
			| Self::Owner
			| Self::Owners
			| Self::Program
			| Self::Sysvar => Phase::Identity,
			Self::Empty | Self::NotEmpty | Self::DataLen => Phase::Data,
			Self::DistinctFrom => Phase::Relationship,
		}
	}

	fn from_name(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|kind| kind.name() == name)
	}
}

/// A declared account rule with its location.
#[derive(Clone, Debug)]
pub(crate) struct DeclaredRule {
	pub(crate) kind: RuleKind,
	pub(crate) value: Option<syn::Expr>,
	/// Leading path of the group's `error = ...` override, such as
	/// `["ValidationError", "InvalidAccounts"]`.
	pub(crate) error: Option<Vec<String>>,
	/// Index of the `validate(...)` group on its field.
	pub(crate) group: usize,
	pub(crate) location: Location,
}

impl DeclaredRule {
	/// The rule as written, such as `owner = ID`.
	pub(crate) fn describe(&self) -> String {
		match &self.value {
			Some(value) => format!("{} = {}", self.kind.name(), compact_tokens(value)),
			None => self.kind.name().to_owned(),
		}
	}
}

/// How `#[derive(Accounts)]` parses one field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
	/// `&AccountView`, `&mut AccountView`, or an `Option` of either.
	Account { mutable: bool, optional: bool },
	/// Another `#[derive(Accounts)]` struct, parsed in place.
	Nested(String),
	/// A trailing `#[pina(remaining)]` slice.
	Remaining { mutable: bool, distinct: bool },
}

#[derive(Clone, Debug)]
pub(crate) struct LayoutField {
	pub(crate) name: String,
	pub(crate) kind: FieldKind,
	pub(crate) location: Location,
	pub(crate) rules: Vec<DeclaredRule>,
}

/// The slot layout and rules of one `#[derive(Accounts)]` struct.
#[derive(Clone, Debug)]
pub(crate) struct AccountsLayout {
	pub(crate) name: String,
	pub(crate) location: Location,
	pub(crate) fields: Vec<LayoutField>,
	/// Function named by the struct's `validate(with = ...)` hook.
	pub(crate) hook: Option<String>,
}

/// An account call in a `process()` body.
#[derive(Clone, Debug)]
pub(crate) struct ProcessSite {
	pub(crate) field: String,
	pub(crate) method: String,
	pub(crate) location: Location,
}

/// A value rule on an instruction argument.
#[derive(Clone, Debug)]
pub(crate) struct ValueRule {
	pub(crate) field: String,
	pub(crate) rule: String,
	pub(crate) error: Option<Vec<String>>,
	pub(crate) location: Location,
}

/// The value rules and hook of one `#[instruction]` struct.
#[derive(Clone, Debug)]
pub(crate) struct InstructionRules {
	pub(crate) rules: Vec<ValueRule>,
	pub(crate) hook: Option<String>,
	/// The struct identifier.
	pub(crate) location: Location,
}

/// Where the program constructs an error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorSite {
	/// `path:line` of the construction.
	pub location: String,
	/// Enclosing item, such as `CheckAccounts::process` or `validate_policy`.
	pub context: String,
	/// Whether the site belongs to the failing instruction's accounts struct,
	/// instruction struct, processor, or validation hooks.
	pub in_failing_instruction: bool,
}

/// Types and functions that belong to the failing instruction.
#[derive(Clone, Debug, Default)]
pub(crate) struct Flow {
	pub(crate) types: HashSet<String>,
	pub(crate) functions: HashSet<String>,
}

/// The source side table.
pub(crate) struct SourceIndex {
	pub(crate) accounts: HashMap<String, AccountsLayout>,
	pub(crate) process_sites: HashMap<String, Vec<ProcessSite>>,
	pub(crate) instructions: HashMap<String, InstructionRules>,
	pub(crate) errors: Vec<DeclaredError>,
	/// Snake-case instruction name → accounts struct.
	pub(crate) dispatch: HashMap<String, String>,
	files: Vec<(String, syn::File)>,
}

/// Index resolved source files, recording paths relative to `root`.
///
/// # Errors
///
/// Returns an error when an attribute the index reads does not follow the
/// grammar the Pina macros accept.
pub(crate) fn index(files: Vec<ResolvedFile>, root: &Path) -> Result<SourceIndex, syn::Error> {
	let mut accounts = HashMap::new();
	let mut process_sites = HashMap::new();
	let mut instructions = HashMap::new();
	let mut errors = Vec::new();
	let mut dispatch = HashMap::new();
	let paths = files
		.iter()
		.map(|resolved| {
			resolved
				.path
				.strip_prefix(root)
				.unwrap_or(&resolved.path)
				.to_string_lossy()
				.replace('\\', "/")
		})
		.collect::<Vec<_>>();
	let syntax = files
		.iter()
		.map(|resolved| &resolved.file)
		.collect::<Vec<_>>();
	// A processor can pass an account to a helper declared in any module, and a
	// check found there is located at the helper's own file and line.
	let helpers = HelperFunctions::collect(&syntax);

	for (file, path) in syntax.iter().copied().zip(&paths) {
		for item in &file.items {
			let Item::Struct(item) = item else {
				continue;
			};

			if has_accounts_derive(&item.attrs) {
				accounts.insert(item.ident.to_string(), accounts_layout(item, path)?);
			} else if item.attrs.iter().any(is_instruction_attribute) {
				instructions.insert(item.ident.to_string(), instruction_rules(item, path)?);
			}
		}

		for (struct_name, sites) in extract_assertion_sites(file, &helpers) {
			let sites = sites
				.into_iter()
				.map(|site| {
					let site_path = site.helper_file.map_or(path, |index| &paths[index]);
					ProcessSite {
						location: Location::new(site_path, site.span),
						field: site.field,
						method: site.method,
					}
				})
				.collect();
			process_sites.insert(struct_name, sites);
		}

		errors.extend(extract_declared_errors(file)?);

		// The generated dispatch reads the same routing facts a hand-written
		// match spells out; a hand-written match wins, as in IDL extraction.
		let file_dispatch = match entrypoint::extract_dispatch_map(file) {
			entries if !entries.is_empty() => entries,
			_ => entrypoint::extract_dispatch_from_attribute(file),
		};
		for entry in file_dispatch {
			if let Some(accounts_struct) = entry.accounts_struct {
				dispatch.insert(entry.variant.to_snake_case(), accounts_struct);
			}
		}
	}

	let indexed = paths
		.into_iter()
		.zip(files.into_iter().map(|resolved| resolved.file))
		.collect();

	Ok(SourceIndex {
		accounts,
		process_sites,
		instructions,
		errors,
		dispatch,
		files: indexed,
	})
}

impl SourceIndex {
	/// Find every `Enum::Variant` construction of one of `paths`, sites that
	/// belong to `flow` first.
	pub(crate) fn error_sites(&self, paths: &[(String, String)], flow: &Flow) -> Vec<ErrorSite> {
		let mut sites = Vec::new();

		for (path, file) in &self.files {
			scan_items(&file.items, path, paths, flow, &mut sites);
		}

		sites.sort_by(|left: &(bool, Location, String), right| {
			right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1))
		});
		sites.dedup_by(|left, right| left.1 == right.1);
		sites
			.into_iter()
			.map(|(in_flow, location, context)| {
				ErrorSite {
					location: location.to_string(),
					context,
					in_failing_instruction: in_flow,
				}
			})
			.collect()
	}
}

fn scan_items(
	items: &[Item],
	path: &str,
	paths: &[(String, String)],
	flow: &Flow,
	sites: &mut Vec<(bool, Location, String)>,
) {
	for item in items {
		match item {
			Item::Fn(function) => {
				let name = function.sig.ident.to_string();
				let in_flow = flow.functions.contains(&name);
				for span in path_spans(function.to_token_stream(), paths) {
					sites.push((in_flow, Location::new(path, span), name.clone()));
				}
			}
			Item::Impl(implementation) => {
				let type_name = type_name(&implementation.self_ty);
				let in_flow = flow.types.contains(&type_name);
				for impl_item in &implementation.items {
					let syn::ImplItem::Fn(function) = impl_item else {
						continue;
					};
					let context = format!("{type_name}::{}", function.sig.ident);
					for span in path_spans(function.to_token_stream(), paths) {
						sites.push((in_flow, Location::new(path, span), context.clone()));
					}
				}
			}
			Item::Struct(item) => {
				let name = item.ident.to_string();
				let in_flow = flow.types.contains(&name);
				for span in path_spans(item.to_token_stream(), paths) {
					sites.push((in_flow, Location::new(path, span), name.clone()));
				}
			}
			Item::Mod(module) => {
				if let Some((_, items)) = &module.content {
					scan_items(items, path, paths, flow, sites);
				}
			}
			_ => {}
		}
	}
}

/// Spans of every `Enum::Variant` token sequence in `tokens` that names one of
/// `paths`.
fn path_spans(tokens: TokenStream, paths: &[(String, String)]) -> Vec<Span> {
	let tokens: Vec<TokenTree> = tokens.into_iter().collect();
	let mut spans = Vec::new();

	for (index, token) in tokens.iter().enumerate() {
		if let TokenTree::Group(group) = token {
			spans.extend(path_spans(group.stream(), paths));
			continue;
		}

		let Some(
			[
				TokenTree::Ident(first),
				TokenTree::Punct(colon),
				TokenTree::Punct(path_separator),
				TokenTree::Ident(second),
			],
		) = tokens.get(index..index + 4)
		else {
			continue;
		};

		if colon.as_char() == ':'
			&& path_separator.as_char() == ':'
			&& paths
				.iter()
				.any(|(enum_name, variant)| first == enum_name && second == variant)
		{
			spans.push(first.span());
		}
	}

	spans
}

fn accounts_layout(item: &syn::ItemStruct, path: &str) -> Result<AccountsLayout, syn::Error> {
	let fields = named_fields(item)
		.map(|(ident, field)| layout_field(ident, field, path))
		.collect::<Result<_, _>>()?;

	Ok(AccountsLayout {
		name: item.ident.to_string(),
		location: Location::new(path, item.ident.span()),
		fields,
		hook: hook_name(&item.attrs, "pina"),
	})
}

fn layout_field(
	ident: &syn::Ident,
	field: &syn::Field,
	path: &str,
) -> Result<LayoutField, syn::Error> {
	let mut rules = Vec::new();
	let mut group = 0;
	let mut remaining = false;
	let mut distinct = true;

	for attribute in field
		.attrs
		.iter()
		.filter(|attribute| attribute.path().is_ident("pina"))
	{
		attribute.parse_nested_meta(|meta| {
			if meta.path.is_ident("validate") {
				rules.extend(validate_group(&meta, group, path)?);
				group += 1;
				return Ok(());
			}

			if meta.path.is_ident("remaining") {
				remaining = true;
				return Ok(());
			}

			if meta.path.is_ident("distinct") {
				if meta.input.peek(syn::Token![=]) {
					let value: syn::Expr = meta.value()?.parse()?;
					distinct = !matches!(
						value,
						syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(ref flag), .. }) if !flag.value
					);
				}
				return Ok(());
			}

			Err(meta.error(
				"unknown `#[pina]` account-field option; expected `validate(...)`, `remaining`, \
				 or `distinct`",
			))
		})?;
	}

	Ok(LayoutField {
		name: ident.to_string(),
		kind: field_kind(&field.ty, remaining, distinct),
		location: Location::new(path, ident.span()),
		rules,
	})
}

/// Parse one `validate(...)` group, applying its `error = ...` to every rule.
fn validate_group(
	meta: &ParseNestedMeta<'_>,
	group: usize,
	path: &str,
) -> Result<Vec<DeclaredRule>, syn::Error> {
	let mut rules = Vec::new();
	let mut error = None;

	meta.parse_nested_meta(|rule| {
		let name = compact_tokens(&rule.path);

		if name == "error" {
			let value: syn::Expr = rule.value()?.parse()?;
			error = Some(leading_path(value.to_token_stream()));
			return Ok(());
		}

		let kind = RuleKind::from_name(&name)
			.ok_or_else(|| rule.error(format!("unknown account validation rule `{name}`")))?;
		let location = Location::new(path, syn::spanned::Spanned::span(&rule.path));
		let value = if rule.input.peek(syn::Token![=]) {
			Some(rule.value()?.parse::<syn::Expr>()?)
		} else {
			None
		};

		rules.push(DeclaredRule {
			kind,
			value,
			error: None,
			group,
			location,
		});
		Ok(())
	})?;

	for rule in &mut rules {
		rule.error.clone_from(&error);
	}

	Ok(rules)
}

fn field_kind(ty: &syn::Type, remaining: bool, distinct: bool) -> FieldKind {
	let mutable = matches!(ty, syn::Type::Reference(reference) if reference.mutability.is_some());

	if remaining {
		return FieldKind::Remaining {
			mutable,
			distinct: mutable && distinct,
		};
	}

	if let Some(mutable) = optional_reference(ty) {
		return FieldKind::Account {
			mutable,
			optional: true,
		};
	}

	if matches!(ty, syn::Type::Reference(_)) {
		return FieldKind::Account {
			mutable,
			optional: false,
		};
	}

	FieldKind::Nested(type_name(ty))
}

/// Whether `ty` is `Option<&T>` (`Some(false)`) or `Option<&mut T>`
/// (`Some(true)`).
fn optional_reference(ty: &syn::Type) -> Option<bool> {
	let syn::Type::Path(path) = ty else {
		return None;
	};
	let segment = path.path.segments.last()?;
	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return None;
	};
	let Some(syn::GenericArgument::Type(syn::Type::Reference(reference))) = arguments.args.first()
	else {
		return None;
	};

	(segment.ident == "Option").then_some(reference.mutability.is_some())
}

fn type_name(ty: &syn::Type) -> String {
	if let syn::Type::Path(path) = ty
		&& let Some(segment) = path.path.segments.last()
	{
		return segment.ident.to_string();
	}

	compact_tokens(ty)
}

fn is_instruction_attribute(attribute: &syn::Attribute) -> bool {
	attribute
		.path()
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "instruction")
}

fn instruction_rules(item: &syn::ItemStruct, path: &str) -> Result<InstructionRules, syn::Error> {
	let mut rules = Vec::new();

	for (ident, field) in named_fields(item) {
		let name = ident.to_string();
		for attribute in field
			.attrs
			.iter()
			.filter(|attribute| attribute.path().is_ident("pina"))
		{
			// Schema fields accept only `validate(...)`, which the macro has
			// already checked.
			attribute.parse_nested_meta(|meta| {
				let content;
				syn::parenthesized!(content in meta.input);
				rules.extend(value_rules(&name, content.parse()?, path));

				Ok(())
			})?;
		}
	}

	Ok(InstructionRules {
		rules,
		hook: item
			.attrs
			.iter()
			.filter(|attribute| is_instruction_attribute(attribute))
			.find_map(|attribute| hook_in(attribute.meta.to_token_stream())),
		location: Location::new(path, item.ident.span()),
	})
}

/// Split one schema `validate(...)` group into its rules.
///
/// Schema rules are not all Rust expressions, so the group is split on
/// top-level commas and each rule is kept as written.
fn value_rules(field: &str, tokens: TokenStream, path: &str) -> Vec<ValueRule> {
	let mut items = Vec::new();
	let mut current: Vec<TokenTree> = Vec::new();

	for token in tokens {
		if matches!(&token, TokenTree::Punct(punct) if punct.as_char() == ',') {
			items.push(std::mem::take(&mut current));
		} else {
			current.push(token);
		}
	}
	items.push(current);

	let mut error = None;
	let mut rules = Vec::new();

	for item in items.into_iter().filter(|item| !item.is_empty()) {
		if let [TokenTree::Ident(ident), TokenTree::Punct(equals), rest @ ..] = item.as_slice()
			&& ident == "error"
			&& equals.as_char() == '='
		{
			error = Some(leading_path(rest.iter().cloned().collect()));
			continue;
		}

		let span = item[0].span();
		rules.push(ValueRule {
			field: field.to_owned(),
			rule: item.into_iter().collect::<TokenStream>().to_string(),
			error: None,
			location: Location::new(path, span),
		});
	}

	for rule in &mut rules {
		rule.error.clone_from(&error);
	}

	rules
}

/// The function named by a `with = path` pair anywhere in an attribute.
fn hook_name(attrs: &[syn::Attribute], attribute: &str) -> Option<String> {
	attrs
		.iter()
		.filter(|candidate| candidate.path().is_ident(attribute))
		.find_map(|candidate| hook_in(candidate.meta.to_token_stream()))
}

fn hook_in(tokens: TokenStream) -> Option<String> {
	let tokens: Vec<TokenTree> = tokens.into_iter().collect();

	for (index, token) in tokens.iter().enumerate() {
		match token {
			TokenTree::Group(group) => {
				if let Some(name) = hook_in(group.stream()) {
					return Some(name);
				}
			}
			TokenTree::Ident(ident) if ident == "with" => {
				if let Some(TokenTree::Punct(equals)) = tokens.get(index + 1)
					&& equals.as_char() == '='
				{
					return leading_path(tokens[index + 2..].iter().cloned().collect()).pop();
				}
			}
			_ => {}
		}
	}

	None
}

/// The identifiers of the path a token stream starts with.
fn leading_path(tokens: TokenStream) -> Vec<String> {
	let mut segments = Vec::new();

	for token in tokens {
		match token {
			TokenTree::Ident(ident) => segments.push(ident.to_string()),
			TokenTree::Punct(punct) if punct.as_char() == ':' => {}
			_ => break,
		}
	}

	segments
}

/// The named fields of a struct with their identifiers; tuple and unit
/// structs have none.
fn named_fields(item: &syn::ItemStruct) -> impl Iterator<Item = (&syn::Ident, &syn::Field)> {
	item.fields
		.iter()
		.filter_map(|field| field.ident.as_ref().map(|ident| (ident, field)))
}

/// Render tokens without the spaces `TokenStream` inserts, such as `system::ID`.
pub(crate) fn compact_tokens(value: &impl ToTokens) -> String {
	value.to_token_stream().to_string().replace(' ', "")
}

#[cfg(test)]
mod tests {
	use super::*;

	fn index_source(source: &str) -> Result<SourceIndex, syn::Error> {
		let file = syn::parse_file(source).expect("parses");

		index(
			vec![ResolvedFile {
				path: "/project/src/lib.rs".into(),
				file,
			}],
			Path::new("/project"),
		)
	}

	#[test]
	fn classifies_account_field_types() {
		assert_eq!(
			field_kind(&syn::parse_quote!(&'a AccountView), false, true),
			FieldKind::Account {
				mutable: false,
				optional: false
			}
		);
		assert_eq!(
			field_kind(&syn::parse_quote!(Option<&'a mut AccountView>), false, true),
			FieldKind::Account {
				mutable: true,
				optional: true
			}
		);
		assert_eq!(
			field_kind(&syn::parse_quote!(&'a mut [AccountView]), true, true),
			FieldKind::Remaining {
				mutable: true,
				distinct: true
			}
		);
		assert_eq!(
			field_kind(&syn::parse_quote!(&'a [AccountView]), true, true),
			FieldKind::Remaining {
				mutable: false,
				distinct: false
			}
		);
		assert_eq!(
			field_kind(&syn::parse_quote!(nested::Signers<'a>), false, true),
			FieldKind::Nested("Signers".to_owned())
		);
		assert_eq!(optional_reference(&syn::parse_quote!(Plain)), None);
		assert_eq!(optional_reference(&syn::parse_quote!(Vec<&'a u8>)), None);
		assert_eq!(type_name(&syn::parse_quote!((A, B))), "(A,B)");
	}

	#[test]
	fn reads_hooks_paths_and_value_rules() {
		assert_eq!(
			hook_in(quote::quote!(pina(validate(with = crate::hooks::check)))),
			Some("check".to_owned())
		);
		assert_eq!(hook_in(quote::quote!(pina(validate(with check)))), None);
		assert_eq!(hook_in(quote::quote!(pina(remaining))), None);
		assert_eq!(
			leading_path(quote::quote!(Enum::Variant.into())),
			["Enum", "Variant"]
		);

		let rules = value_rules(
			"amount",
			quote::quote!(value >= 1 && value <= 4, , error = AppError::Denied,),
			"src/lib.rs",
		);
		let rendered: Vec<_> = rules
			.iter()
			.map(|rule| (rule.rule.as_str(), rule.error.clone()))
			.collect();
		assert_eq!(
			rendered,
			[(
				"value >= 1 && value <= 4",
				Some(vec!["AppError".to_owned(), "Denied".to_owned()])
			)]
		);
	}

	#[test]
	fn indexes_remaining_options_and_rejects_unknown_ones() {
		let index = index_source(
			r"
			#[derive(Accounts)]
			pub struct Members<'a> {
				#[pina(remaining, distinct = true)]
				pub members: &'a mut [AccountView],
			}
			#[derive(Accounts)]
			pub struct Tuple<'a>(&'a AccountView);
			",
		)
		.expect("indexes");
		assert_eq!(
			index.accounts["Members"].fields[0].kind,
			FieldKind::Remaining {
				mutable: true,
				distinct: true
			}
		);
		assert!(index.accounts["Tuple"].fields.is_empty());

		let error = index_source(
			r"
			#[derive(Accounts)]
			pub struct Bad<'a> {
				#[pina(bogus)]
				pub account: &'a AccountView,
			}
			",
		)
		.err()
		.expect("an unknown option fails");
		assert!(
			error
				.to_string()
				.contains("unknown `#[pina]` account-field option")
		);
	}

	#[test]
	fn lists_each_construction_site_once_per_line() {
		let index = index_source(
			r"
			pub fn twice() -> ProgramResult {
				if check() { return Err(AppError::Denied.into()); } Err(AppError::Denied.into())
			}
			pub fn ratio() { compare!(AppError := Denied); }
			pub const IGNORED: AppError = AppError::Denied;
			mod external;
			impl Thing {
				const LIMIT: u32 = 1;
				fn make() -> ProgramResult { Err(AppError::Denied.into()) }
			}
			",
		)
		.expect("indexes");
		let flow = Flow {
			types: HashSet::from(["Thing".to_owned()]),
			functions: HashSet::new(),
		};
		let sites = index.error_sites(&[("AppError".to_owned(), "Denied".to_owned())], &flow);

		assert_eq!(
			sites,
			[
				ErrorSite {
					location: "src/lib.rs:10".to_owned(),
					context: "Thing::make".to_owned(),
					in_failing_instruction: true,
				},
				ErrorSite {
					location: "src/lib.rs:3".to_owned(),
					context: "twice".to_owned(),
					in_failing_instruction: false,
				},
			]
		);
	}
}
