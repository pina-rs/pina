use pina::Address;
use pina::CpiContext;
use pina::CpiHandle;
use pina::ToCpiAccounts;

#[derive(Clone, Copy)]
struct NoAccounts;

impl<'a> ToCpiAccounts<'a, 0> for NoAccounts {
	fn to_cpi_handles(&self) -> [CpiHandle<'a>; 0] {
		[]
	}
}

fn main() {
	let raw_program_address = Address::new_from_array([1; 32]);
	let _context = CpiContext::new(&raw_program_address, NoAccounts);
}
