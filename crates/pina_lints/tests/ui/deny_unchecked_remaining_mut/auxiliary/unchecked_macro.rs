#![crate_type = "lib"]

#[macro_export]
macro_rules! unchecked_external_remaining_mut {
	($cursor:expr) => {
		$cursor.remaining_mut()
	};
}
