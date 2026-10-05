use std::collections::BTreeSet;
use std::collections::HashMap;

use quote::ToTokens as _;
use syn::Expr;
use syn::ImplItem;
use syn::Item;
use syn::Pat;
use syn::Stmt;
use syn::punctuated::Punctuated;

use crate::ir::DefaultValueIr;

/// Known program addresses used for default value resolution.
const KNOWN_ADDRESSES: &[(&str, &str)] = &[
	("system::ID", "11111111111111111111111111111111"),
	("token::ID", "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
	(
		"token_2022::ID",
		"TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
	),
	(
		"associated_token_account::ID",
		"ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
	),
	(
		"instructions::ID",
		"Sysvar1nstructions1111111111111111111111111",
	),
	("clock::ID", "SysvarC1ock11111111111111111111111111111111"),
	(
		"epoch_rewards::ID",
		"SysvarEpochRewards1111111111111111111111111",
	),
	(
		"epoch_schedule::ID",
		"SysvarEpochSchedu1e111111111111111111111111",
	),
	("fees::ID", "SysvarFees111111111111111111111111111111111"),
	(
		"last_restart_slot::ID",
		"SysvarLastRestartS1ot1111111111111111111111",
	),
	(
		"recent_blockhashes::ID",
		"SysvarRecentB1ockHashes11111111111111111111",
	),
	("rent::ID", "SysvarRent111111111111111111111111111111111"),
	("rewards::ID", "SysvarRewards111111111111111111111111111111"),
	(
		"slot_hashes::ID",
		"SysvarS1otHashes111111111111111111111111111",
	),
	(
		"slot_history::ID",
		"SysvarS1otHistory11111111111111111111111111",
	),
	(
		"stake_history::ID",
		"SysvarStakeHistory1111111111111111111111111",
	),
];

const PDA_CREATION_BUILDERS: &[&str] = &[
	"CreateProgramAccount",
	"CreateProgramAccountWithBump",
	"CreateProgramAccountWithUncheckedBump",
	"CreateCompactProgramAccount",
	"CreateCompactProgramAccountWithBump",
];

const PDA_CREATION_METHODS: &[&str] = &[
	"invoke",
	"invoke_with",
	"invoke_signed",
	"invoke_signed_with",
	"invoke_with_bump",
	"invoke_signed_with_bump",
];

/// Calls whose first argument is an account they prove is a PDA.
///
/// Most are the static functions `#[pda]` generates on the account type
/// (`State::assert_stored_bump(self.state, ..)`); `assert_seeds_with_bump` and
/// `assert_canonical_bump` are the `AccountInfoValidation` methods the generated
/// functions call, which programs also invoke directly.
const PDA_VALIDATION_CALLS: &[&str] = &[
	"assert_seeds",
	"assert_stored_bump",
	"assert_seeds_with_bump",
	"assert_canonical_bump",
	"load_pda",
	"load_pda_mut",
	"load_checked_pda",
	"load_checked_pda_mut",
	"with_pda",
	"with_stored_bump_pda",
	"with_checked_pda",
];

/// PDA loaders that also borrow the account mutably, which the runtime only
/// allows for a writable account.
const MUTABLE_PDA_LOADERS: &[&str] = &["load_pda_mut", "load_checked_pda_mut"];

/// Account methods whose first generic argument names the account type they
/// load or validate, as in `self.state.as_account::<State>(&ID)`.
const TYPED_ACCOUNT_METHODS: &[&str] = &[
	"as_account",
	"as_account_mut",
	"with_compact_account",
	"update_compact_account",
	"assert_type",
	"assert_compact_type",
];

/// How many nested helper calls the analysis follows from a `process` body.
const MAX_HELPER_DEPTH: usize = 4;

/// Client properties declared by annotations or inferred from a validation
/// chain for one account field.
#[derive(Debug, Clone, Default)]
pub struct AccountProperties {
	pub is_signer: bool,
	pub is_writable: bool,
	pub is_pda: bool,
	pub default_value: Option<DefaultValueIr>,
}

/// Everything a `process` body reveals about one account field.
#[derive(Debug, Clone, Default)]
pub(crate) struct FieldFacts {
	pub(crate) properties: AccountProperties,
	/// Account types the body names for the field: `T` in
	/// `T::assert_seeds(self.field, ..)`, `self.field.as_account::<T>(..)`, and
	/// `CreateProgramAccount { account: self.field, .. }.invoke::<T>()`.
	///
	/// Assembly resolves these against the `#[pda]` account types, so a typed
	/// load of a PDA account marks the field as that PDA.
	pub(crate) account_types: BTreeSet<String>,
}

/// Free functions the crate declares at module level, by name.
///
/// A `process` body that passes an account field to one of these helpers is
/// analysed through the helper's body too. A name declared more than once is
/// ambiguous and never followed.
#[derive(Debug, Default)]
pub(crate) struct HelperFunctions<'file> {
	by_name: HashMap<String, Option<Helper<'file>>>,
}

/// A module-level function and the index of the file that declares it.
#[derive(Clone, Copy, Debug)]
struct Helper<'file> {
	function: &'file syn::ItemFn,
	file: usize,
}

impl<'file> HelperFunctions<'file> {
	/// Index the module-level functions of `files`. A site found in a helper
	/// names its file by its position in `files`.
	pub(crate) fn collect(files: &[&'file syn::File]) -> Self {
		let mut by_name = HashMap::new();

		for (file, item) in files
			.iter()
			.enumerate()
			.flat_map(|(index, file)| file.items.iter().map(move |item| (index, item)))
		{
			let Item::Fn(function) = item else {
				continue;
			};

			by_name
				.entry(function.sig.ident.to_string())
				.and_modify(|existing| *existing = None)
				.or_insert(Some(Helper { function, file }));
		}

		Self { by_name }
	}

	fn get(&self, name: &str) -> Option<Helper<'file>> {
		self.by_name.get(name).copied().flatten()
	}
}

/// Return a stable representation of declarative account constraints.
///
/// Signer, writable, optional, known-address, and PDA properties also have
/// dedicated ABI fields. Keeping the complete declarative rule set here makes
/// owner, executable, data-length, distinctness, and related changes visible
/// to process compatibility checks.
pub(crate) fn canonical_account_constraints(
	attributes: &[syn::Attribute],
) -> Result<Vec<String>, syn::Error> {
	let mut constraints = Vec::new();

	for attribute in attributes {
		if !attribute.path().is_ident("pina") {
			continue;
		}
		attribute.parse_nested_meta(|meta| {
			if meta.path.is_ident("validate") {
				return meta.parse_nested_meta(|rule| {
					let name = rule
						.path
						.segments
						.last()
						.map(|segment| segment.ident.to_string())
						.ok_or_else(|| rule.error("validation rule path cannot be empty"))?;
					if name == "error" {
						let _: Expr = rule.value()?.parse()?;
						return Ok(());
					}
					if !rule.input.peek(syn::Token![=]) {
						constraints.push(name);
						return Ok(());
					}
					let value: Expr = rule.value()?.parse()?;
					let rendered = value.to_token_stream().to_string().replace(' ', "");
					constraints.push(format!("{name}={rendered}"));
					Ok(())
				});
			}
			if meta.path.is_ident("remaining") {
				constraints.push("remaining".to_owned());
				return Ok(());
			}
			if meta.path.is_ident("distinct") {
				if meta.input.peek(syn::Token![=]) {
					let value: Expr = meta.value()?.parse()?;
					let rendered = value.to_token_stream().to_string().replace(' ', "");
					constraints.push(format!("distinct={rendered}"));
				} else {
					constraints.push("distinct".to_owned());
				}
				return Ok(());
			}
			Err(meta.error(
				"unknown account-field option; expected validate(...), remaining, or distinct",
			))
		})?;
	}

	constraints.sort();
	constraints.dedup();
	Ok(constraints)
}

