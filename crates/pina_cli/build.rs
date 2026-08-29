fn main() {
	let target = std::env::var("TARGET").unwrap_or_else(|error| {
		panic!("Cargo did not define TARGET for the build script: {error}")
	});
	println!("cargo:rustc-env=PINA_BUILD_TARGET={target}");
}
