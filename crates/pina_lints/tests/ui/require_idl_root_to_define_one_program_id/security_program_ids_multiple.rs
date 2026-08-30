// normalize-stderr-test: "\n$" -> ""

#![allow(dead_code)]

// Two `declare_id!` expansions expose two program IDs for one example crate.

macro_rules! declare_id {
	($id:expr) => {
		pub const ID: &str = $id;
	};
}

mod authority {
	declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
}

mod migration {
	declare_id!("Nine2m0RDevWqwCnEVKioA22yB1q9FrAoIvNC5n2ehE2v");
}

fn entrypoint() -> Result<(), ()> {
	Ok(())
}

fn main() {}