/// Extract client-visible properties from declarative account validation.
///
/// Codama can represent signer, writable, and default-address metadata. Other
/// Pina validators remain runtime-only constraints and are still parsed here
/// so malformed source fails with a useful error instead of being ignored.
pub fn extract_attribute_properties(
	attributes: &[syn::Attribute],
) -> Result<AccountProperties, syn::Error> {
	let mut properties = AccountProperties::default();

	for attribute in attributes {
		if !attribute.path().is_ident("pina") {
			continue;
		}

		attribute.parse_nested_meta(|meta| {
			if meta.path.is_ident("validate") {
				return meta.parse_nested_meta(|rule| {
					if rule.path.is_ident("signer") {
						properties.is_signer = true;
						return Ok(());
					}
					if rule.path.is_ident("writable") {
						properties.is_writable = true;
						return Ok(());
					}
					if rule.path.is_ident("executable")
						|| rule.path.is_ident("empty")
						|| rule.path.is_ident("not_empty")
					{
						return Ok(());
					}

					let is_default_address = rule.path.is_ident("address")
						|| rule.path.is_ident("program")
						|| rule.path.is_ident("sysvar");
					if is_default_address
						|| rule.path.is_ident("addresses")
						|| rule.path.is_ident("owner")
						|| rule.path.is_ident("owners")
						|| rule.path.is_ident("data_len")
						|| rule.path.is_ident("distinct_from")
						|| rule.path.is_ident("error")
					{
						let value: Expr = rule.value()?.parse()?;
						if is_default_address && let Some(address) = known_address_from_expr(&value)
						{
							properties.default_value = Some(DefaultValueIr::PublicKey(address));
						}
						return Ok(());
					}

					Err(rule.error(
						"unknown account validation rule; expected `signer`, `writable`, \
						 `executable`, `address`, `addresses`, `owner`, `owners`, `program`, \
						 `sysvar`, `empty`, `not_empty`, `data_len`, `distinct_from`, or `error`",
					))
				});
			}

			if meta.path.is_ident("remaining") {
				return Ok(());
			}
			if meta.path.is_ident("distinct") {
				if meta.input.peek(syn::Token![=]) {
					let _: Expr = meta.value()?.parse()?;
				}
				return Ok(());
			}

			Err(meta.error(
				"unknown `#[pina]` account-field option; expected `validate(...)`, `remaining`, \
				 or `distinct`",
			))
		})?;
	}

	Ok(properties)
}

/// Extract declarative properties without adding parser state to the public
/// `AccountsField` model.
pub(crate) fn extract_declared_validation_properties(
	file: &syn::File,
) -> Result<HashMap<String, HashMap<String, AccountProperties>>, syn::Error> {
	let mut result = HashMap::new();

	for item in &file.items {
		let Item::Struct(item_struct) = item else {
			continue;
		};
		if !super::accounts_struct::has_accounts_derive(&item_struct.attrs) {
			continue;
		}
		let syn::Fields::Named(fields) = &item_struct.fields else {
			continue;
		};

		let mut field_properties = HashMap::new();
		for field in &fields.named {
			let name = field
				.ident
				.as_ref()
				.expect("named fields always have identifiers");
			field_properties.insert(
				name.to_string(),
				extract_attribute_properties(&field.attrs)?,
			);
		}
		result.insert(item_struct.ident.to_string(), field_properties);
	}

	Ok(result)
}

/// Analyse all `impl ProcessAccountInfos for X` blocks in a file and return a
/// map from the struct name (without lifetime) to a map of field name ->
/// properties.
///
/// Helper functions declared in the same file are followed when a `process`
/// body passes them an account field.
pub fn extract_validation_properties(
	file: &syn::File,
) -> HashMap<String, HashMap<String, AccountProperties>> {
	let helpers = HelperFunctions::collect(&[file]);

	process_walks(file, &helpers)
		.into_iter()
		.map(|(struct_name, walk)| {
			let properties = walk
				.fields
				.into_iter()
				.map(|(field_name, facts)| (field_name, facts.properties))
				.collect();
			(struct_name, properties)
		})
		.collect()
}

/// [`extract_validation_properties`] with the account types each field is
/// loaded as, following the crate-wide `helpers`.
pub(crate) fn extract_validation_facts(
	file: &syn::File,
	helpers: &HelperFunctions<'_>,
) -> HashMap<String, HashMap<String, FieldFacts>> {
	process_walks(file, helpers)
		.into_iter()
		.map(|(struct_name, walk)| (struct_name, walk.fields))
		.collect()
}

/// A recognized method or generated static call on one account field inside a
/// `process()` body, or inside a helper the body passes the field to.
#[derive(Debug, Clone)]
pub(crate) struct AssertionSite {
	/// Accounts-struct field the call validates.
	pub(crate) field: String,
	/// Called method, such as `assert_signer` or `load_pda`.
	pub(crate) method: String,
	/// Span of the method name, which carries its source line.
	pub(crate) span: proc_macro2::Span,
	/// The file the call is written in when a followed helper makes it: an
	/// index into the files `helpers` was collected from. `None` when the
	/// `process()` body makes it, so the span belongs to the body's own file.
	///
	/// A site inside a helper keeps the helper's own line rather than the line
	/// of the call in `process()`, because that is where the check is written
	/// and where a reader has to look to change it.
	pub(crate) helper_file: Option<usize>,
}

/// Collect the account calls in every `impl ProcessAccountInfos for X`, keyed by
/// the struct name, in execution order: a call's receiver and arguments are
/// recorded before the call itself, so a chain records its first link first,
/// and a followed helper's calls are recorded where the helper runs.
pub(crate) fn extract_assertion_sites(
	file: &syn::File,
	helpers: &HelperFunctions<'_>,
) -> HashMap<String, Vec<AssertionSite>> {
	process_walks(file, helpers)
		.into_iter()
		.map(|(struct_name, walk)| (struct_name, walk.sites))
		.collect()
}

fn process_walks<'helpers, 'file>(
	file: &syn::File,
	helpers: &'helpers HelperFunctions<'file>,
) -> HashMap<String, ProcessWalk<'helpers, 'file>> {
	let mut result = HashMap::new();

	for item in &file.items {
		let Item::Impl(item_impl) = item else {
			continue;
		};

		// Must be `impl ProcessAccountInfos for X`.
		let Some(trait_path) = item_impl.trait_.as_ref().map(|(path, _)| path) else {
			continue;
		};

		if !path_ends_with(trait_path, "ProcessAccountInfos") {
			continue;
		}

		// Get the implementing struct name.
		let struct_name = type_to_name(&item_impl.self_ty);

		// Find the `process` method.
		let Some(process_fn) = find_process_method(&item_impl.items) else {
			continue;
		};

		let mut walk = ProcessWalk::new(helpers);
		walk.stmts(&process_fn.block.stmts, &mut HashMap::new());
		result.insert(struct_name, walk);
	}

	result
}

/// Walks one `process` body, and the crate helpers it passes account fields
/// to, collecting what each account field proves and where each account call
/// is written.
struct ProcessWalk<'helpers, 'file> {
	helpers: &'helpers HelperFunctions<'file>,
	/// Helpers on the current call path, each with the index of the file it is
	/// declared in, so recursive helpers terminate and sites name their file.
	active_helpers: Vec<(String, usize)>,
	fields: HashMap<String, FieldFacts>,
	sites: Vec<AssertionSite>,
}

/// Bring the identifiers `pat` introduces into scope: each one shadows what it
/// meant before, and aliases `field_name` when the matched value is that
/// account field.
fn bind_pattern(pat: &Pat, field_name: Option<&str>, bindings: &mut HashMap<String, String>) {
	remove_pattern_idents(pat, bindings);

	if let Some(field_name) = field_name {
		bind_pattern_idents(pat, field_name, bindings);
	}
}

