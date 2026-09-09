#![crate_type = "lib"]

pub fn cast<T>(_bytes: [u8; 8]) -> T {
	panic!()
}

pub fn try_cast<T>(_bytes: [u8; 8]) -> Result<T, ()> {
	Err(())
}

pub fn from_bytes<T>(_bytes: &[u8]) -> &T {
	panic!()
}

pub fn from_bytes_mut<T>(_bytes: &mut [u8]) -> &mut T {
	panic!()
}

pub fn try_from_bytes<T>(bytes: &[u8]) -> Result<&T, ()> {
	let _ = bytes;
	Err(())
}

pub fn try_from_bytes_mut<T>(_bytes: &mut [u8]) -> Result<&mut T, ()> {
	Err(())
}

pub fn pod_read_unaligned<T>(_bytes: &[u8]) -> T {
	panic!()
}

pub fn try_pod_read_unaligned<T>(_bytes: &[u8]) -> Result<T, ()> {
	Err(())
}

pub fn pod_align_to<T>(_bytes: &[u8]) -> (&[u8], &[T], &[u8]) {
	(&[], &[], &[])
}

pub fn pod_align_to_mut<T>(_bytes: &mut [u8]) -> (&mut [u8], &mut [T], &mut [u8]) {
	(&mut [], &mut [], &mut [])
}

pub fn cast_ref<T>(value: &T) -> Result<&T, ()> {
	Ok(value)
}

pub fn cast_mut<T>(value: &mut T) -> Result<&mut T, ()> {
	Ok(value)
}

pub fn try_cast_ref<T>(value: &T) -> Result<&T, ()> {
	Ok(value)
}

pub fn try_cast_mut<T>(value: &mut T) -> Result<&mut T, ()> {
	Ok(value)
}

pub fn cast_slice<T>(_bytes: &[u8]) -> &[T] {
	panic!()
}

pub fn cast_slice_mut<T>(_bytes: &mut [u8]) -> &mut [T] {
	panic!()
}

pub fn try_cast_slice<T>(_bytes: &[u8]) -> Result<&[T], ()> {
	Err(())
}

pub fn try_cast_slice_mut<T>(_bytes: &mut [u8]) -> Result<&mut [T], ()> {
	Err(())
}
