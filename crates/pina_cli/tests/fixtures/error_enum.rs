declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[error]
pub enum FixtureError {
	/// The account state is invalid for this operation.
	InvalidState = 6000,
	/// The instruction requires a signer that was not provided.
	MissingSigner = 6001,
}