/// Map every identifier captured by `pat` onto `field_name` so later
/// assertions written against the local alias are attributed correctly.
fn bind_pattern_idents(pat: &Pat, field_name: &str, bindings: &mut HashMap<String, String>) {
	match pat {
		Pat::Ident(ident) => {
			bindings.insert(ident.ident.to_string(), field_name.to_owned());
		}
		Pat::Reference(reference) => bind_pattern_idents(&reference.pat, field_name, bindings),
		Pat::Guard(guarded) => bind_pattern_idents(&guarded.pat, field_name, bindings),
		Pat::Type(typed) => bind_pattern_idents(&typed.pat, field_name, bindings),
		Pat::Or(or_pattern) => {
			for alternative in &or_pattern.cases {
				bind_pattern_idents(alternative, field_name, bindings);
			}
		}
		Pat::Slice(slice) => {
			for element in &slice.elems {
				bind_pattern_idents(element, field_name, bindings);
			}
		}
		Pat::Tuple(tuple) => {
			for element in &tuple.elems {
				bind_pattern_idents(element, field_name, bindings);
			}
		}
		Pat::TupleStruct(tuple_struct) => {
			for element in &tuple_struct.elems {
				bind_pattern_idents(element, field_name, bindings);
			}
		}
		Pat::Struct(pattern_struct) => {
			for member in &pattern_struct.fields {
				bind_pattern_idents(&member.pat, field_name, bindings);
			}
		}
		_ => {}
	}
}

/// Remove every identifier introduced by `pat` from the current lexical scope.
fn remove_pattern_idents(pat: &Pat, bindings: &mut HashMap<String, String>) {
	match pat {
		Pat::Ident(ident) => {
			bindings.remove(&ident.ident.to_string());
		}
		Pat::Reference(reference) => remove_pattern_idents(&reference.pat, bindings),
		Pat::Guard(guarded) => remove_pattern_idents(&guarded.pat, bindings),
		Pat::Type(typed) => remove_pattern_idents(&typed.pat, bindings),
		Pat::Or(or_pattern) => {
			for alternative in &or_pattern.cases {
				remove_pattern_idents(alternative, bindings);
			}
		}
		Pat::Slice(slice) => {
			for element in &slice.elems {
				remove_pattern_idents(element, bindings);
			}
		}
		Pat::Tuple(tuple) => {
			for element in &tuple.elems {
				remove_pattern_idents(element, bindings);
			}
		}
		Pat::TupleStruct(tuple_struct) => {
			for element in &tuple_struct.elems {
				remove_pattern_idents(element, bindings);
			}
		}
		Pat::Struct(pattern_struct) => {
			for member in &pattern_struct.fields {
				remove_pattern_idents(&member.pat, bindings);
			}
		}
		_ => {}
	}
}

impl<'helpers, 'file> ProcessWalk<'helpers, 'file> {
	fn new(helpers: &'helpers HelperFunctions<'file>) -> Self {
		Self {
			helpers,
			active_helpers: Vec::new(),
			fields: HashMap::new(),
			sites: Vec::new(),
		}
	}

	fn field(&mut self, field_name: String) -> &mut FieldFacts {
		self.fields.entry(field_name).or_default()
	}

	/// Record a call on an account field: its client-visible effect and where
	/// it is written.
	fn record(
		&mut self,
		field_name: String,
		method: &str,
		args: &Punctuated<Expr, syn::Token![,]>,
		span: proc_macro2::Span,
	) {
		apply_assertion(method, args, &mut self.field(field_name.clone()).properties);
		self.sites.push(AssertionSite {
			field: field_name,
			method: method.to_owned(),
			span,
			helper_file: self.active_helpers.last().map(|(_, file)| *file),
		});
	}

	fn stmts(&mut self, stmts: &[Stmt], bindings: &mut HashMap<String, String>) {
		for stmt in stmts {
			self.stmt(stmt, bindings);
		}
	}

	fn stmt(&mut self, stmt: &Stmt, bindings: &mut HashMap<String, String>) {
		match stmt {
			Stmt::Expr(expr, _) => {
				self.expr(expr, bindings);
			}

			Stmt::Local(local) => {
				let field_name = local
					.init
					.as_ref()
					.and_then(|init| resolve_self_field(&init.expr, bindings));

				// The initializer, and the `else` block of a `let … else`, run in
				// the enclosing scope: a name the pattern rebinds, as in
				// `let vault = State::load_pda(vault, ..)?`, still means the old
				// binding there.
				if let Some(init) = &local.init {
					self.expr(&init.expr, bindings);
					if let Some((_, diverge)) = &init.diverge {
						self.expr(diverge, &mut bindings.clone());
					}
				}

				// Aliases such as `let escrow = self.escrow.as_ref()` capture an
				// account field, so assertions written against the alias must be
				// attributed back to the originating field. Any other name the
				// pattern introduces shadows whatever it meant before.
				bind_pattern(&local.pat, field_name.as_deref(), bindings);
			}

			_ => {}
		}
	}

	fn expr(&mut self, expr: &Expr, bindings: &mut HashMap<String, String>) {
		match expr {
			Expr::MethodCall(mc) => {
				let method = mc.method.to_string();
				let pda_target = pda_creation_target(&method, &mc.receiver, bindings);
				let field_name = resolve_self_field(&mc.receiver, bindings);

				// Rust evaluates the receiver, then the arguments, then calls the
				// method. Walking in that order records call sites in execution
				// order, so `self.a.assert_data_len(8)?.assert_writable()?` records
				// `assert_data_len` first.
				self.expr(&mc.receiver, bindings);

				for arg in &mc.args {
					self.expr(arg, bindings);
				}

				if let Some(field_name) = pda_target {
					let facts = self.field(field_name);
					facts.properties.is_pda = true;
					facts.properties.is_writable = true;
					facts.account_types.extend(turbofish_type_name(mc));
				} else if let Some(field_name) = field_name {
					if TYPED_ACCOUNT_METHODS.contains(&method.as_str()) {
						let account_types = &mut self.field(field_name.clone()).account_types;
						account_types.extend(turbofish_type_name(mc));
					}
					self.record(field_name, &method, &mc.args, mc.method.span());
				}
			}

			Expr::Try(t) => {
				self.expr(&t.expr, bindings);
			}

			Expr::Block(b) => {
				self.stmts(&b.block.stmts, &mut bindings.clone());
			}

			Expr::If(if_expr) => {
				// An `if let` alias exists only in the `then` branch, so the
				// condition binds into a copy. The `else` branch and the
				// surrounding block keep their original bindings.
				let mut then_bindings = bindings.clone();
				self.expr(&if_expr.cond, &mut then_bindings);
				self.stmts(&if_expr.then_branch.stmts, &mut then_bindings);

				if let Some((_, else_expr)) = &if_expr.else_branch {
					self.expr(else_expr, &mut bindings.clone());
				}
			}

			Expr::Let(let_expr) => {
				// The scrutinee runs before the pattern binds, in the old scope.
				let field_name = resolve_self_field(&let_expr.expr, bindings);
				self.expr(&let_expr.expr, bindings);
				bind_pattern(&let_expr.pat, field_name.as_deref(), bindings);
			}

			Expr::Match(match_expr) => {
				// The scrutinee runs first. Each arm's pattern then binds, its
				// guard (which syn keeps inside the pattern) runs with those
				// bindings, and the body of the arm that matches runs last.
				let scrutinee_field = resolve_self_field(&match_expr.expr, bindings);
				self.expr(&match_expr.expr, bindings);

				for arm in &match_expr.arms {
					let mut arm_bindings = bindings.clone();
					bind_pattern(&arm.pat, scrutinee_field.as_deref(), &mut arm_bindings);

					if let Pat::Guard(guarded) = &arm.pat {
						self.expr(&guarded.guard, &mut arm_bindings);
					}
					self.expr(&arm.body, &mut arm_bindings);
				}
			}

			Expr::Call(call) => {
				let path = match &*call.func {
					Expr::Path(path) => Some(&path.path),
					_ => None,
				};
				let pda_validation =
					path.and_then(|path| pda_validation_target(path, &call.args, bindings));
				let helper = path.and_then(|path| self.helper_target(path, &call.args, bindings));

				// The arguments run before the call, so the call and anything a
				// followed helper does are recorded after them.
				self.expr(&call.func, bindings);

				for arg in &call.args {
					self.expr(arg, bindings);
				}

				if let Some(target) = pda_validation {
					let account_types = &mut self.field(target.field.clone()).account_types;
					account_types.extend(target.account_type);
					self.record(target.field, &target.function, &call.args, target.span);
				}
				if let Some(helper) = helper {
					self.walk_helper(helper);
				}
			}

			Expr::Paren(p) => {
				self.expr(&p.expr, bindings);
			}

			Expr::Reference(r) => {
				self.expr(&r.expr, bindings);
			}

			Expr::Unary(unary) => {
				self.expr(&unary.expr, bindings);
			}

			Expr::Struct(builder) => {
				for field_name in lamport_moving_fields(builder, bindings) {
					self.field(field_name).properties.is_writable = true;
				}
				for field in &builder.fields {
					self.expr(&field.expr, bindings);
				}
			}

			_ => {}
		}
	}

