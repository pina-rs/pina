// build.rs
fn main() {
	// This line tells rustc to expect the `solana` cfg flag.
	println!("cargo:rustc-check-cfg=cfg(solana)");
}
