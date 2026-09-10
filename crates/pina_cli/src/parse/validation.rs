use std::collections::HashMap;

use quote::ToTokens as _;
use syn::Expr;
use syn::ImplItem;
use syn::Item;
use syn::Pat;
use syn::Stmt;

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

/// Client properties declared by annotations or inferred from a validation
/// chain for one account field.
#[derive(Debug, Clone, Default)]
pub struct AccountProperties {
	pub is_signer: bool,
	pub is_writable: bool,
	pub is_pda: bool,
	pub default_value: Option<DefaultValueIr>,
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
pub fn extract_validation_properties(
	file: &syn::File,
) -> HashMap<String, HashMap<String, AccountProperties>> {
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

		let props = analyse_process_body(&process_fn.block.stmts);
		result.insert(struct_name, props);
	}

	result
}

/// Walk the statements in a `process()` body and collect assertions per field.
fn analyse_process_body(stmts: &[Stmt]) -> HashMap<String, AccountProperties> {
	let mut props: HashMap<String, AccountProperties> = HashMap::new();
	let mut bindings: HashMap<String, String> = HashMap::new();

	for stmt in stmts {
		collect_assertions_from_stmt(stmt, &mut props, &mut bindings);
	}

	props
}

fn collect_assertions_from_stmt(
	stmt: &Stmt,
	props: &mut HashMap<String, AccountProperties>,
	bindings: &mut HashMap<String, String>,
) {
	match stmt {
		Stmt::Expr(expr, _) => {
			collect_assertions_from_expr(expr, props, bindings);
		}

		Stmt::Local(local) => {
			let field_name = local
				.init
				.as_ref()
				.and_then(|init| resolve_self_field(&init.expr, bindings));
			remove_pattern_idents(&local.pat, bindings);

			// Aliases such as `if let Some(escrow) = &self.escrow` or
			// `let escrow = self.escrow.as_ref()` capture an account field, so
			// assertions written against the alias must be attributed back to
			// the originating field.
			if let Some(field_name) = field_name {
				bind_pattern_idents(&local.pat, &field_name, bindings);
			}

			let Some(init) = &local.init else {
				return;
			};

			collect_assertions_from_expr(&init.expr, props, bindings);
			if let Some((_, diverge)) = &init.diverge {
				collect_assertions_from_expr(diverge, props, bindings);
			}
		}

		_ => {}
	}
}