	/// The crate helper a call runs, if the walk should follow it: one passed
	/// at least one account field, not already on the call path, and within the
	/// depth limit. The bindings map each parameter to the field it receives.
	fn helper_target(
		&self,
		path: &syn::Path,
		args: &Punctuated<Expr, syn::Token![,]>,
		bindings: &HashMap<String, String>,
	) -> Option<HelperCall<'file>> {
		let name = helper_function_name(path)?;
		let helper = self.helpers.get(&name)?;
		let followable = self.active_helpers.len() < MAX_HELPER_DEPTH
			&& !self
				.active_helpers
				.iter()
				.any(|(active, _)| *active == name);
		let mut parameters = HashMap::new();

		for (input, arg) in helper.function.sig.inputs.iter().zip(args) {
			if let syn::FnArg::Typed(parameter) = input
				&& let Some(field_name) = resolve_self_field(arg, bindings)
			{
				bind_pattern_idents(&parameter.pat, &field_name, &mut parameters);
			}
		}

		(followable && !parameters.is_empty()).then_some(HelperCall {
			name,
			helper,
			parameters,
		})
	}

	/// Analyse a crate helper the body passes account fields to, attributing
	/// what the helper proves about each parameter back to its field.
	fn walk_helper(&mut self, call: HelperCall<'file>) {
		let HelperCall {
			name,
			helper,
			mut parameters,
		} = call;

		self.active_helpers.push((name, helper.file));
		self.stmts(&helper.function.block.stmts, &mut parameters);
		self.active_helpers.pop();
	}
}

/// A crate helper the walk follows, with the field each parameter receives.
struct HelperCall<'file> {
	name: String,
	helper: Helper<'file>,
	parameters: HashMap<String, String>,
}

/// A recognized static PDA validation call such as
/// `State::assert_stored_bump(self.state, ..)`.
struct PdaValidation {
	field: String,
	function: String,
	/// The path segment before the function: the account type the generated
	/// function belongs to, which identifies the PDA more reliably than the
	/// field name.
	account_type: Option<String>,
	span: proc_macro2::Span,
}

fn pda_validation_target(
	path: &syn::Path,
	args: &Punctuated<Expr, syn::Token![,]>,
	bindings: &HashMap<String, String>,
) -> Option<PdaValidation> {
	let segment = path
		.segments
		.last()
		.filter(|segment| PDA_VALIDATION_CALLS.contains(&segment.ident.to_string().as_str()))?;
	let field = resolve_self_field(args.first()?, bindings)?;

	Some(PdaValidation {
		field,
		function: segment.ident.to_string(),
		account_type: path
			.segments
			.iter()
			.rev()
			.nth(1)
			.map(|owner| owner.ident.to_string()),
		span: segment.ident.span(),
	})
}

/// The name of the free function a call path can refer to.
///
/// Every segment before the function must be a module (`helpers::spend`,
/// `crate::spend`), so associated functions such as `State::seeds` are never
/// mistaken for a helper that shares their name.
fn helper_function_name(path: &syn::Path) -> Option<String> {
	let function = path.segments.last()?;
	let is_module_path = path
		.segments
		.iter()
		.take(path.segments.len() - 1)
		.all(|segment| {
			segment
				.ident
				.to_string()
				.starts_with(|first: char| first.is_ascii_lowercase())
		});

	is_module_path.then(|| function.ident.to_string())
}

/// The account type named by a method's first generic argument, such as
/// `State` in `as_account::<State>(..)`.
fn turbofish_type_name(call: &syn::ExprMethodCall) -> Option<String> {
	let syn::GenericArgument::Type(syn::Type::Path(account_type)) =
		call.turbofish.as_ref()?.args.first()?
	else {
		return None;
	};

	account_type
		.path
		.segments
		.last()
		.map(|segment| segment.ident.to_string())
}

/// Builders whose named fields debit or credit lamports, so the runtime
/// requires those accounts to be writable even when the Rust field is a
/// shared reference.
const LAMPORT_MOVING_BUILDERS: &[(&str, &[&str])] = &[
	("CreateAccount", &["from", "to"]),
	("CreateProgramAccount", &["payer"]),
	("CreateProgramAccountWithBump", &["payer"]),
	("CreateProgramAccountWithUncheckedBump", &["payer"]),
	("CreateCompactProgramAccount", &["payer"]),
	("CreateCompactProgramAccountWithBump", &["payer"]),
	("AllocateAccount", &["payer"]),
	("AllocateAccountWithNonCanonicalBump", &["payer"]),
	("Transfer", &["from", "to"]),
];

/// Account fields a lamport-moving builder literal writes to.
///
/// A payer passed as `&AccountView` still has its lamports debited by the
/// system program, and an IDL that marks it read-only produces clients whose
/// transactions fail with a privilege escalation whenever the payer is not
/// also the fee payer.
fn lamport_moving_fields(
	builder: &syn::ExprStruct,
	bindings: &HashMap<String, String>,
) -> Vec<String> {
	let name = builder
		.path
		.segments
		.last()
		.map(|segment| segment.ident.to_string())
		.unwrap_or_default();
	let Some((_, fields)) = LAMPORT_MOVING_BUILDERS
		.iter()
		.find(|(builder_name, _)| *builder_name == name)
	else {
		return Vec::new();
	};
	builder
		.fields
		.iter()
		.filter(|field| fields.contains(&member_to_string(&field.member).as_str()))
		.filter_map(|field| resolve_self_field(&field.expr, bindings))
		.collect()
}

/// Return the account field passed to a canonical PDA creation builder.
fn pda_creation_target(
	method: &str,
	receiver: &Expr,
	bindings: &HashMap<String, String>,
) -> Option<String> {
	if !PDA_CREATION_METHODS.contains(&method) {
		return None;
	}

	let Expr::Struct(builder) = receiver else {
		return None;
	};
	let builder_name = builder.path.segments.last()?.ident.to_string();

	if !PDA_CREATION_BUILDERS.contains(&builder_name.as_str()) {
		return None;
	}
	if ["account", "payer", "owner", "seeds"]
		.iter()
		.any(|required| {
			!builder
				.fields
				.iter()
				.any(|field| member_to_string(&field.member) == *required)
		}) {
		return None;
	}

	let account = builder
		.fields
		.iter()
		.find(|field| member_to_string(&field.member) == "account")?;

	resolve_self_field(&account.expr, bindings)
}

