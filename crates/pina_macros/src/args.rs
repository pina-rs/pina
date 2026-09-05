// Darling's derive expansions emit `continue` in generated code.
#![allow(clippy::needless_continue)]

use darling::FromDeriveInput;
use darling::FromField;
use darling::FromMeta;
use darling::ast::Data;
use darling::util::Flag;
use darling::util::Ignored;
use quote::ToTokens;
use syn::Expr;
use syn::Generics;
use syn::Ident;
use syn::Path;
use syn::Type;
use syn::ext::IdentExt;

/// Arguments for the `#[account(...)]` attribute macro.
#[derive(Debug, FromMeta)]
pub(crate) struct AccountArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: Path,
	/// Set the discriminator enum for this account.
	pub(crate) discriminator: Path,
	/// Set the variant of the discriminator enum.
	pub(crate) variant: Option<Ident>,
}

/// Arguments for the `#[instruction(...)]` attribute macro.
#[derive(Debug, FromMeta)]
pub(crate) struct InstructionArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: Path,
	/// Set the discriminator enum for this instruction.
	pub(crate) discriminator: Path,
	/// Set the variant of the discriminator enum.
	pub(crate) variant: Option<Ident>,
}

/// Arguments for the `#[event(...)]` attribute macro.
#[derive(Debug, FromMeta)]
pub(crate) struct EventArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: Path,
	/// Set the discriminator enum for this event.
	pub(crate) discriminator: Path,
	/// Set the variant of the discriminator enum.
	pub(crate) variant: Option<Ident>,
}

/// Arguments for the `#[error(...)]` attribute macro.
#[derive(Debug, FromMeta)]
pub(crate) struct ErrorArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: Path,
	/// Set whether the error enum is in it's final form.
	#[darling(rename = "final")]
	pub(crate) is_final: Flag,
}

/// Arguments for the `#[pda(...)]` attribute macro.
#[derive(Debug)]
pub(crate) struct PdaArgs {
	/// Set the path to the crate
	pub(crate) crate_path: Path,
	/// The typed PDA seed list.
	pub(crate) seeds: Vec<PdaSeedArg>,
	/// The name of the field that stores the PDA bump seed.
	pub(crate) bump: Option<Ident>,
}

/// A single element of a `#[pda(seeds = [...])]` list.
#[derive(Debug)]
pub(crate) enum PdaSeedArg {
	/// A byte-string literal seed (e.g. `b"counter"`).
	Constant(Vec<u8>),
	/// A reference to a `const &[u8]` seed (e.g. `SEED_COUNTER`).
	ConstantRef(Path),
	/// A typed dynamic seed (e.g. `authority: Address`).
	Variable { name: Ident, ty: SeedType },
}

/// The supported typed PDA seed parameter types.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SeedType {
	Address,
	U8,
	U16,
	U32,
	U64,
	Bytes(usize),
}

impl SeedType {
	/// The maximum number of seeds before the bump seed.
	pub(crate) const MAX_SEEDS: usize = 16;
	/// The maximum byte length of a single seed.
	pub(crate) const MAX_SEED_LEN: usize = 32;

	/// Whether the generated seed field borrows data for its lifetime.
	pub(crate) const fn borrows(self) -> bool {
		matches!(self, Self::Address)
	}

