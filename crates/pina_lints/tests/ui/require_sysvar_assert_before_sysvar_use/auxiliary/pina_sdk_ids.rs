#![allow(dead_code)]

pub mod sysvar {
	pub mod clock {
		pub static ID: () = ();
		pub const NAME: &str = "clock";
	}

	pub mod rent {
		pub static ID: () = ();
	}
}
