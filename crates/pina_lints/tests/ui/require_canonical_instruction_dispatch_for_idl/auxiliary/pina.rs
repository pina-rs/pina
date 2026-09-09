#![crate_type = "lib"]

pub fn parse_instruction<T: Default>(_data: &[u8]) -> Result<T, ()> {
	Ok(T::default())
}
