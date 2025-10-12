use darling::FromMeta;

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
	/// Set the primitive type that this enum discriminator will use.
	#[darling(default = "default_primitive_type")]
	pub(crate) primitive: syn::Type,
	/// Set the path to the crate
	#[darling(default = "default_crate_path", rename = "crate")]
	pub(crate) crate_path: syn::Path,
	/// Set whether the error enum is in it's final form.
	#[darling(rename = "final")]
	pub(crate) is_final: darling::util::Flag,
}

fn default_primitive_type() -> syn::Type {
	syn::parse_str("u8").unwrap()
}