/// Bind pattern identifiers for `let <pat> = <expr>` forms where `<expr>`
/// resolves to an account field.
fn bind_let_bindings(expr: &Expr, bindings: &mut HashMap<String, String>) {
	let Expr::Let(let_expr) = expr else {
		return;
	};

	if let Some(field_name) = resolve_self_field(&let_expr.expr, bindings) {
		bind_pattern_idents(&let_expr.pat, &field_name, bindings);
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

fn collect_assertions_from_expr(
	expr: &Expr,
	props: &mut HashMap<String, AccountProperties>,
	bindings: &mut HashMap<String, String>,
) {
	match expr {
		Expr::MethodCall(mc) => {
			let method = mc.method.to_string();

			if let Some(field_name) = pda_creation_target(&method, &mc.receiver, bindings) {
				let entry = props.entry(field_name).or_default();
				entry.is_pda = true;
				entry.is_writable = true;
			} else if let Some(field_name) = resolve_self_field(&mc.receiver, bindings) {
				let entry = props.entry(field_name).or_default();
				apply_assertion(&method, &mc.args, entry);
			}

			// Also recurse into the receiver (for chained calls).
			collect_assertions_from_expr(&mc.receiver, props, bindings);

			// And recurse into arguments.
			for arg in &mc.args {
				collect_assertions_from_expr(arg, props, bindings);
			}
		}

		Expr::Try(t) => {
			collect_assertions_from_expr(&t.expr, props, bindings);
		}

		Expr::Block(b) => {
			let mut block_bindings = bindings.clone();
			for stmt in &b.block.stmts {
				collect_assertions_from_stmt(stmt, props, &mut block_bindings);
			}
		}

		Expr::If(if_expr) => {
			// An `if let` alias exists only in the `then` branch. The `else`
			// branch and surrounding block retain their original bindings.
			let mut then_bindings = bindings.clone();
			bind_let_bindings(&if_expr.cond, &mut then_bindings);
			collect_assertions_from_expr(&if_expr.cond, props, &mut then_bindings);

			for stmt in &if_expr.then_branch.stmts {
				collect_assertions_from_stmt(stmt, props, &mut then_bindings);
			}

			if let Some((_, else_expr)) = &if_expr.else_branch {
				let mut else_bindings = bindings.clone();
				collect_assertions_from_expr(else_expr, props, &mut else_bindings);
			}
		}

		Expr::Let(let_expr) => {
			bind_let_bindings(expr, bindings);
			collect_assertions_from_expr(&let_expr.expr, props, bindings);
		}

		Expr::Match(match_expr) => {
			let scrutinee_field = resolve_self_field(&match_expr.expr, bindings);

			for arm in &match_expr.arms {
				let mut arm_bindings = bindings.clone();
				if let Some(field_name) = &scrutinee_field {
					bind_pattern_idents(&arm.pat, field_name, &mut arm_bindings);
				}

				collect_assertions_from_expr(&arm.body, props, &mut arm_bindings);
			}

			collect_assertions_from_expr(&match_expr.expr, props, bindings);
		}

		Expr::Call(call) => {
			// Recognize generated static calls from the `#[pda]` attribute
			// macro. Stored-bump assertions and one-pass loaders all mark the
			// account as a PDA; the mutable loader also proves writability.
			if let Expr::Path(path) = &*call.func {
				let method = path.path.segments.last().map(|s| s.ident.to_string());
				if matches!(
					method.as_deref(),
					Some(
						"assert_seeds"
							| "assert_seeds_with_bump"
							| "assert_canonical_bump"
							| "load_pda" | "load_pda_mut"
							| "with_pda"
					)
				) && let Some(first_arg) = call.args.first()
					&& let Some(field_name) = resolve_self_field(first_arg, bindings)
				{
					let entry = props.entry(field_name).or_default();
					apply_assertion(method.as_deref().unwrap_or_default(), &call.args, entry);
				}
			}

			collect_assertions_from_expr(&call.func, props, bindings);

			for arg in &call.args {
				collect_assertions_from_expr(arg, props, bindings);
			}
		}

		Expr::Paren(p) => {
			collect_assertions_from_expr(&p.expr, props, bindings);
		}

		Expr::Reference(r) => {
			collect_assertions_from_expr(&r.expr, props, bindings);
		}

		Expr::Unary(unary) => {
			collect_assertions_from_expr(&unary.expr, props, bindings);
		}

		_ => {}
	}
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
	args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
	props: &mut AccountProperties,
) {
	match method {
		"assert_signer" => props.is_signer = true,
		"assert_writable" => props.is_writable = true,
		"assert_seeds"
		| "assert_seeds_with_bump"
		| "assert_canonical_bump"
		| "load_pda"
		| "with_pda" => {
			props.is_pda = true;
		}
		"load_pda_mut" => {
			props.is_pda = true;
			props.is_writable = true;
		}
		"assert_address" => {
			if let Some(addr) = first_arg_to_known_address(args) {
				props.default_value = Some(DefaultValueIr::PublicKey(addr));
			}
		}
		// Other assertions don't map directly to IDL properties.
		_ => {}
	}
}

/// If the first argument to `assert_address` is a known program ID reference,
/// return its base58 address.
fn first_arg_to_known_address(
	args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
) -> Option<String> {
	let first = args.first()?;
	known_address_from_expr(first)
}

/// Resolve a known Solana program or sysvar path to its base58 address.
fn known_address_from_expr(expression: &Expr) -> Option<String> {
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
					CounterState::with_pda(
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
}
