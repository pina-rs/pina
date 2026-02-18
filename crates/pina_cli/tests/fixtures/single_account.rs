declare_id!("GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS");

#[discriminator]
pub enum SingleAccountDiscriminator {
	SingleAccountState = 7,
}

#[account(discriminator = SingleAccountDiscriminator)]
pub struct SingleAccountState {
	pub bump: u8,
	pub authority: Address,
}
