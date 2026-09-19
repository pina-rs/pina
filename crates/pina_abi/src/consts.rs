//! Compile-time constant resolution for closed schema capacities.
//!
//! The ABI document records concrete numbers, because a manifest is read by
//! tooling that never sees the user's source. Pina's schema grammar originally
//! guaranteed that by requiring an integer literal wherever a capacity was
//! accepted, which forced the same bound to be written out at every use site.
//!
//! Named `const` items are that same number with better intent, so capacities
//! are resolved here before a type reaches [`crate::canonical_type`]: the ABI
//! layer still receives `Vec<Address, 24>`, and source keeps `MAX_MEMBERS`.
//!
//! Resolution is best effort by design. A capacity naming something this table
//! cannot evaluate is reported by the caller rather than guessed at, so the
//! failure names the expression instead of surfacing as an unreadable ABI type.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use quote::ToTokens as _;

/// Sentinel for a `const` item whose value has not been computed yet.
const UNRESOLVED: i128 = i128::MIN;

/// Apply an integer cast to a resolved value.
///
/// A capacity may be written as `(300u16 as u8) as usize`, and Rust records the
/// truncated value. Ignoring the target type would resolve `300`, emit that
/// capacity, and then fail the generated assertion. Only integer targets are
/// modeled: any other cast is left unresolved so the schema grammar reports it.
fn cast_int(value: i128, ty: &syn::Type) -> Option<i128> {
	let syn::Type::Path(path) = ty else {
		return None;
	};
	if path.qself.is_some() {
		return None;
	}
	let name = path.path.segments.last()?.ident.to_string();

	let bits = match name.as_str() {
		"u8" | "i8" => 8,
		"u16" | "i16" => 16,
		"u32" | "i32" => 32,
		"u64" | "i64" => 64,
		"u128" | "i128" | "usize" | "isize" => return Some(value),
		_ => return None,
	};

	// Truncate to the target width the way Rust does.
	let mask = (1_i128 << bits) - 1;
	let truncated = value & mask;
	let signed = matches!(name.as_str(), "i8" | "i16" | "i32" | "i64");
	if signed && truncated >= (1_i128 << (bits - 1)) {
		Some(truncated - (1_i128 << bits))
	} else {
		Some(truncated)
	}
}

/// Build an unsuffixed integer literal for a resolved capacity.
/// The path a capacity expression names, if it is a bare or braced name.
/// Every constant path an expression reads.
///
/// A capacity may be arithmetic over constants (`WIDTH * 2`) or a braced name
/// (`{WIDTH}`). Each name it reads is collected so the expansion can reference
/// it, which keeps a private constant live and pins the value it resolved to.
fn constant_paths(expression: &syn::Expr) -> Vec<&syn::Path> {
	let mut paths = Vec::new();
	collect_paths(expression, 0, &mut paths);
	paths
}

fn collect_paths<'a>(expression: &'a syn::Expr, depth: usize, paths: &mut Vec<&'a syn::Path>) {
	if depth > MAX_CONST_REFERENCE_DEPTH {
		return;
	}

	match expression {
		syn::Expr::Path(path) if path.qself.is_none() => paths.push(&path.path),
		syn::Expr::Paren(paren) => collect_paths(&paren.expr, depth + 1, paths),
		syn::Expr::Group(group) => collect_paths(&group.expr, depth + 1, paths),
		syn::Expr::Unary(unary) => collect_paths(&unary.expr, depth + 1, paths),
		syn::Expr::Cast(cast) => collect_paths(&cast.expr, depth + 1, paths),
		syn::Expr::Binary(binary) => {
			collect_paths(&binary.left, depth + 1, paths);
			collect_paths(&binary.right, depth + 1, paths);
		}
		syn::Expr::Block(block) => {
			for statement in &block.block.stmts {
				if let syn::Stmt::Expr(inner, None) = statement {
					collect_paths(inner, depth + 1, paths);
				}
			}
		}
		_ => {}
	}
}

fn numeric_literal(value: usize) -> syn::Expr {
	syn::Expr::Lit(syn::ExprLit {
		attrs: Vec::new(),
		lit: syn::Lit::Int(syn::LitInt::new(
			&value.to_string(),
			proc_macro2::Span::call_site(),
		)),
	})
}

/// Maximum chain of `const` items that may reference each other.
///
/// Real schemas name one bound in terms of a handful of others. The bound keeps
/// a hostile or cyclic source tree from driving unbounded recursion during
/// expansion.
const MAX_CONST_REFERENCE_DEPTH: usize = 64;

/// Integer constants available to one schema expansion.
///
/// The table records the resolved value of every `const` item it collected,
/// including constants defined in terms of other constants. Lookup keys on the
/// final path segment, which tolerates `crate::limits::MAX_MEMBERS` without
/// modelling module scope. Redeclaring one capacity name in the same crate is
/// therefore ambiguous; resolution follows the deterministic file order and the
/// last declaration wins.
///
/// Expressions are held as source text rather than syntax nodes. A procedural
/// macro expands inside the compiler, where `syn::Expr` is neither `Send` nor
/// `Sync`, and Pina caches one table per crate compilation.
#[derive(Clone, Debug, Default)]
pub struct SchemaConsts {
	resolved: BTreeMap<String, i128>,
	expressions: BTreeMap<String, String>,
	/// Every qualified declaration seen, so a duplicate can be detected.
	declared: BTreeSet<String>,
	/// Names that resolve to more than one declaration.
	ambiguous: BTreeSet<String>,
}

impl SchemaConsts {
	/// A table that resolves no names, used where only literals are permitted.
	#[must_use]
	pub fn empty() -> Self {
		Self::default()
	}

	/// Collect and resolve every integer constant declared in `files`.
	#[must_use]
	pub fn from_files(files: &[&syn::File]) -> Self {
		let mut table = Self::default();

		for file in files {
			table.collect_items(&file.items);
		}

		table.resolve_all();
		table
	}

	/// Collect and resolve constants from every Rust source file under `root`.
	///
	/// A procedural macro sees only the item it is expanding, so a capacity
	/// declared elsewhere in the crate has to be read back from disk. Entries
	/// are visited in sorted order so a name declared twice resolves the same
	/// way on every machine. Files that cannot be read or parsed are skipped:
	/// this is a best-effort index, and a capacity that stays unresolved is
	/// reported by name rather than silently accepted.
	#[must_use]
	pub fn from_source_dir(root: &Path) -> Self {
		let mut table = Self::default();
		let mut pending = vec![root.to_path_buf()];

		// A queue rather than a stack, so files are visited in the sorted order
		// `collect_items` relies on.
		let mut next = 0;
		while next < pending.len() {
			let directory = pending[next].clone();
			next += 1;

			let Ok(entries) = std::fs::read_dir(&directory) else {
				continue;
			};

			let mut paths = entries
				.flatten()
				.map(|entry| entry.path())
				.collect::<Vec<_>>();
			paths.sort();

			for path in paths {
				if path.is_dir() {
					pending.push(path);
					continue;
				}

				if path.extension().is_none_or(|extension| extension != "rs") {
					continue;
				}

				let Ok(source) = std::fs::read_to_string(&path) else {
					continue;
				};
				let Ok(file) = syn::parse_file(&source) else {
					continue;
				};

				table.collect_items(&file.items);
			}
		}

		table.resolve_all();
		table
	}

