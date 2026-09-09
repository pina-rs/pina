pub struct AccountView;
pub struct RefMut;

impl AccountView {
	pub fn try_borrow_mut(&mut self) -> Result<RefMut, ()> {
		Ok(RefMut)
	}
}
