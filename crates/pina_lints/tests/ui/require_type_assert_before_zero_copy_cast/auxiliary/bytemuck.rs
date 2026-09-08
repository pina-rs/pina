#![crate_type = "lib"]

pub fn try_from_bytes<T>(bytes: &[u8]) -> Result<&T, ()> {
	let _ = bytes;
	Err(())
}

pub fn cast_ref<T>(value: &T) -> Result<&T, ()> {
	Ok(value)
}