	/// The Rust type used for the constructor parameter.
	pub(crate) fn param_type(self) -> Type {
		match self {
			SeedType::Address => syn::parse_quote!(&Address),
			SeedType::U8 => syn::parse_quote!(u8),
			SeedType::U16 => syn::parse_quote!(u16),
			SeedType::U32 => syn::parse_quote!(u32),
			SeedType::U64 => syn::parse_quote!(u64),
			SeedType::Bytes(len) => syn::parse_quote!([u8; #len]),
		}
	}

	/// The constructor parameter type with an explicit lifetime, used by the
	/// `seeds()` method so multiple `&Address` parameters can share one
	/// lifetime.
	pub(crate) fn param_type_lt(self) -> Type {
		match self {
			SeedType::Address => syn::parse_quote!(&'a Address),
			_ => self.param_type(),
		}
	}

	/// The field type stored in the generated seeds struct.
	pub(crate) fn field_type(self) -> Type {
		match self {
			SeedType::Address => syn::parse_quote!(&'a Address),
			SeedType::U8 => syn::parse_quote!([u8; 1]),
			SeedType::U16 => syn::parse_quote!([u8; 2]),
			SeedType::U32 => syn::parse_quote!([u8; 4]),
			SeedType::U64 => syn::parse_quote!([u8; 8]),
			SeedType::Bytes(len) => syn::parse_quote!([u8; #len]),
		}
	}

	/// The expression that stores a constructor parameter in the struct field.
	pub(crate) fn stored_expr(&self, param: &Ident) -> proc_macro2::TokenStream {
		match self {
			SeedType::Address | SeedType::Bytes(_) => quote::quote!(#param),
			SeedType::U8 => quote::quote!([#param]),
			SeedType::U16 | SeedType::U32 | SeedType::U64 => {
				quote::quote!(#param.to_le_bytes())
			}
		}
	}

	/// The expression that turns a struct field into a seed byte slice.
	pub(crate) fn slice_expr(self, field: &Ident) -> proc_macro2::TokenStream {
		match self {
			SeedType::Address => quote::quote!(self.#field.as_ref()),
			SeedType::U8 | SeedType::U16 | SeedType::U32 | SeedType::U64 | SeedType::Bytes(_) => {
				quote::quote!(&self.#field)
			}
		}
	}

	/// The expression that turns a field of the inner seeds struct into a
	/// seed byte slice (used by the `SeedsWithBump` wrapper).
	pub(crate) fn slice_expr_inner(self, field: &Ident) -> proc_macro2::TokenStream {
		match self {
			SeedType::Address => quote::quote!(self.inner.#field.as_ref()),
			SeedType::U8 | SeedType::U16 | SeedType::U32 | SeedType::U64 | SeedType::Bytes(_) => {
				quote::quote!(&self.inner.#field)
			}
		}
	}
}

impl syn::parse::Parse for PdaArgs {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let mut crate_path = default_crate_path();
		let mut seeds = None;
		let mut bump = None;

		while !input.is_empty() {
			let key: Ident = input.call(Ident::parse_any)?;
			input.parse::<syn::Token![=]>()?;

			if key == "seeds" {
				if seeds.is_some() {
					return Err(syn::Error::new(key.span(), "duplicate `seeds` argument"));
				}
				seeds = Some(parse_seed_list(input)?);
			} else if key == "bump" {
				if bump.is_some() {
					return Err(syn::Error::new(key.span(), "duplicate `bump` argument"));
				}
				bump = Some(input.parse()?);
			} else if key == "crate" {
				crate_path = input.parse()?;
			} else {
				return Err(syn::Error::new(
					key.span(),
					format!(
						"unknown `#[pda]` argument `{key}`; expected `seeds`, `bump`, or `crate`"
					),
				));
			}

			if input.is_empty() {
				break;
			}
			input.parse::<syn::Token![,]>()?;
		}

		let seeds = seeds.ok_or_else(|| {
			syn::Error::new(
				input.span(),
				"`seeds` is required (e.g. `seeds = [b\"counter\", authority: Address]`)",
			)
		})?;

		Ok(Self {
			crate_path,
			seeds,
			bump,
		})
	}
}

/// Parse a `[b"literal", name: Type, ...]` seed list.
///
/// The list is parsed from raw tokens because `name: Type` type ascriptions
/// are not valid Rust expressions.
fn parse_seed_list(input: syn::parse::ParseStream) -> syn::Result<Vec<PdaSeedArg>> {
	let content;
	syn::bracketed!(content in input);

	let mut seeds = Vec::new();
	while !content.is_empty() {
		if content.peek(syn::LitByteStr) {
			let lit: syn::LitByteStr = content.parse()?;
			let value = lit.value();
			if value.len() > SeedType::MAX_SEED_LEN {
				return Err(syn::Error::new_spanned(
					&lit,
					format!(
						"seed literal is {} bytes, exceeds the maximum of {}",
						value.len(),
						SeedType::MAX_SEED_LEN
					),
				));
			}
			seeds.push(PdaSeedArg::Constant(value));
		} else if content.peek2(syn::Token![:]) && !content.peek3(syn::Token![:]) {
			let name: Ident = content.parse()?;
			content.parse::<syn::Token![:]>()?;
			let ty: Type = content.parse()?;
			let ty = parse_seed_type(&ty)?;
			seeds.push(PdaSeedArg::Variable { name, ty });
		} else {
			// A path to a `const &[u8]` seed (e.g. `SEED_COUNTER`).
			let path: Path = content.parse()?;
			seeds.push(PdaSeedArg::ConstantRef(path));
		}

		if content.is_empty() {
			break;
		}
		content.parse::<syn::Token![,]>()?;
	}

	if seeds.len() > SeedType::MAX_SEEDS {
		return Err(syn::Error::new(
			input.span(),
			format!(
				"PDA seed list has {} seeds before the bump; the maximum is {}",
				seeds.len(),
				SeedType::MAX_SEEDS
			),
		));
	}

	Ok(seeds)
}

/// Parse a typed seed parameter type.
fn parse_seed_type(ty: &Type) -> syn::Result<SeedType> {
	let error = |span: &dyn ToTokens| {
		syn::Error::new_spanned(
			span,
			"unsupported seed type; expected `Address`, `u8`, `u16`, `u32`, `u64`, or `[u8; N]`",
		)
	};

	match ty {
		Type::Path(type_path) => {
			let Some(ident) = type_path.path.get_ident() else {
				return Err(error(ty));
			};
			match ident.to_string().as_str() {
				"Address" => Ok(SeedType::Address),
				"u8" => Ok(SeedType::U8),
				"u16" => Ok(SeedType::U16),
				"u32" => Ok(SeedType::U32),
				"u64" => Ok(SeedType::U64),
				_ => Err(error(ident)),
			}
		}
		Type::Array(array) => {
			let Type::Path(item_path) = &*array.elem else {
				return Err(error(ty));
			};
			let Some(ident) = item_path.path.get_ident() else {
				return Err(error(ty));
			};
			if ident != "u8" {
				return Err(error(ty));
			}
			let Expr::Lit(lit) = &array.len else {
				return Err(error(ty));
			};
			let syn::Lit::Int(int) = &lit.lit else {
				return Err(error(ty));
			};
			let len = int.base10_parse::<usize>().map_err(|e| {
				syn::Error::new_spanned(ty, format!("invalid seed array length: {e}"))
			})?;
			if len == 0 || len > SeedType::MAX_SEED_LEN {
				return Err(syn::Error::new_spanned(
					ty,
					format!(
						"seed array length must be between 1 and {}",
						SeedType::MAX_SEED_LEN
					),
				));
			}
			Ok(SeedType::Bytes(len))
		}
		_ => Err(error(ty)),
	}
}

fn default_crate_path() -> Path {
	syn::parse_str("::pina")
		.unwrap_or_else(|e| panic!("internal error: failed to parse default crate path: {e}"))
}

/// Arguments for the `#[discriminator(...)]` attribute macro.
#[derive(Debug, FromMeta)]
pub(crate) struct DiscriminatorArgs {
	/// Set the primitive type that this enum discriminator will use. Can be one
	/// of:
	/// - `u8` (default)
	/// - `u16`
	/// - `u32`
	/// - `u64`
	#[darling(default = "Primitive::default")]
	pub(crate) primitive: Primitive,
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: Path,
	/// Set whether the error enum is in it's final form.
	#[darling(rename = "final")]
	pub(crate) is_final: Flag,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum Primitive {
	#[default]
	U8,
	U16,
	U32,
	U64,
}

// This allows the enum to be used in a `quote!` macro.
impl ToTokens for Primitive {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		let ty = match self {
			Primitive::U8 => quote::quote!(u8),
			Primitive::U16 => quote::quote!(u16),
			Primitive::U32 => quote::quote!(u32),
			Primitive::U64 => quote::quote!(u64),
		};

		tokens.extend(ty);
	}
}

impl Primitive {
	/// The byte width of the primitive, used to produce a concrete error
	/// message if it ever exceeds `MAX_DISCRIMINATOR_SPACE`.
	pub(crate) fn byte_size(self) -> usize {
		match self {
			Primitive::U8 => 1,
			Primitive::U16 => 2,
			Primitive::U32 => 4,
			Primitive::U64 => 8,
		}
	}
}

impl FromMeta for Primitive {
	fn from_expr(expr: &Expr) -> darling::Result<Self> {
		let error = darling::Error::unsupported_format(
			"Expected a primitive type path. Must be one of: `u8`, `u16`, `u32`, `u64`.",
		)
		.with_span(expr);
		match expr {
			Expr::Path(path) => {
				let Some(ident) = path.path.get_ident() else {
					return Err(error);
				};

				let ident_str = ident.to_string();
				match ident_str.as_str() {
					"u8" => Ok(Primitive::U8),
					"u16" => Ok(Primitive::U16),
					"u32" => Ok(Primitive::U32),
					"u64" => Ok(Primitive::U64),
					"u128" => {
						Err(darling::Error::custom(
							"A discriminator with primitive `u128` (16 bytes) exceeds \
							 `MAX_DISCRIMINATOR_SPACE` and cannot be safely used for zero-copy \
							 layouts. Supported primitives: `u8`, `u16`, `u32`, `u64`.",
						)
						.with_span(&ident))
					}
					_ => {
						Err(darling::Error::custom(
							"Unsupported primitive type. Must be one of: `u8`, `u16`, `u32`, \
							 `u64`.",
						)
						.with_span(&ident))
					}
				}
			}
			Expr::Group(group) => Self::from_expr(&group.expr),
			_ => Err(error),
		}
	}
}

/// Parsed input for `#[derive(Accounts)]`.
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(pina), supports(struct_named))]
pub(crate) struct AccountsInput {
	pub(crate) ident: Ident,
	pub(crate) generics: Generics,
	pub(crate) data: Data<Ignored, AccountsField>,
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: Path,
}

#[derive(Debug, FromField)]
#[darling(attributes(pina))]
pub(crate) struct AccountsField {
	pub(crate) ident: Option<Ident>,
	pub(crate) ty: Type,
	#[darling(default)]
	pub(crate) remaining: Flag,
	#[darling(default)]
	pub(crate) distinct: Option<bool>,
}
