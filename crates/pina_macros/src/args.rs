use darling::FromMeta;
use quote::ToTokens;
use syn::Expr;

#[derive(Debug, FromMeta)]
pub(crate) struct AccountArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: syn::Path,
	/// Set the discriminator enum for this account.
	pub(crate) discriminator: syn::Path,
}

#[derive(Debug, FromMeta)]
pub(crate) struct InstructionArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: syn::Path,
	/// Set the discriminator enum for this instruction.
	pub(crate) discriminator: syn::Path,
}

#[derive(Debug, FromMeta)]
pub(crate) struct EventArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: syn::Path,
	/// Set the discriminator enum for this event.
	pub(crate) discriminator: syn::Path,
	/// Set the variant of the discriminator enum.
	pub(crate) variant: Option<syn::Ident>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct ErrorArgs {
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: syn::Path,
	/// Set whether the error enum is in it's final form.
	#[darling(rename = "final")]
	pub(crate) is_final: darling::util::Flag,
}

fn default_crate_path() -> syn::Path {
	syn::parse_str("::pina").unwrap()
}

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
	pub(crate) crate_path: syn::Path,
	/// Set whether the error enum is in it's final form.
	#[darling(rename = "final")]
	pub(crate) is_final: darling::util::Flag,
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
