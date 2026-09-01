use pina::*;

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct InitAccounts<'a> {
	pub payer: &'a AccountView,
	pub config: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct TransferAccounts<'a> {
	pub authority: &'a AccountView,
	pub source: &'a AccountView,
	pub destination: &'a AccountView,
	#[pina(remaining)]
	pub extra: &'a [AccountView],
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct MutableTransferAccounts<'a> {
	pub authority: &'a mut AccountView,
	#[pina(remaining)]
	pub extra: &'a mut [AccountView],
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct DuplicateMutableRemainingAccounts<'a> {
	pub authority: &'a AccountView,
	/// Duplicate accounts represent repeated weights in this instruction.
	#[pina(remaining, distinct = false)]
	pub extra: &'a mut [AccountView],
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct SingleAccount<'a> {
	pub account: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct EscrowAccounts<'a> {
	pub maker: &'a AccountView,
	pub escrow: &'a AccountView,
	pub mint_a: &'a AccountView,
	pub mint_b: &'a AccountView,
	pub maker_ata_a: &'a AccountView,
	pub vault: &'a AccountView,
	pub token_program: &'a AccountView,
	pub associated_token_program: &'a AccountView,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
pub struct DefaultCrateAccounts<'a> {
	pub authority: &'a AccountView,
	pub data: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct MakeAccounts<'a> {
	pub maker: &'a mut AccountView,
	pub escrow: Option<&'a mut AccountView>,
	pub witness: Option<&'a AccountView>,
	pub system_program: &'a AccountView,
}

#[derive(Accounts, Debug)]
#[pina(crate = pina)]
pub struct BadAccounts<'a> {
	pub payer: &'a AccountView,
	pub weird: Option<u8>,
}
