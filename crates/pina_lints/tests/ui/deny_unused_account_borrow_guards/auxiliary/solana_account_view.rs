pub struct AccountView;
pub struct Ref;
pub struct RefMut;
pub struct Value;

impl AccountView {
	pub fn try_borrow(&self) -> Result<Ref, ()> {
		Ok(Ref)
	}

	pub fn try_borrow_mut(&mut self) -> Result<RefMut, ()> {
		Ok(RefMut)
	}
}

impl Ref {
	pub fn value(&self) -> u8 {
		0
	}

	pub fn into_value(self) -> Value {
		Value
	}
}

impl RefMut {
	pub fn value(&self) -> u8 {
		0
	}
}