/// Walk through chained method calls and `?` to find the originating
/// `self.<field>`, following known local aliases along the way.
fn resolve_self_field(expr: &Expr, bindings: &HashMap<String, String>) -> Option<String> {
	match expr {
		Expr::Field(f) => {
			if is_self(&f.base) {
				Some(member_to_string(&f.member))
			} else {
				None
			}
		}
		Expr::Path(path) => {
			let ident = path.path.get_ident()?;
			bindings.get(&ident.to_string()).cloned()
		}
		Expr::Try(t) => resolve_self_field(&t.expr, bindings),
		Expr::MethodCall(mc) => resolve_self_field(&mc.receiver, bindings),
		Expr::Paren(p) => resolve_self_field(&p.expr, bindings),
		Expr::Reference(r) => resolve_self_field(&r.expr, bindings),
		Expr::Unary(unary) => resolve_self_field(&unary.expr, bindings),
		_ => None,
	}
}

fn is_self(expr: &Expr) -> bool {
	matches!(expr, Expr::Path(p) if p.path.is_ident("self"))
}

fn member_to_string(member: &syn::Member) -> String {
	match member {
		syn::Member::Named(ident) => ident.to_string(),
		syn::Member::Unnamed(idx) => idx.index.to_string(),
	}
}

/// Record the effect of a recognized assertion method.
fn apply_assertion(
	method: &str,
	args: &Punctuated<Expr, syn::Token![,]>,
	props: &mut AccountProperties,
) {
	match method {
		"assert_signer" => props.is_signer = true,
		"assert_writable" => props.is_writable = true,
		"assert_address" => {
			if let Some(addr) = first_arg_to_known_address(args) {
				props.default_value = Some(DefaultValueIr::PublicKey(addr));
			}
		}
		_ if PDA_VALIDATION_CALLS.contains(&method) => {
			props.is_pda = true;
			props.is_writable |= MUTABLE_PDA_LOADERS.contains(&method);
		}
		// Other assertions don't map directly to IDL properties.
		_ => {}
	}
}

/// If the first argument to `assert_address` is a known program ID reference,
/// return its base58 address.
fn first_arg_to_known_address(args: &Punctuated<Expr, syn::Token![,]>) -> Option<String> {
	let first = args.first()?;
	known_address_from_expr(first)
}

/// Resolve a known Solana program or sysvar path to its base58 address.
pub(crate) fn known_address_from_expr(expression: &Expr) -> Option<String> {
	let path_str = expr_to_path_string(expression)?;
	for &(known_path, known_addr) in KNOWN_ADDRESSES {
		if path_str.contains(known_path) {
			return Some(known_addr.to_owned());
		}
	}
	None
}

fn expr_to_path_string(expr: &Expr) -> Option<String> {
	match expr {
		Expr::Reference(r) => expr_to_path_string(&r.expr),
		Expr::Path(p) => {
			Some(
				p.path
					.segments
					.iter()
					.map(|s| s.ident.to_string())
					.collect::<Vec<_>>()
					.join("::"),
			)
		}
		_ => None,
	}
}

fn find_process_method(items: &[ImplItem]) -> Option<&syn::ImplItemFn> {
	for item in items {
		if let ImplItem::Fn(f) = item
			&& f.sig.ident == "process"
		{
			return Some(f);
		}
	}
	None
}

fn path_ends_with(path: &syn::Path, ident: &str) -> bool {
	path.segments.last().is_some_and(|seg| seg.ident == ident)
}