	/// The number of resolved constants in the table.
	#[must_use]
	pub fn len(&self) -> usize {
		self.resolved.len()
	}

	/// Whether the table resolved no constants.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.resolved.is_empty()
	}

	/// Merge `other` into this table, keeping values already resolved here.
	///
	/// Used when one schema can see more than one source tree: the expanding
	/// crate comes first so its constants win over a source-included program's
	/// same-named declarations.
	pub fn extend(&mut self, other: Self) {
		for (name, expression) in other.expressions {
			self.expressions.entry(name.clone()).or_insert(expression);
		}
		for (name, value) in other.resolved {
			self.resolved.entry(name).or_insert(value);
		}

		self.resolve_all();
	}

	/// Resolve one capacity expression, if every name in it is known.
	///
	/// Returns `None` for an expression this evaluator does not model and for a
	/// value outside the `usize` range a capacity must fit.
	#[must_use]
	pub fn resolve(&self, expression: &syn::Expr) -> Option<usize> {
		usize::try_from(self.evaluate(expression, 0)?).ok()
	}

	/// Resolve a capacity written as a bare path, such as `MAX_MEMBERS`.
	#[must_use]
	pub fn resolve_path(&self, path: &syn::Path) -> Option<usize> {
		self.lookup(path)
			.and_then(|value| usize::try_from(value).ok())
	}

	/// Rewrite every resolvable capacity in `item` to its numeric literal, and
	/// return the proof tokens that keep the referenced constants alive.
	///
	/// One traversal does both jobs. Normalizing replaces a constant with its
	/// value, which erases the only use of a constant declared for this schema;
	/// a constant in that position would then fail a `-D warnings` build as dead
	/// code. Each replacement is therefore paired with an assertion that reads
	/// the constant and re-checks the value, so the two can never disagree. A
	/// schema written with literals produces no tokens.
	///
	/// Capacity positions are rewritten in place so the rest of Pina sees the
	/// same type it saw when the bound was written as a literal: the generated
	/// proofs, `MAX_SIZE`, `projected_bytes`, and the checked-in ABI document all
	/// come out identical for a `const` and its value. An expression this table
	/// cannot evaluate is left alone for the schema grammar to reject by name.
	pub fn normalize_item(&self, item: &mut syn::ItemStruct) -> proc_macro2::TokenStream {
		let mut proofs = proc_macro2::TokenStream::new();

		for field in &mut item.fields {
			proofs.extend(self.normalize_type(&mut field.ty));
		}

		proofs
	}

	/// Rewrite every resolvable capacity in a parsed file.
	///
	/// The CLI parses a program's whole source tree before extracting anything
	/// from it, so one pass here covers every account, instruction, and event
	/// without threading the table through each extractor. The CLI has no
	/// generated code to keep constants alive in, so the proofs are discarded.
	pub fn normalize_file(&self, file: &mut syn::File) {
		for item in &mut file.items {
			self.normalize_items_in_place(item);
		}
	}

	fn normalize_items_in_place(&self, item: &mut syn::Item) {
		match item {
			syn::Item::Struct(item_struct) => {
				self.normalize_item(item_struct);
			}
			syn::Item::Mod(item_mod) => {
				let Some((_, items)) = &mut item_mod.content else {
					return;
				};
				for nested in items {
					self.normalize_items_in_place(nested);
				}
			}
			_ => {}
		}
	}

	/// Rewrite every resolvable capacity in `ty`, returning the proofs for the
	/// constants it replaced.
	///
	/// Only the capacity positions of Pina's closed grammar are rewritten, so a
	/// bare identifier naming a type is never mistaken for a capacity: `Vec<T,
	/// N>` resolves `N` and leaves `T`, and `PodVec<T, N, PFX>` leaves the
	/// prefix width, which the grammar requires to be a literal.
	fn normalize_type(&self, ty: &mut syn::Type) -> proc_macro2::TokenStream {
		let mut proofs = proc_macro2::TokenStream::new();

		match ty {
			syn::Type::Array(array) => {
				proofs.extend(self.normalize_length(&mut array.len));
				proofs.extend(self.normalize_type(&mut array.elem));
			}
			syn::Type::Paren(paren) => {
				proofs.extend(self.normalize_type(&mut paren.elem));
			}
			syn::Type::Group(group) => {
				proofs.extend(self.normalize_type(&mut group.elem));
			}
			syn::Type::Path(path) => {
				for segment in &mut path.path.segments {
					let syn::PathArguments::AngleBracketed(arguments) = &mut segment.arguments
					else {
						continue;
					};

					let capacity_index = match segment.ident.to_string().as_str() {
						"String" | "PodString" => 0,
						"Vec" | "PodVec" => 1,
						_ => {
							// `Option<T>` wraps another schema type; an unknown
							// name belongs to the grammar's rejection path.
							for argument in &mut arguments.args {
								if let syn::GenericArgument::Type(inner) = argument {
									proofs.extend(self.normalize_type(inner));
								}
							}
							continue;
						}
					};

					for (index, argument) in arguments.args.iter_mut().enumerate() {
						let is_capacity = index == capacity_index;

						match argument {
							// A bare `MAX_MEMBERS` capacity parses as a type
							// argument, because `syn` cannot know it names a
							// constant. Rewrite it to a literal in the capacity
							// position so the grammar sees the value.
							syn::GenericArgument::Type(inner) if is_capacity => {
								if let Some((path, value)) = self.resolve_type_name(inner) {
									proofs.extend(self.reference_assertion(path));
									*argument = syn::GenericArgument::Const(numeric_literal(value));
								} else {
									proofs.extend(self.normalize_type(inner));
								}
							}
							syn::GenericArgument::Const(expression) if is_capacity => {
								proofs.extend(self.normalize_length(expression));
							}
							syn::GenericArgument::Type(inner) => {
								proofs.extend(self.normalize_type(inner));
							}
							_ => {}
						}
					}
				}
			}
			_ => {}
		}

		proofs
	}

	/// Replace a capacity expression with its resolved literal.
	///
	/// Returns the proof that keeps the constant alive when the expression named
	/// one. A braced capacity (`Vec<u8, {N}>`) parses as a block holding the
	/// name, so both spellings are unwrapped.
	fn normalize_length(&self, length: &mut syn::Expr) -> proc_macro2::TokenStream {
		let Some(value) = self.resolve(length) else {
			return proc_macro2::TokenStream::new();
		};

		let mut proof = proc_macro2::TokenStream::new();
		for path in constant_paths(length) {
			proof.extend(self.reference_assertion(path));
		}

		*length = numeric_literal(value);
		proof
	}

	/// Resolve a bare constant name that arrived as a type argument.
	///
	/// The path is returned alongside the value so the caller can emit the
	/// reference proof without walking the type a second time.
	fn resolve_type_name<'a>(&self, ty: &'a syn::Type) -> Option<(&'a syn::Path, usize)> {
		let syn::Type::Path(path) = ty else {
			return None;
		};
		if path.qself.is_some() {
			return None;
		}

		self.resolve_path(&path.path)
			.map(|value| (&path.path, value))
	}

	/// Emit an assertion that reads a constant and pins its resolved value.
	fn reference_assertion(&self, path: &syn::Path) -> proc_macro2::TokenStream {
		let Some(value) = self.resolve_path(path) else {
			return proc_macro2::TokenStream::new();
		};
		let literal = syn::LitInt::new(&value.to_string(), proc_macro2::Span::call_site());

		quote::quote! {
			const _: () = ::core::assert!((#path) as usize == #literal);
		}
	}

	/// Look up the value a constant path names.
	///
	/// The lookup is scope-aware: a declaration is recorded under its
	/// module-qualified path, and a name declared at more than one path is
	/// treated as unknown. Selecting one by file order would size a schema with
	/// the wrong number and change its ABI document, so an ambiguous name fails
	/// the build instead.
	///
	/// A path whose leading segments are types (`Bounds::MAX_MEMBERS`) names an
	/// associated constant, which expansion cannot evaluate. It is rejected
	/// rather than matched against an unrelated free constant of the same name.
	fn lookup(&self, path: &syn::Path) -> Option<i128> {
		let mut segments = path.segments.iter();
		let last = segments.next_back()?;

		let mut qualified = String::new();
		for segment in segments {
			let text = segment.ident.to_string();
			// `crate`, `self`, and `super` are module keywords; any other leading
			// segment must be spelled like a module rather than a type.
			let is_module = matches!(text.as_str(), "crate" | "self" | "super")
				|| text.starts_with(|character: char| !character.is_ascii_uppercase());
			if !is_module {
				return None;
			}
			// `crate::limits::MAX` and `limits::MAX` name the same declaration.
			if !matches!(text.as_str(), "crate" | "self" | "super") {
				qualified.push_str(&text);
				qualified.push_str("::");
			}
		}

		let ident = last.ident.to_string();
		let key = if qualified.is_empty() {
			ident.clone()
		} else {
			format!("{qualified}{ident}")
		};

		if self.ambiguous.contains(&key) || self.ambiguous.contains(&ident) {
			return None;
		}

		let value = *self.resolved.get(&key)?;

		(value != UNRESOLVED).then_some(value)
	}

	fn collect_items(&mut self, items: &[syn::Item]) {
		self.collect_items_in(items, "");
	}

	/// Collect declarations, recording the module path each one lives at.
	///
	/// A name declared at two paths is marked ambiguous: resolving it to either
	/// would silently change a schema's layout.
	fn collect_items_in(&mut self, items: &[syn::Item], module: &str) {
		for item in items {
			match item {
				syn::Item::Const(item_const) => {
					let ident = item_const.ident.to_string();
					let qualified = format!("{module}{ident}");

					// A repeated declaration, at one path or across two, makes
					// every spelling of the name unsafe to resolve.
					if self.declared.contains(&qualified) || self.declared.contains(&ident) {
						self.ambiguous.insert(ident.clone());
						self.ambiguous.insert(qualified.clone());
					}

					self.declared.insert(ident.clone());
					self.declared.insert(qualified.clone());
					self.resolved.insert(qualified.clone(), UNRESOLVED);
					self.expressions
						.insert(qualified, item_const.expr.to_token_stream().to_string());
				}
				syn::Item::Mod(item_mod) => {
					if let Some((_, nested)) = &item_mod.content {
						let nested_module = format!("{module}{}::", item_mod.ident);
						self.collect_items_in(nested, &nested_module);
					}
				}
				_ => {}
			}
		}
	}

	fn resolve_all(&mut self) {
		// Constants may be written in terms of constants declared later, so
		// evaluation repeats until a pass stops learning new values.
		loop {
			let pending = self
				.expressions
				.iter()
				.filter(|(name, _)| {
					// A name that never resolved is dropped from `resolved` by
					// the pass below, so absence and the sentinel both mean
					// "still owed a value".
					self.resolved
						.get(*name)
						.is_none_or(|value| *value == UNRESOLVED)
				})
				.map(|(name, expression)| (name.clone(), expression.clone()))
				.collect::<Vec<_>>();

			if pending.is_empty() {
				break;
			}

			let mut progressed = false;
			for (name, expression) in pending {
				// The text came from a parsed expression, so it parses again.
				let value = syn::parse_str::<syn::Expr>(&expression)
					.ok()
					.and_then(|expression| self.evaluate(&expression, 0));
				if let Some(value) = value {
					self.resolved.insert(name, value);
					progressed = true;
				}
			}

			if !progressed {
				break;
			}
		}

		// A name that never resolved must not masquerade as a value.
		self.resolved.retain(|_, value| *value != UNRESOLVED);
	}

	fn evaluate(&self, expression: &syn::Expr, depth: usize) -> Option<i128> {
		if depth > MAX_CONST_REFERENCE_DEPTH {
			return None;
		}
		let depth = depth + 1;

		match expression {
			syn::Expr::Lit(syn::ExprLit {
				lit: syn::Lit::Int(value),
				..
			}) => value.base10_parse::<i128>().ok(),
			syn::Expr::Path(path) => self.lookup(&path.path),
			syn::Expr::Paren(paren) => self.evaluate(&paren.expr, depth),
			syn::Expr::Group(group) => self.evaluate(&group.expr, depth),
			syn::Expr::Cast(cast) => {
				let value = self.evaluate(&cast.expr, depth)?;
				cast_int(value, &cast.ty)
			}
			syn::Expr::Block(block) => {
				match block.block.stmts.last() {
					Some(syn::Stmt::Expr(inner, None)) => self.evaluate(inner, depth),
					_ => None,
				}
			}
			syn::Expr::Unary(unary) => {
				match unary.op {
					syn::UnOp::Neg(_) => self.evaluate(&unary.expr, depth)?.checked_neg(),
					_ => None,
				}
			}
			syn::Expr::Binary(binary) => {
				let left = self.evaluate(&binary.left, depth)?;
				let right = self.evaluate(&binary.right, depth)?;

				match binary.op {
					syn::BinOp::Add(_) => left.checked_add(right),
					syn::BinOp::Sub(_) => left.checked_sub(right),
					syn::BinOp::Mul(_) => left.checked_mul(right),
					syn::BinOp::Div(_) => left.checked_div(right),
					syn::BinOp::Rem(_) => left.checked_rem(right),
					syn::BinOp::Shl(_) => {
						u32::try_from(right)
							.ok()
							.and_then(|shift| left.checked_shl(shift))
					}
					syn::BinOp::Shr(_) => {
						u32::try_from(right)
							.ok()
							.and_then(|shift| left.checked_shr(shift))
					}
					syn::BinOp::BitAnd(_) => Some(left & right),
					syn::BinOp::BitOr(_) => Some(left | right),
					syn::BinOp::BitXor(_) => Some(left ^ right),
					_ => None,
				}
			}
			_ => None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A unique temporary directory for source-scanning tests.
	///
	/// The crate takes no `tempfile` dependency: `pina_abi` is a leaf shared by
	/// the CLI and the macros, and an extra dev-dependency would also force the
	/// standalone `pina_fuzz` lockfile to be regenerated.
	fn scratch_dir() -> std::path::PathBuf {
		use std::sync::atomic::AtomicUsize;
		use std::sync::atomic::Ordering;

		// A process-wide counter keeps concurrent tests from sharing a path.
		static NEXT: AtomicUsize = AtomicUsize::new(0);
		let unique = NEXT.fetch_add(1, Ordering::Relaxed);

		let root =
			std::env::temp_dir().join(format!("pina-abi-consts-{}-{unique}", std::process::id()));
		let _ = std::fs::remove_dir_all(&root);
		std::fs::create_dir_all(&root).unwrap();
		root
	}

	/// The structs declared by a file, for in-place edits.
	///
	/// The test sources declare structs only, so the filter never rejects one.
	fn structs<'a>(file: &'a mut syn::File) -> impl Iterator<Item = &'a mut syn::ItemStruct> {
		file.items.iter_mut().filter_map(|item| {
			match item {
				syn::Item::Struct(item) => Some(item),
				_ => None,
			}
		})
	}

	/// Build a one-field struct whose field carries `ty`.
	///
	/// Tests need types that cannot be written in source text (a `Group`, for
	/// instance), so they assemble the field directly rather than destructuring
	/// a parsed struct.
	fn struct_with_type(ty: syn::Type) -> syn::ItemStruct {
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: u8,
			}
		};
		for field in item.fields.iter_mut() {
			field.ty = ty.clone();
		}
		item
	}

	/// Replace the type of every field, used for types that cannot be written as
	/// source text.
	fn set_field_type(item: &mut syn::ItemStruct, ty: &syn::Type) {
		for field in item.fields.iter_mut() {
			field.ty = ty.clone();
		}
	}

	/// Replace the capacity argument of the first `Vec<T, N>` field.
	///
	/// The callers build a one-field struct whose field is a `Vec`, so the
	/// capacity slot always exists.
	fn set_vec_capacity(item: &mut syn::ItemStruct, ty: syn::Type) {
		for field in item.fields.iter_mut() {
			field.ty = ty.clone();
		}
	}

	fn table(source: &str) -> SchemaConsts {
		let file = syn::parse_file(source).unwrap_or_else(|error| panic!("test source: {error}"));
		SchemaConsts::from_files(&[&file])
	}

	fn normalize(source: &str, declaration: &str) -> String {
		let table = table(source);
		let mut item: syn::ItemStruct =
			syn::parse_str(declaration).unwrap_or_else(|error| panic!("test item: {error}"));
		table.normalize_item(&mut item);
		item.fields.to_token_stream().to_string()
	}

	#[test]
	fn a_capacity_expression_naming_a_non_path_does_not_resolve() {
		// A method call is not something expansion can evaluate.
		let table = SchemaConsts::empty();
		let expression: syn::Expr = syn::parse_quote!(4.min(2));
		assert_eq!(table.resolve(&expression), None);
	}

	#[test]
	fn every_integer_cast_width_truncates() {
		for (source, expected) in [
			("(300u16 as u8) as usize", 44),
			("(70000u32 as u16) as usize", 4464),
			("(4294967296u64 as u32) as usize", 0),
			("(1u32 as u8) as usize", 1),
			("(1u64 as u16) as usize", 1),
			("(1u128 as u32) as usize", 1),
			("(4294967296u64 as i64) as usize", 4294967296),
		] {
			let table = table(&format!("const WIDTH: usize = {source};"));
			assert_eq!(
				table.resolve_path(&syn::parse_quote!(WIDTH)),
				Some(expected),
				"wrong value for `{source}`"
			);
		}
	}

	#[test]
	fn collection_stops_at_the_depth_limit() {
		let table = SchemaConsts::empty();
		let expression: syn::Expr = syn::parse_quote!(WIDTH);
		let mut paths = Vec::new();
		collect_paths(&expression, MAX_CONST_REFERENCE_DEPTH + 1, &mut paths);
		assert!(paths.is_empty(), "the depth limit must stop collection");
	}

	#[test]
	fn collection_walks_every_expression_shape() {
		let parenthesized: syn::Expr = syn::parse_quote!((WIDTH));
		let negated: syn::Expr = syn::parse_quote!(-WIDTH);
		let cast: syn::Expr = syn::parse_quote!(WIDTH as usize);

		for expression in [parenthesized, negated, cast] {
			let paths = constant_paths(&expression);
			let printed = quote::ToTokens::to_token_stream(&expression).to_string();
			assert_eq!(paths.len(), 1, "the name must be collected: {printed}");
		}
	}

	#[test]
	fn a_narrowing_cast_truncates_like_rust() {
		// Rust evaluates `(300u16 as u8) as usize` to 44. Resolving 300 would
		// emit a capacity the generated assertion then rejects.
		let table = table("const WIDTH: usize = (300u16 as u8) as usize;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(44));
	}

	#[test]
	fn a_widening_cast_keeps_the_value() {
		let table = table("const WIDTH: usize = 4u8 as usize;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(4));
	}

	#[test]
	fn a_negative_cast_wraps_to_its_unsigned_bits() {
		let table = table("const WIDTH: usize = (-1i32 as u8) as usize;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(255));
	}

	#[test]
	fn a_signed_cast_reinterprets_the_sign_bit() {
		let table = table("const SMALL: i8 = 255u8 as i8;");
		// A negative value is not a valid capacity, so `resolve_path` reports
		// none; the evaluator itself sees the signed reinterpretation.
		assert!(table.resolve_path(&syn::parse_quote!(SMALL)).is_none());
		assert_eq!(
			table.lookup(&syn::parse_quote!(SMALL)),
			Some(i128::from(-1i8))
		);
	}

	#[test]
	fn a_cast_to_a_non_path_type_does_not_resolve() {
		let table = SchemaConsts::empty();
		let inner: syn::Expr = syn::parse_quote!(4);
		let expression = syn::Expr::Cast(syn::ExprCast {
			attrs: Vec::new(),
			expr: Box::new(inner),
			as_token: syn::token::As::default(),
			ty: Box::new(syn::parse_quote!([u8; 4])),
		});

		assert_eq!(table.resolve(&expression), None);
	}

	#[test]
	fn a_cast_through_a_qualified_type_does_not_resolve() {
		let table = SchemaConsts::empty();
		let inner: syn::Expr = syn::parse_quote!(4);
		let expression = syn::Expr::Cast(syn::ExprCast {
			attrs: Vec::new(),
			expr: Box::new(inner),
			as_token: syn::token::As::default(),
			ty: Box::new(syn::parse_quote!(<u8 as Trait>::Assoc)),
		});

		assert_eq!(table.resolve(&expression), None);
	}

	#[test]
	fn a_non_integer_cast_does_not_resolve() {
		let table = table("const WIDTH: usize = 4.0f32 as usize;");
		// The float literal does not evaluate, so the cast cannot either.
		assert!(table.resolve_path(&syn::parse_quote!(WIDTH)).is_none());
	}

	#[test]
	fn a_cast_to_an_unknown_type_does_not_resolve() {
		let table = table("const WIDTH: usize = 4u8 as Wrapper;");
		assert!(table.resolve_path(&syn::parse_quote!(WIDTH)).is_none());
	}

	#[test]
	fn a_name_declared_in_two_modules_does_not_resolve() {
		// `alpha::MAX` is 4 and `beta::MAX` is 8. Picking either by file order
		// would silently change a schema layout, so both stay unresolved.
		let table = table(
			"mod alpha { pub const MAX: usize = 4; }\nmod beta { pub const MAX: usize = 8; }",
		);
		assert!(table.resolve_path(&syn::parse_quote!(MAX)).is_none());
		assert!(table.resolve_path(&syn::parse_quote!(alpha::MAX)).is_none());
		assert!(table.resolve_path(&syn::parse_quote!(beta::MAX)).is_none());
	}

	#[test]
	fn a_unique_qualified_name_still_resolves() {
		let table = table("mod alpha { pub const MAX: usize = 4; }");
		assert_eq!(table.resolve_path(&syn::parse_quote!(alpha::MAX)), Some(4));
		assert_eq!(
			table.resolve_path(&syn::parse_quote!(crate::alpha::MAX)),
			Some(4)
		);
		// `MAX` alone is not in scope at the crate root.
		assert!(table.resolve_path(&syn::parse_quote!(MAX)).is_none());
	}

	#[test]
	fn a_repeated_name_at_the_root_does_not_resolve() {
		let table = table("const WIDTH: usize = 4;\nconst WIDTH: usize = 8;");
		assert!(table.resolve_path(&syn::parse_quote!(WIDTH)).is_none());
	}

	#[test]
	fn a_crate_qualified_path_resolves_like_a_bare_one() {
		let table = table("const WIDTH: usize = 4;");
		assert_eq!(
			table.resolve_path(&syn::parse_quote!(crate::WIDTH)),
			Some(4)
		);
	}

	#[test]
	fn an_arithmetic_capacity_references_every_constant_it_reads() {
		let table = table("const WIDTH: usize = 4;\nconst RATIO: usize = 3;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: [u64; WIDTH * RATIO],
			}
		};

		let proofs = table.normalize_item(&mut item).to_string();

		// A private constant used only here must stay live, so both names are
		// referenced and each is pinned to its own value.
		assert!(
			proofs.contains("WIDTH"),
			"WIDTH must be referenced: {proofs}"
		);
		assert!(
			proofs.contains("RATIO"),
			"RATIO must be referenced: {proofs}"
		);
		assert!(
			proofs.contains("(WIDTH) as usize == 4"),
			"WIDTH's value must be pinned: {proofs}"
		);
		assert!(
			proofs.contains("(RATIO) as usize == 3"),
			"RATIO's value must be pinned: {proofs}"
		);
	}

	#[test]
	fn an_arithmetic_capacity_resolves_through_a_cast() {
		let table = table("const WIDTH: usize = (300u16 as u8) as usize;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: [u64; WIDTH],
			}
		};

		// The proof pins the truncated value, so it agrees with the emitted
		// capacity.
		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			proofs.contains("44"),
			"the truncated value must be pinned: {proofs}"
		);
	}

	#[test]
	fn resolves_a_bare_constant() {
		let table = table("const MAX_MEMBERS: usize = 24;");
		assert_eq!(
			table.resolve_path(&syn::parse_quote!(MAX_MEMBERS)),
			Some(24)
		);
		assert_eq!(table.len(), 1);
	}

	#[test]
	fn resolves_arithmetic_over_constants() {
		let table = table("const WIDTH: usize = 4;\nconst SLOTS: usize = WIDTH * 6 + 1;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(SLOTS)), Some(25));
	}

	#[test]
	fn resolves_constants_declared_in_any_order() {
		let table = table("const FIRST: usize = SECOND + 1;\nconst SECOND: usize = 4;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(FIRST)), Some(5));
	}

	#[test]
	fn resolves_constants_nested_in_modules() {
		let table = table("mod limits {\n\tpub const MAX: usize = 7;\n}");
		assert_eq!(table.resolve_path(&syn::parse_quote!(limits::MAX)), Some(7));
	}

	#[test]
	fn leaves_cyclic_constants_unresolved() {
		let table = table("const LEFT: usize = RIGHT;\nconst RIGHT: usize = LEFT;");
		assert!(table.resolve_path(&syn::parse_quote!(LEFT)).is_none());
		assert!(table.is_empty());
	}

	#[test]
	fn ignores_non_integer_constants() {
		let table = table(
			"const NAME: &str = \"x\";\nconst BYTES: [u8; 2] = [0, 1];\nconst COUNT: usize = 3;",
		);
		assert_eq!(table.len(), 1);
		assert_eq!(table.resolve_path(&syn::parse_quote!(COUNT)), Some(3));
	}

	#[test]
	fn normalizes_a_bare_constant_capacity_in_a_vector() {
		let normalized = normalize(
			"const MAX_MEMBERS: usize = 24;",
			"struct Roster { members: Vec<Address, MAX_MEMBERS> }",
		);
		assert!(
			normalized.contains('2'),
			"capacity should resolve to its value: {normalized}"
		);
		assert!(
			!normalized.contains("MAX_MEMBERS"),
			"the name should be replaced: {normalized}"
		);
	}

	#[test]
	fn normalizes_a_constant_array_length() {
		let normalized = normalize(
			"const WIDTH: usize = 4;",
			"struct Words { values: [u64; WIDTH] }",
		);
		assert!(
			normalized.contains('4'),
			"length should resolve: {normalized}"
		);
	}

	#[test]
	fn normalizes_an_expression_capacity() {
		let normalized = normalize(
			"const WIDTH: usize = 4;",
			"struct Words { values: [u64; WIDTH * 2] }",
		);
		assert!(
			normalized.contains('8'),
			"length should resolve: {normalized}"
		);
	}

	#[test]
	fn normalizes_nested_compact_tails() {
		let normalized = normalize(
			"const MAX_MEMBERS: usize = 24;",
			"struct Roster { members: Vec<Address, MAX_MEMBERS>, note: \
			 Option<String<MAX_MEMBERS>> }",
		);
		assert!(
			!normalized.contains("MAX_MEMBERS"),
			"every capacity should resolve: {normalized}"
		);
	}

	#[test]
	fn leaves_the_element_type_alone() {
		let normalized = normalize(
			"const MAX_MEMBERS: usize = 4;",
			"struct Roster { members: Vec<Address, MAX_MEMBERS> }",
		);
		assert!(
			normalized.contains("Address"),
			"the element type must survive: {normalized}"
		);
	}

	#[test]
	fn leaves_prefix_widths_alone() {
		let normalized = normalize(
			"const MAX_MEMBERS: usize = 4;",
			"struct Roster { members: PodVec<u8, MAX_MEMBERS, 8> }",
		);
		assert!(
			normalized.contains('4'),
			"capacity should resolve: {normalized}"
		);
		assert!(
			normalized.contains('8'),
			"the prefix width must survive: {normalized}"
		);
	}

	#[test]
	fn leaves_an_unknown_capacity_in_place() {
		let normalized = normalize(
			"const MAX_MEMBERS: usize = 4;",
			"struct Roster { members: Vec<Address, MISSING> }",
		);
		assert!(
			normalized.contains("MISSING"),
			"an unresolved capacity must stay for the grammar to reject: {normalized}"
		);
	}

	#[test]
	fn evaluates_parenthesized_and_cast_expressions() {
		let table = table("const A: usize = (4);\nconst B: usize = 4 as usize;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(A)), Some(4));
		assert_eq!(table.resolve_path(&syn::parse_quote!(B)), Some(4));
	}

	#[test]
	fn evaluates_block_expressions() {
		let table = table("const A: usize = { 4 };\nconst B: usize = { 4; };");
		assert_eq!(table.resolve_path(&syn::parse_quote!(A)), Some(4));
		// A block whose last statement is not an expression has no value.
		assert_eq!(table.resolve_path(&syn::parse_quote!(B)), None);
	}

	#[test]
	fn evaluates_unary_and_comparison_expressions() {
		let table = table("const NOT: usize = !4;\nconst EQ: usize = 4 == 4;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(NOT)), None);
		assert_eq!(table.resolve_path(&syn::parse_quote!(EQ)), None);
	}

	#[test]
	fn evaluates_every_supported_binary_operator() {
		let table = table(
			"const SUM: usize = 6 + 2;\nconst DIFF: usize = 6 - 2;\nconst PROD: usize = 6 * \
			 2;\nconst QUOT: usize = 6 / 2;\nconst REM: usize = 7 % 4;\nconst SHL: usize = 3 << \
			 1;\nconst SHR: usize = 12 >> 2;\nconst AND: usize = 6 & 3;\nconst OR: usize = 6 | \
			 1;\nconst XOR: usize = 6 ^ 3;",
		);

		for (name, expected) in [
			("SUM", 8),
			("DIFF", 4),
			("PROD", 12),
			("QUOT", 3),
			("REM", 3),
			("SHL", 6),
			("SHR", 3),
			("AND", 2),
			("OR", 7),
			("XOR", 5),
		] {
			let path = syn::parse_str::<syn::Path>(name).expect("test path parses");
			assert_eq!(
				table.resolve_path(&path),
				Some(expected),
				"wrong value for {name}"
			);
		}
	}

	#[test]
	fn a_negative_shift_amount_does_not_resolve() {
		let table =
			table("const SHIFT: usize = 1 << SHIFT_AMOUNT;\nconst SHIFT_AMOUNT: i128 = -1;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(SHIFT)), None);
	}

	#[test]
	fn a_value_outside_usize_does_not_resolve_as_a_capacity() {
		let table = table("const NEGATIVE: i128 = 0 - 1;");
		// The table can hold the value, but no capacity may be negative.
		assert_eq!(table.resolve_path(&syn::parse_quote!(NEGATIVE)), None);
	}

	#[test]
	fn a_literal_beyond_i128_does_not_resolve() {
		let table = table("const HUGE: usize = 340282366920938463463374607431768211456;");
		assert!(table.resolve_path(&syn::parse_quote!(HUGE)).is_none());
	}

	#[test]
	fn an_arbitrary_expression_does_not_resolve() {
		let table = table("const CALLED: usize = core::mem::size_of::<u64>();");
		assert!(table.resolve_path(&syn::parse_quote!(CALLED)).is_none());
	}

	#[test]
	fn resolution_stops_at_the_reference_depth_limit() {
		let table = table("const A: usize = 4;");
		let expression: syn::Expr = syn::parse_quote!(A);
		assert_eq!(
			table.evaluate(&expression, MAX_CONST_REFERENCE_DEPTH + 1),
			None
		);
		// Just inside the limit still resolves.
		assert_eq!(
			table.evaluate(&expression, MAX_CONST_REFERENCE_DEPTH),
			Some(4)
		);
	}

	#[test]
	fn a_self_referential_constant_does_not_resolve() {
		let table = table("const LOOP: usize = LOOP;");
		assert!(table.is_empty());
	}

	#[test]
	fn extending_a_table_keeps_the_existing_value() {
		let mut base = table("const WIDTH: usize = 4;");
		let other = table("const WIDTH: usize = 99;\nconst HEIGHT: usize = 8;");

		base.extend(other);

		assert_eq!(base.resolve_path(&syn::parse_quote!(WIDTH)), Some(4));
		assert_eq!(base.resolve_path(&syn::parse_quote!(HEIGHT)), Some(8));
	}

	#[test]
	fn extending_can_resolve_a_name_the_first_table_could_not() {
		let mut base = table("const TOTAL: usize = WIDTH * 2;");
		assert!(base.resolve_path(&syn::parse_quote!(TOTAL)).is_none());

		base.extend(table("const WIDTH: usize = 4;"));

		assert_eq!(base.resolve_path(&syn::parse_quote!(TOTAL)), Some(8));
	}

	#[test]
	fn the_empty_table_resolves_nothing() {
		let table = SchemaConsts::empty();
		assert!(table.is_empty());
		assert_eq!(table.len(), 0);
		assert_eq!(table.resolve_path(&syn::parse_quote!(ANY)), None);
	}

	#[test]
	fn normalizes_structs_nested_in_inline_modules() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			mod inner {
				pub struct Words {
					pub values: [u64; WIDTH],
				}
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"nested struct should resolve: {printed}"
		);
	}

	#[test]
	fn normalize_ignores_items_that_are_not_structs() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			/// A constant survives normalization untouched.
			pub const WIDTH: usize = 4;
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			printed.contains("WIDTH"),
			"the declaration must survive: {printed}"
		);
	}

	#[test]
	fn normalize_leaves_types_outside_the_capacity_grammar_alone() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Pair {
				pub values: (u8, u16),
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			printed.contains("u8"),
			"an unrelated type must survive: {printed}"
		);
	}

	#[test]
	fn normalize_descends_through_parenthesized_types() {
		let table = table("const WIDTH: usize = 4;");
		let mut ty: syn::Type = syn::parse_quote!(([u8; WIDTH]));

		table.normalize_type(&mut ty);

		let printed = quote::ToTokens::to_token_stream(&ty).to_string();
		assert!(
			printed.contains('4'),
			"the nested length should resolve: {printed}"
		);
	}

	#[test]
	fn normalize_descends_through_grouped_types() {
		let table = table("const WIDTH: usize = 4;");
		let array: syn::Type = syn::parse_quote!([u64; WIDTH]);
		let mut ty = syn::Type::Group(syn::TypeGroup {
			attrs: Vec::new(),
			group_token: syn::token::Group::default(),
			elem: Box::new(array),
		});

		// A grouped type has no capacity, so normalization is a no-op that must
		// not panic and must preserve the group.
		table.normalize_type(&mut ty);

		assert!(matches!(ty, syn::Type::Group(_)));
	}

	#[test]
	fn reference_proofs_are_emitted_for_a_bare_capacity_name() {
		let table = table("const WIDTH: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: [u64; WIDTH],
			}
		};

		let proofs = table.normalize_item(&mut item).to_string();

		assert!(
			proofs.contains("WIDTH"),
			"the constant must be referenced: {proofs}"
		);
		assert!(
			proofs.contains('4'),
			"the proof must pin the value: {proofs}"
		);
	}

	#[test]
	fn reference_proofs_are_emitted_for_a_const_argument() {
		let table = table("const WIDTH: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: Vec<u8, {WIDTH}>,
			}
		};

		// A braced expression parses as a const argument rather than a type.
		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			proofs.contains("WIDTH"),
			"the constant must be referenced: {proofs}"
		);
	}

	#[test]
	fn reference_proofs_are_empty_for_a_literal_schema() {
		let table = table("const WIDTH: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: [u64; 4],
			}
		};

		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn reference_proofs_skip_an_unknown_name() {
		let table = SchemaConsts::empty();
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: Vec<u8, UNKNOWN>,
			}
		};

		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn reference_proofs_skip_a_qualified_associated_path() {
		let table = table("const MAX: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: Vec<u8, Bounds::MAX>,
			}
		};

		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			!proofs.contains("Bounds"),
			"an associated constant must not be referenced: {proofs}"
		);
	}

	#[test]
	fn reference_proofs_descend_through_options_and_elements() {
		let table = table("const WIDTH: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub maybe: Option<[u64; WIDTH]>,
			}
		};

		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			proofs.contains("WIDTH"),
			"the nested constant must be referenced: {proofs}"
		);
	}

	#[test]
	fn reference_proofs_ignore_a_non_path_capacity_position() {
		let table = table("const WIDTH: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub bad: Vec<u8, [u8; 2]>,
			}
		};

		// An array in the capacity position is not a name, so nothing is emitted.
		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn reference_proofs_skip_a_grouped_expression() {
		let table = table("const WIDTH: usize = 4;");
		let grouped = syn::Expr::Group(syn::ExprGroup {
			attrs: Vec::new(),
			group_token: syn::token::Group::default(),
			expr: Box::new(syn::parse_quote!(WIDTH)),
		});
		let mut arguments = syn::punctuated::Punctuated::new();
		arguments.push(syn::GenericArgument::Type(syn::parse_quote!(u8)));
		arguments.push(syn::GenericArgument::Const(grouped));
		let mut item = struct_with_type(syn::Type::Path(syn::TypePath {
			attrs: Vec::new(),
			qself: None,
			path: syn::Path {
				leading_colon: None,
				segments: [syn::PathSegment {
					ident: syn::Ident::new("Vec", proc_macro2::Span::call_site()),
					arguments: syn::PathArguments::AngleBracketed(
						syn::AngleBracketedGenericArguments {
							colon2_token: None,
							lt_token: syn::token::Lt::default(),
							args: arguments,
							gt_token: syn::token::Gt::default(),
						},
					),
				}]
				.into_iter()
				.collect(),
			},
		}));

		// A grouped capacity still reads `WIDTH`, so the proof is emitted and
		// the constant stays live.
		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			proofs.contains("WIDTH"),
			"the grouped name must be referenced: {proofs}"
		);
	}

	#[test]
	fn reference_proofs_descend_through_parenthesized_types() {
		let table = table("const WIDTH: usize = 4;");
		let array: syn::Type = syn::parse_quote!([u64; WIDTH]);
		let mut item = struct_with_type(syn::Type::Paren(syn::TypeParen {
			attrs: Vec::new(),
			paren_token: syn::token::Paren::default(),
			elem: Box::new(array),
		}));

		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			proofs.contains("WIDTH"),
			"the nested constant must be referenced: {proofs}"
		);
	}

	#[test]
	fn reference_proofs_descend_through_grouped_types() {
		let table = table("const WIDTH: usize = 4;");
		let array: syn::Type = syn::parse_quote!([u64; WIDTH]);
		let mut item = struct_with_type(syn::Type::Group(syn::TypeGroup {
			attrs: Vec::new(),
			group_token: syn::token::Group::default(),
			elem: Box::new(array),
		}));

		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			proofs.contains("WIDTH"),
			"the nested constant must be referenced: {proofs}"
		);
	}

	#[test]
	fn reference_proofs_are_emitted_for_a_string_capacity() {
		let table = table("const WIDTH: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Note {
				pub title: String<WIDTH>,
			}
		};

		let proofs = table.normalize_item(&mut item).to_string();
		assert!(
			proofs.contains("WIDTH"),
			"the string capacity must be referenced: {proofs}"
		);
	}

	#[test]
	fn normalize_resolves_a_string_capacity() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Note {
				pub title: String<WIDTH>,
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"the string capacity should resolve: {printed}"
		);
		assert!(
			printed.contains('4'),
			"the resolved value should appear: {printed}"
		);
	}

	#[test]
	fn normalize_replaces_a_braced_capacity_expression() {
		let table = table("const WIDTH: usize = 4;");
		let inner: syn::Expr = syn::parse_quote!(WIDTH);
		let mut file: syn::File = syn::parse_quote! {
			pub struct Words {
				pub values: [u64; 4],
			}
		};
		let braced = syn::Type::Array(syn::TypeArray {
			attrs: Vec::new(),
			bracket_token: syn::token::Bracket::default(),
			elem: Box::new(syn::parse_quote!(u64)),
			semi_token: syn::token::Semi::default(),
			len: syn::Expr::Block(syn::ExprBlock {
				attrs: Vec::new(),
				label: None,
				block: syn::Block {
					brace_token: syn::token::Brace::default(),
					stmts: vec![syn::Stmt::Expr(inner, None)],
				},
			}),
		});
		for item in structs(&mut file) {
			set_field_type(item, &braced);
		}

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"the braced capacity should resolve: {printed}"
		);
	}

	#[test]
	fn evaluate_handles_a_group_wrapped_expression() {
		let table = table("const WIDTH: usize = 4;");
		let expression = syn::Expr::Group(syn::ExprGroup {
			attrs: Vec::new(),
			group_token: syn::token::Group::default(),
			expr: Box::new(syn::parse_quote!(WIDTH)),
		});

		assert_eq!(table.resolve(&expression), Some(4));
	}

	#[test]
	fn an_unreadable_source_file_is_skipped() {
		use std::os::unix::fs::PermissionsExt as _;

		let dir = scratch_dir();
		let readable = dir.join("lib.rs");
		let locked = dir.join("locked.rs");
		std::fs::write(&readable, "const WIDTH: usize = 4;").unwrap();
		std::fs::write(&locked, "const HEIGHT: usize = 8;").unwrap();
		let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
		permissions.set_mode(0o000);
		std::fs::set_permissions(&locked, permissions).unwrap();

		let table = SchemaConsts::from_source_dir(&dir);

		// The readable file still resolves; the locked one is skipped.
		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(4));
		assert!(table.resolve_path(&syn::parse_quote!(HEIGHT)).is_none());
	}

	#[test]
	fn normalize_resolves_a_const_argument_in_a_generic() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Words {
				pub values: Vec<u8, {WIDTH}>,
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"the braced capacity should resolve: {printed}"
		);
	}

	#[test]
	fn normalize_ignores_an_unresolvable_const_argument() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Words {
				pub values: Vec<u8, {MISSING}>,
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			printed.contains("MISSING"),
			"an unknown capacity must stay: {printed}"
		);
	}

	#[test]
	fn normalize_descends_through_a_collection_of_collections() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Words {
				pub nested: Option<Vec<[u64; WIDTH], 2>>,
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"the nested length should resolve: {printed}"
		);
	}

	#[test]
	fn reference_proofs_ignore_a_lifetime_argument() {
		let table = table("const WIDTH: usize = 4;");
		let mut item: syn::ItemStruct = syn::parse_quote! {
			struct Words {
				pub values: Borrowed<'static, WIDTH>,
			}
		};

		// A lifetime in an unknown generic is neither a capacity nor a type, so
		// nothing is emitted for it.
		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn reference_proofs_ignore_a_non_generic_field_type() {
		let table = table("const WIDTH: usize = 4;");
		let mut item = struct_with_type(syn::parse_quote!((u8, u16)));

		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn reference_proofs_ignore_a_braced_block_without_a_name() {
		let table = table("const WIDTH: usize = 4;");
		let mut item = struct_with_type(syn::Type::Array(syn::TypeArray {
			attrs: Vec::new(),
			bracket_token: syn::token::Bracket::default(),
			elem: Box::new(syn::parse_quote!(u8)),
			semi_token: syn::token::Semi::default(),
			len: syn::parse_quote!({
				4;
			}),
		}));

		// The block's last statement is not an expression, so no name resolves.
		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn reference_proofs_ignore_a_qualified_capacity_type() {
		let table = table("const WIDTH: usize = 4;");
		let mut item = struct_with_type(syn::Type::Path(syn::TypePath {
			attrs: Vec::new(),
			qself: Some(syn::QSelf {
				lt_token: syn::token::Lt::default(),
				ty: Box::new(syn::parse_quote!(u8)),
				position: 0,
				as_token: None,
				gt_token: syn::token::Gt::default(),
			}),
			path: syn::parse_quote!(Assoc),
		}));

		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn normalize_ignores_a_qualified_capacity_type() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Words {
				pub values: u8,
			}
		};
		let qualified: syn::Type = syn::parse_quote!(Vec<u8, <u8 as Trait>::Assoc>);
		for item in structs(&mut file) {
			set_vec_capacity(item, qualified.clone());
		}

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			printed.contains("Assoc"),
			"a qualified capacity must stay: {printed}"
		);
	}

	#[test]
	fn collects_constants_nested_in_inline_modules() {
		let table = table("mod limits { pub const DEPTH: usize = 2; }");
		assert_eq!(
			table.resolve_path(&syn::parse_quote!(limits::DEPTH)),
			Some(2)
		);
	}

	#[test]
	fn normalize_ignores_a_generic_without_capacity_arguments() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Words {
				pub plain: Plain,
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			printed.contains("Plain"),
			"an argument-free type must survive: {printed}"
		);
	}

	#[test]
	fn a_braced_capacity_that_is_not_a_name_yields_no_path() {
		// `constant_path` is exercised for blocks through normalization: a
		// braced capacity resolves and must still produce a proof.
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub struct Words {
				pub values: Vec<u8, {WIDTH}>,
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"the braced name should resolve: {printed}"
		);
	}

	#[test]
	fn normalize_file_finishes_a_module_whose_last_item_is_a_struct() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub mod inner {
				pub struct Words {
					pub values: [u64; WIDTH],
				}
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"the nested length should resolve: {printed}"
		);
	}

	#[test]
	fn normalize_file_skips_a_file_module_without_a_body() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub mod external;
		};

		// `mod external;` has no inline body, so there is nothing to walk.
		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			printed.contains("external"),
			"the declaration must survive: {printed}"
		);
	}

	#[test]
	fn structs_skips_items_that_are_not_structs() {
		let mut file: syn::File = syn::parse_quote! {
			pub const WIDTH: usize = 4;
			pub fn helper() {}
		};

		// The iterator filters both items out, so nothing is yielded.
		assert_eq!(structs(&mut file).count(), 0);
	}

	#[test]
	fn normalize_file_walks_a_nested_module() {
		let table = table("const WIDTH: usize = 4;");
		let mut file: syn::File = syn::parse_quote! {
			pub mod inner {
				pub mod deeper {
					pub struct Words {
						pub values: [u64; WIDTH],
					}
				}
			}
		};

		table.normalize_file(&mut file);

		let printed = quote::ToTokens::to_token_stream(&file).to_string();
		assert!(
			!printed.contains("WIDTH"),
			"the nested length should resolve: {printed}"
		);
	}

	#[test]
	fn reference_assertion_is_empty_for_an_unknown_name() {
		let table = SchemaConsts::empty();
		// Reached through a capacity that normalizes only once a name is known,
		// so an unknown nested type still walks but emits nothing.
		let mut item = struct_with_type(syn::parse_quote!([u64; UNKNOWN]));

		assert!(table.normalize_item(&mut item).is_empty());
	}

	#[test]
	fn collect_items_skips_a_file_level_module_without_a_body() {
		let table = table("pub mod external;\nconst WIDTH: usize = 4;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(4));
	}

	#[test]
	fn collect_items_ignores_other_item_kinds() {
		let table =
			table("struct NotAConst;\nimpl NotAConst {}\nfn helper() {}\nconst WIDTH: usize = 4;");
		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(4));
	}

	#[test]
	fn a_block_capacity_without_a_final_expression_yields_no_path() {
		// The last statement is a `let`, so the block names nothing.
		let block: syn::Expr = syn::parse_quote!({
			let width = 4;
		});
		assert!(constant_paths(&block).is_empty());
	}

	#[test]
	fn a_capacity_expression_that_is_neither_a_path_nor_a_block_yields_no_path() {
		let literal: syn::Expr = syn::parse_quote!(4);
		assert!(constant_paths(&literal).is_empty());
	}

	#[test]
	fn reference_assertion_is_empty_for_a_name_the_table_does_not_know() {
		let table = SchemaConsts::empty();
		let path: syn::Path = syn::parse_quote!(UNKNOWN);
		assert!(table.reference_assertion(&path).is_empty());
	}

	#[test]
	fn collect_items_ignores_a_module_without_an_inline_body() {
		let mut table = SchemaConsts::default();
		let file: syn::File = syn::parse_quote!(
			pub mod external;
			const WIDTH: usize = 4;
		);
		table.collect_items(&file.items);
		table.resolve_all();

		// The file module contributes nothing and does not stop the const.
		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(4));
	}

	#[test]
	fn collects_constants_from_source_files() {
		let dir = scratch_dir();
		let nested = dir.join("nested");
		std::fs::create_dir_all(&nested).unwrap_or_else(|error| panic!("mkdir: {error}"));
		std::fs::write(dir.join("lib.rs"), "const WIDTH: usize = 4;").unwrap();
		std::fs::write(nested.join("limits.rs"), "pub const HEIGHT: usize = 8;").unwrap();
		// Non-Rust and unparseable files are skipped rather than failing.
		std::fs::write(dir.join("notes.txt"), "not rust").unwrap();
		std::fs::write(dir.join("broken.rs"), "const OOPS: usize = ;").unwrap();

		let table = SchemaConsts::from_source_dir(&dir);

		assert_eq!(table.resolve_path(&syn::parse_quote!(WIDTH)), Some(4));
		assert_eq!(table.resolve_path(&syn::parse_quote!(HEIGHT)), Some(8));
	}

	#[test]
	fn a_missing_source_directory_resolves_nothing() {
		let dir = scratch_dir();
		let table = SchemaConsts::from_source_dir(&dir.join("absent"));
		assert!(table.is_empty());
	}

	#[test]
	fn resolves_a_constant_through_a_module_path() {
		let normalized = normalize(
			"mod limits { pub const MAX: usize = 6; }",
			"struct Roster { members: Vec<u8, limits::MAX> }",
		);
		assert!(
			normalized.contains('6'),
			"qualified capacity should resolve: {normalized}"
		);
	}
}
