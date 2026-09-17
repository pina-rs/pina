//! Re-expand Pina macros when the migration manifest changes.

fn main() {
	println!("cargo:rerun-if-changed=migrations/manifest.json");
}