fn type_to_name(ty: &syn::Type) -> String {
	match ty {
		syn::Type::Path(p) => {
			p.path
				.segments
				.last()
				.map_or_else(|| "Unknown".to_owned(), |seg| seg.ident.to_string())
		}
		_ => "Unknown".to_owned(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extracts_declarative_account_properties_privately() {
		let source = r#"
			enum NotAnAccount { Value }
			struct PlainStruct { value: u8 }
			#[derive(Accounts)]
			struct TupleAccounts(&'static AccountView);
			#[derive(Accounts)]
			struct ValidateAccounts<'a> {
				#[pina(validate(signer, writable, executable, empty, not_empty))]
				authority: &'a AccountView,
				#[pina(validate(program = system::ID))]
				system_program: &'a AccountView,
				#[pina(validate(sysvar = sysvars::clock::ID))]
				clock: &'a AccountView,
				#[pina(remaining)]
				remaining: &'a [AccountView],
				#[pina(distinct)]
				payer: &'a AccountView,
				#[pina(distinct = authority)]
				recipient: &'a AccountView,
			}
		"#;
		let file = syn::parse_file(source).expect("valid Rust");
		let all = extract_declared_validation_properties(&file)
			.expect("supported declarative validation");
		let fields = &all["ValidateAccounts"];

		assert!(fields["authority"].is_signer);
		assert!(fields["authority"].is_writable);
		assert!(matches!(
			&fields["system_program"].default_value,
			Some(DefaultValueIr::PublicKey(address))
				if address == "11111111111111111111111111111111"
		));
		assert!(matches!(
			&fields["clock"].default_value,
			Some(DefaultValueIr::PublicKey(address))
				if address == "SysvarC1ock11111111111111111111111111111111"
		));
		assert!(!all.contains_key("TupleAccounts"));
		assert!(!all.contains_key("PlainStruct"));
	}

	#[test]
	fn canonicalizes_bare_and_valued_account_constraints() {
		let field: syn::Field = syn::parse_quote! {
			#[pina(validate(signer, writable, owner = ID, not_empty, error = Error::Denied))]
			#[pina(distinct)]
			#[pina(distinct = authority)]
			account: &'static AccountView
		};

		let constraints =
			canonical_account_constraints(&field.attrs).expect("supported declarative constraints");
		assert_eq!(
			constraints,
			[
				"distinct",
				"distinct=authority",
				"not_empty",
				"owner=ID",
				"signer",
				"writable"
			]
		);
	}

	#[test]
	fn rejects_unknown_declarative_account_option() {
		let field: syn::Field = syn::parse_quote! {
			#[pina(validte(signer))]
			authority: &'static AccountView
		};
		let error = extract_attribute_properties(&field.attrs)
			.expect_err("an unknown outer helper option must fail");
		let message = error.to_string();

		assert!(message.contains("unknown `#[pina]` account-field option"));
		assert!(message.contains("`validate(...)`"));
		assert!(message.contains("`remaining`"));
		assert!(message.contains("`distinct`"));

		let error = canonical_account_constraints(&field.attrs)
			.expect_err("an unknown compatibility constraint must fail");
		assert!(error.to_string().contains("unknown account-field option"));
	}

	#[test]
	fn extracts_signer_and_writable() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					self.authority.assert_signer()?;
					self.counter.assert_writable()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["authority"].is_signer);
		assert!(!props["authority"].is_writable);
		assert!(props["counter"].is_writable);
		assert!(!props["counter"].is_signer);
	}

	#[test]
	fn extracts_chained_assertions() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					self.sender.assert_signer()?.assert_writable()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["sender"].is_signer);
		assert!(props["sender"].is_writable);
	}

	/// Every call site of `MyAccounts` as `(field, method, line, helper file)`.
	fn sites_of(files: &[&syn::File]) -> Vec<(String, String, usize, Option<usize>)> {
		let helpers = HelperFunctions::collect(files);
		let all = extract_assertion_sites(files[0], &helpers);

		all["MyAccounts"]
			.iter()
			.map(|site| {
				(
					site.field.clone(),
					site.method.clone(),
					site.span.start().line,
					site.helper_file,
				)
			})
			.collect()
	}

	#[test]
	fn records_account_call_sites_in_execution_order() {
		let source = r"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					self.sender.assert_signer()?
						.assert_writable()?;
					let vault = self.vault;
					CounterState::load_pda(vault, self.authority.assert_signer()?.address())?;
					self.vault.assert_owner(self.mint.assert_executable()?.address())?;
					Ok(())
				}
			}
		";
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let sites = sites_of(&[&file]);
		let site =
			|field: &str, method: &str, line| (field.to_owned(), method.to_owned(), line, None);

		assert_eq!(
			sites,
			[
				site("sender", "assert_signer", 4),
				site("sender", "assert_writable", 5),
				site("authority", "assert_signer", 7),
				site("authority", "address", 7),
				site("vault", "load_pda", 7),
				site("mint", "assert_executable", 8),
				site("mint", "address", 8),
				site("vault", "assert_owner", 8),
			]
		);

		// Walking order does not change the client-visible properties.
		let properties = extract_validation_properties(&file);
		let properties = &properties["MyAccounts"];
		assert!(properties["sender"].is_signer && properties["sender"].is_writable);
		assert!(properties["vault"].is_pda);
		assert!(properties["authority"].is_signer);
	}

	#[test]
	fn records_helper_call_sites_at_the_helpers_own_lines() {
		let process = r"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					self.payer.assert_writable()?;
					require_viewer(self.viewer, self.note.assert_owner(&ID)?)?;
					local_check(self.payer)?;
					self.viewer.assert_executable()?;
					Ok(())
				}
			}

			fn local_check(account: &AccountView) -> ProgramResult {
				account.assert_empty()?;
				Ok(())
			}
		";
		let helpers = r"
			fn require_viewer(viewer: &AccountView, _note: &AccountView) -> ProgramResult {
				viewer.assert_signer()?;
				EscrowState::assert_stored_bump(viewer, 7, &ID)?;
				Ok(())
			}
		";
		let process = syn::parse_file(process).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let helpers = syn::parse_file(helpers).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let site = |field: &str, method: &str, line, file| {
			(field.to_owned(), method.to_owned(), line, file)
		};

		assert_eq!(
			sites_of(&[&process, &helpers]),
			[
				site("payer", "assert_writable", 4, None),
				// The arguments run first, then the helper's body, in its file.
				site("note", "assert_owner", 5, None),
				site("viewer", "assert_signer", 3, Some(1)),
				site("viewer", "assert_stored_bump", 4, Some(1)),
				// A helper in the body's own file names that file's index.
				site("payer", "assert_empty", 13, Some(0)),
				site("viewer", "assert_executable", 7, None),
			]
		);
	}

	#[test]
	fn extracts_pda() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					self.counter
						.assert_empty()?
						.assert_writable()?
						.assert_seeds_with_bump(seeds, &ID)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["counter"].is_pda);
		assert!(props["counter"].is_writable);
	}

	#[test]
	fn extracts_pda_from_generated_static_assert() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					CounterState::assert_seeds(self.counter, authority_key, &ID)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["counter"].is_pda);
	}

	#[test]
	fn extracts_pda_and_writable_from_generated_one_pass_loader() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let mut counter = CounterState::load_pda_mut(
						self.counter,
						self.authority.address(),
						&ID,
					)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["counter"].is_pda);
		assert!(props["counter"].is_writable);
	}

	#[test]
	fn extracts_pda_from_generated_compact_loader() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					CounterState::with_stored_bump_pda(
						self.counter,
						self.authority.address(),
						&ID,
						|state| Ok(state.value),
					)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];

		assert!(props["counter"].is_pda);
		assert!(!props["counter"].is_writable);
	}

	#[test]
	fn extracts_pda_from_generated_checked_compact_loader() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					CounterState::with_checked_pda(
						self.counter,
						self.authority.address(),
						&ID,
						|state| Ok(state.value),
					)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];

		assert!(props["counter"].is_pda);
		assert!(!props["counter"].is_writable);
	}

	#[test]
	fn extracts_pda_from_canonical_creation_builders() {
		for builder in PDA_CREATION_BUILDERS {
			let source = format!(
				r#"
				impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {{
					fn process(self, data: &[u8]) -> ProgramResult {{
						{builder} {{
							account: self.counter,
							payer: self.authority,
							owner: &ID,
							seeds: &seeds,
						}}
						.invoke::<CounterState>()?;
						Ok(())
					}}
				}}
				"#,
			);
			let file =
				syn::parse_file(&source).unwrap_or_else(|error| panic!("parse failed: {error}"));
			let all = extract_validation_properties(&file);
			let props = &all["MyAccounts"]["counter"];

			assert!(props.is_pda, "{builder} must identify its target as a PDA");
			assert!(
				props.is_writable,
				"{builder} must identify its target as writable"
			);
		}
	}

	#[test]
	fn lamport_moving_builders_mark_payers_and_recipients_writable() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let funder = self.funder;
					CreateProgramAccountWithUncheckedBump {
						account: self.counter,
						payer: self.authority,
						owner: &ID,
						seeds: &seeds,
						bump: 1,
					}
					.invoke::<CounterState>()?;
					system::instructions::Transfer {
						from: funder,
						to: self.recipient,
						lamports: 1,
					}
					.invoke()?;
					Unrelated { from: self.bystander }.invoke()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|error| panic!("parse failed: {error}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];

		assert!(
			props["authority"].is_writable,
			"a creation payer is debited"
		);
		assert!(props["funder"].is_writable, "a transfer source is debited");
		assert!(
			props["recipient"].is_writable,
			"a transfer target is credited"
		);
		assert!(
			!props.contains_key("bystander"),
			"unrelated builders must not change writability"
		);
	}

	#[test]
	fn ignores_unrelated_builders_and_non_invocation_methods() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					UnrelatedBuilder { account: self.first }.invoke()?;
					CreateProgramAccount { account: self.second }.inspect()?;
					CreateProgramAccount { account: self.third }.invoke()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|error| panic!("parse failed: {error}"));
		let all = extract_validation_properties(&file);

		assert!(
			all["MyAccounts"]
				.values()
				.all(|properties| !properties.is_pda),
			"only a canonical creation builder invocation may establish PDA metadata"
		);
	}

	#[test]
	fn ignores_invoke_on_non_struct_receiver() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let builder = make_builder();
					builder.invoke::<CounterState>()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|error| panic!("parse failed: {error}"));
		let all = extract_validation_properties(&file);

		assert!(
			all["MyAccounts"]
				.values()
				.all(|properties| !properties.is_pda),
			"a non-struct receiver on an invoke method must not establish PDA metadata"
		);
	}

	#[test]
	fn ignores_static_assert_with_non_self_first_arg() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					CounterState::assert_seeds(&other_account, authority_key, &ID)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(
			props.values().all(|p| !p.is_pda),
			"static asserts on non-self accounts must not mark fields as PDAs"
		);
	}

	#[test]
	fn ignores_static_assert_with_no_args() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					CounterState::assert_seeds()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(
			props.values().all(|p| !p.is_pda),
			"arg-less static asserts must not mark accounts as PDAs"
		);
	}

	#[test]
	fn ignores_calls_with_non_path_func() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					(|| Ok(()))()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(
			props.values().all(|p| !p.is_pda),
			"non-path call funcs must not mark accounts as PDAs"
		);
	}

	#[test]
	fn ignores_non_assert_static_calls() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let seeds = CounterState::seeds(authority_key);
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(
			props.values().all(|p| !p.is_pda),
			"non-assert static calls must not mark accounts as PDAs"
		);
	}

	#[test]
	fn extracts_pda_from_generated_static_assert_with_reference() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					VestingState::assert_seeds(&self.vesting_state, &admin, &beneficiary, &mint, &ID)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["vesting_state"].is_pda);
	}

	#[test]
	fn extracts_known_address() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					self.system_program.assert_address(&system::ID)?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(matches!(
			&props["system_program"].default_value,
			Some(DefaultValueIr::PublicKey(addr)) if addr == "11111111111111111111111111111111"
		));
	}

	#[test]
	fn extracts_assertions_from_if_let_some_binding() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					if let Some(escrow) = &self.escrow {
						escrow.assert_signer()?.assert_writable()?;
					}
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["escrow"].is_signer);
		assert!(props["escrow"].is_writable);
	}

	#[test]
	fn extracts_assertions_from_let_binding_with_method_call() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let witness = self.witness.as_ref();
					if let Some(witness) = witness {
						witness.assert_signer()?;
					}
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		assert!(all["MyAccounts"]["witness"].is_signer);
	}

	#[test]
	fn extracts_assertions_from_match_on_optional_field() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					match self.optional {
						Some(account) => account.assert_writable()?,
						None => {}
					}
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		assert!(all["MyAccounts"]["optional"].is_writable);
	}

	#[test]
	fn extracts_default_address_from_if_let_binding() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					if let Some(system) = self.system_program.as_ref() {
						system.assert_address(&system::ID)?;
					}
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		assert!(matches!(
			&all["MyAccounts"]["system_program"].default_value,
			Some(DefaultValueIr::PublicKey(addr)) if addr == "11111111111111111111111111111111"
		));
	}

	/// A local alias must not leak assertions onto unrelated fields that
	/// happen to share the alias name in a sibling scope.
	#[test]
	fn binding_does_not_attribute_unrelated_assertions() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					if let Some(escrow) = &self.escrow {
						escrow.assert_signer()?;
					}
					let escrow = unrelated;
					escrow.assert_writable()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert_eq!(props.len(), 1, "only the bound field gains properties");
		assert!(props["escrow"].is_signer);
		assert!(!props["escrow"].is_writable);
	}

	#[test]
	fn if_let_alias_is_not_visible_in_the_else_branch() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let account = self.authority;
					if let Some(account) = &self.optional {
						account.assert_signer()?;
					} else {
						account.assert_writable()?;
					}
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		let props = &all["MyAccounts"];
		assert!(props["optional"].is_signer);
		assert!(!props["optional"].is_writable);
		assert!(props["authority"].is_writable);
		assert!(!props["authority"].is_signer);
	}

	#[test]
	fn ignores_assertions_in_non_executed_declarations_and_control_flow() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					while false {
						self.optional.assert_writable()?;
					}
					let deferred = || {
						self.authority.assert_signer()?;
						Ok(())
					};
					impl Helper {
						fn deferred(self) -> ProgramResult {
							self.counter.assert_writable()?;
							Ok(())
						}
					}
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		assert!(all["MyAccounts"].is_empty());
	}

	#[test]
	fn traverses_let_else_and_parenthesized_aliases() {
		let source = r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let shadowed;
					let typed: &AccountView = self.typed;
					typed.assert_signer()?;
					let Some(account) = self.optional else {
						self.fallback.assert_signer()?;
					};
					(account).assert_writable()?;
					Ok(())
				}
			}
		"#;
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let all = extract_validation_properties(&file);
		assert!(all["MyAccounts"]["optional"].is_writable);
		assert!(all["MyAccounts"]["fallback"].is_signer);
		assert!(all["MyAccounts"]["typed"].is_signer);
	}

	#[test]
	fn recursively_tracks_identifiers_in_supported_patterns() {
		use syn::parse::Parser as _;

		for (source, names) in [
			("account", &["account"][..]),
			("&account", &["account"][..]),
			("left | right", &["left", "right"][..]),
			("[first, second]", &["first", "second"][..]),
			("(first, second)", &["first", "second"][..]),
			("Some(account)", &["account"][..]),
			("Shape { account }", &["account"][..]),
		] {
			let pattern = Pat::parse_multi
				.parse_str(source)
				.unwrap_or_else(|error| panic!("failed to parse {source}: {error}"));
			let mut bindings = HashMap::new();
			bind_pattern_idents(&pattern, "field", &mut bindings);
			for name in names {
				assert_eq!(bindings.get(*name).map(String::as_str), Some("field"));
			}

			remove_pattern_idents(&pattern, &mut bindings);
			assert!(bindings.is_empty());
		}
	}

	/// Facts for every field of `MyAccounts`, following helpers in `source`.
	fn facts_for(source: &str) -> HashMap<String, FieldFacts> {
		let file = syn::parse_file(source).unwrap_or_else(|error| panic!("parse failed: {error}"));
		let helpers = HelperFunctions::collect(&[&file]);
		let fields = extract_validation_facts(&file, &helpers).remove("MyAccounts");

		fields.unwrap_or_else(|| panic!("`MyAccounts` has a process body"))
	}

	fn account_types(facts: &FieldFacts) -> Vec<&str> {
		facts.account_types.iter().map(String::as_str).collect()
	}

	#[test]
	fn extracts_pda_and_account_type_from_stored_bump_assertion() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let bump = self.escrow.as_account::<EscrowState>(&ID)?.bump;
					EscrowState::assert_stored_bump(self.escrow, bump, &maker, seed, &ID)?;
					Ok(())
				}
			}
		"#,
		);
		let escrow = &fields["escrow"];

		assert!(escrow.properties.is_pda);
		assert!(!escrow.properties.is_writable);
		assert_eq!(account_types(escrow), ["EscrowState"]);
	}

	#[test]
	fn checked_fixed_loaders_mark_pdas_and_mutable_loaders_writable() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					ConfigState::load_checked_pda(self.reader, &ID)?;
					ConfigState::load_checked_pda_mut(self.writer, &ID)?;
					Ok(())
				}
			}
		"#,
		);

		assert!(fields["reader"].properties.is_pda);
		assert!(!fields["reader"].properties.is_writable);
		assert!(fields["writer"].properties.is_pda);
		assert!(fields["writer"].properties.is_writable);
		assert_eq!(account_types(&fields["writer"]), ["ConfigState"]);
	}

	#[test]
	fn typed_account_methods_record_types_without_proving_pdas() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					self.fixed.as_account::<Fixed>(&ID)?;
					self.fixed_mut.as_account_mut::<state::FixedMut>(&ID)?;
					self.compact.with_compact_account::<Compact, _>(&ID, |state| Ok(()))?;
					self.patched.update_compact_account::<Patched>(&ID, &patch)?;
					self.asserted.assert_type::<Asserted>(&ID)?;
					self.compact_asserted.assert_compact_type::<CompactAsserted>(&ID)?;
					self.untyped.as_account(&ID)?;
					self.referenced.as_account::<&Referenced>(&ID)?;
					self.unrelated.borrow::<Unrelated>()?;
					Ok(())
				}
			}
		"#,
		);
		let typed = [
			("fixed", "Fixed"),
			("fixed_mut", "FixedMut"),
			("compact", "Compact"),
			("patched", "Patched"),
			("asserted", "Asserted"),
			("compact_asserted", "CompactAsserted"),
		];

		for (field, account_type) in typed {
			assert_eq!(account_types(&fields[field]), [account_type], "{field}");
			assert!(!fields[field].properties.is_pda, "{field}");
		}
		for field in ["untyped", "referenced", "unrelated"] {
			assert!(fields[field].account_types.is_empty(), "{field}");
		}
	}

	#[test]
	fn creation_builders_record_the_created_account_type() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					CreateProgramAccountWithBump {
						account: self.tree,
						payer: self.authority,
						owner: &ID,
						seeds: &MerkleTree::seeds().as_slices(),
						bump: 255,
					}
					.invoke_with::<MerkleTree>(|tree| Ok(()))?;
					Ok(())
				}
			}
		"#,
		);

		assert!(fields["tree"].properties.is_pda);
		assert_eq!(account_types(&fields["tree"]), ["MerkleTree"]);
	}

	#[test]
	fn follows_helpers_that_receive_account_fields() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					spend(self.tree, &mut self.nullifiers, self.treasury.as_deref_mut())?;
					crate::helpers::require_viewer(self.viewer)?;
					Ok(())
				}
			}

			fn spend(
				tree: &AccountView,
				nullifiers: &mut AccountView,
				treasury: Option<&mut AccountView>,
			) -> ProgramResult {
				tree.as_account::<MerkleTree>(&ID)?;
				NullifierSet::assert_seeds(nullifiers, &ID)?;
				let treasury = treasury.ok_or(ProgramError::NotEnoughAccountKeys)?;
				treasury.assert_writable()?;
				Ok(())
			}

			fn require_viewer(viewer: &AccountView) -> ProgramResult {
				viewer.assert_signer()?;
				Ok(())
			}
		"#,
		);

		assert_eq!(account_types(&fields["tree"]), ["MerkleTree"]);
		assert!(fields["nullifiers"].properties.is_pda);
		assert!(fields["treasury"].properties.is_writable);
		assert!(fields["viewer"].properties.is_signer);
	}

	#[test]
	fn does_not_follow_ambiguous_associated_or_unbound_helpers() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					duplicated(self.duplicate)?;
					State::associated(self.associated)?;
					associated(self.unbound_argument_free)?;
					unbound(LOCAL_ACCOUNT)?;
					missing(self.missing)?;
					self.associated.assert_owner(&ID)?;
					Ok(())
				}
			}

			fn duplicated(account: &AccountView) -> ProgramResult {
				account.assert_signer()?;
				Ok(())
			}

			fn duplicated(account: &AccountView) -> ProgramResult {
				account.assert_signer()?;
				Ok(())
			}

			fn associated() -> ProgramResult {
				SIGNER.assert_signer()?;
				Ok(())
			}

			fn unbound(account: &AccountView) -> ProgramResult {
				account.assert_signer()?;
				Ok(())
			}
		"#,
		);

		assert!(fields.get("duplicate").is_none());
		assert!(!fields["associated"].properties.is_signer);
		assert!(fields.get("unbound_argument_free").is_none());
		assert!(fields.get("missing").is_none());
		assert!(fields.values().all(|facts| !facts.properties.is_signer));
	}

	#[test]
	fn follows_helpers_inside_match_arms_on_local_values() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					match data.len() {
						0 => require_signer(*self.flagged)?,
						_ => {}
					}
					Ok(())
				}
			}

			fn require_signer(account: AccountView) -> ProgramResult {
				account.assert_signer()?;
				Ok(())
			}
		"#,
		);

		assert!(fields["flagged"].properties.is_signer);
	}

	#[test]
	fn helper_let_initializers_use_the_parameter_they_shadow() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					check(self.state)?;
					Ok(())
				}
			}

			fn check(state: &AccountView) -> ProgramResult {
				let state = State::load_checked_pda(state, &ID)?;
				Ok(())
			}
		"#,
		);

		assert!(fields["state"].properties.is_pda);
		assert_eq!(account_types(&fields["state"]), ["State"]);
	}

	#[test]
	fn helper_let_initializers_follow_helpers_on_the_parameter_they_shadow() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					authorize(self.authority)?;
					Ok(())
				}
			}

			fn authorize(account: &AccountView) -> ProgramResult {
				let account = require_signer(account)?;
				account.assert_writable()?;
				Ok(())
			}

			fn require_signer(account: &AccountView) -> Result<&AccountView, ProgramError> {
				account.assert_signer()
			}
		"#,
		);
		let authority = &fields["authority"].properties;

		assert!(authority.is_signer);
		// The returned value is a new local, not a known account field.
		assert!(!authority.is_writable);
	}

	#[test]
	fn process_let_initializers_use_the_alias_they_shadow() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let vault = self.vault;
					let vault = PoolVault::load_pda_mut(vault, &ID)?;
					vault.assert_signer()?;
					Ok(())
				}
			}
		"#,
		);
		let vault = &fields["vault"];

		assert!(vault.properties.is_pda && vault.properties.is_writable);
		assert_eq!(account_types(vault), ["PoolVault"]);
		assert!(!vault.properties.is_signer);
	}

	#[test]
	fn let_else_blocks_run_before_the_pattern_binds() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let payer = self.payer;
					let Some(payer) = lookup(data) else {
						payer.assert_signer()?;
						let payer = self.fallback;
						return Err(ProgramError::InvalidArgument);
					};
					payer.assert_writable()?;
					let Some(escrow) = self.escrow.as_ref() else {
						return Err(ProgramError::NotEnoughAccountKeys);
					};
					escrow.assert_writable()?;
					Ok(())
				}
			}
		"#,
		);

		assert!(fields["payer"].properties.is_signer);
		assert!(!fields["payer"].properties.is_writable);
		assert!(fields.get("fallback").is_none());
		assert!(fields["escrow"].properties.is_writable);
	}

	#[test]
	fn if_let_patterns_shadow_names_only_after_the_condition() {
		let source = r"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					let vault = self.vault;
					if let Some(vault) = lookup(vault.assert_writable()?) {
						vault.assert_signer()?;
					}
					vault.assert_owner(&ID)?;
					Ok(())
				}
			}
		";
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let site = |method: &str, line| ("vault".to_owned(), method.to_owned(), line, None);

		assert_eq!(
			sites_of(&[&file]),
			[site("assert_writable", 5), site("assert_owner", 8)]
		);
	}

	#[test]
	fn match_scrutinees_run_before_guards_and_arms() {
		let source = r"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					match self.config.assert_owner(&ID)?.address() {
						_ => self.payer.assert_signer()?,
					}
					match self.escrow.as_ref() {
						Some(escrow) if escrow.assert_writable().is_ok() => escrow.assert_signer()?,
						_ => {}
					}
					Ok(())
				}
			}
		";
		let file = syn::parse_file(source).unwrap_or_else(|e| panic!("parse failed: {e}"));
		let site =
			|field: &str, method: &str, line| (field.to_owned(), method.to_owned(), line, None);

		assert_eq!(
			sites_of(&[&file]),
			[
				site("config", "assert_owner", 4),
				site("config", "address", 4),
				site("payer", "assert_signer", 5),
				site("escrow", "as_ref", 7),
				site("escrow", "assert_writable", 8),
				site("escrow", "is_ok", 8),
				site("escrow", "assert_signer", 8),
			]
		);
	}

	#[test]
	fn helper_analysis_stops_at_recursion_and_the_depth_limit() {
		let fields = facts_for(
			r#"
			impl<'a> ProcessAccountInfos<'a> for MyAccounts<'a> {
				fn process(self, data: &[u8]) -> ProgramResult {
					recurse(self.looped)?;
					first(self.deep)?;
					Ok(())
				}
			}

			fn recurse(account: &AccountView) -> ProgramResult {
				account.assert_signer()?;
				recurse(account)
			}

			fn first(account: &AccountView) -> ProgramResult { second(account) }
			fn second(account: &AccountView) -> ProgramResult { third(account) }
			fn third(account: &AccountView) -> ProgramResult { fourth(account) }

			fn fourth(account: &AccountView) -> ProgramResult {
				account.assert_signer()?;
				fifth(account)
			}

			fn fifth(account: &AccountView) -> ProgramResult {
				account.assert_writable()?;
				Ok(())
			}
		"#,
		);

		assert!(fields["looped"].properties.is_signer);
		assert!(fields["deep"].properties.is_signer);
		assert!(
			!fields["deep"].properties.is_writable,
			"a helper beyond the depth limit is not analysed"
		);
	}
}
