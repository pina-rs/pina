declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum SemanticOptionsAccount {
	SemanticOptionsState = 1,
}

/// Account exercising native and explicit zeropod option layouts.
#[account(discriminator = SemanticOptionsAccount)]
pub struct SemanticOptionsState {
	/// The ergonomic one-byte-tag schema form.
	pub native: Option<u64>,
	/// An explicit two-byte tag for compatibility with an existing ABI.
	pub wide: PodOption<PodU64, 2>,
	/// A nested fixed-capacity string.
	pub label: Option<String<8>>,
	/// Options remain fixed-size zeropod elements inside vectors.
	pub values: Vec<Option<u16>, 3>,
}
