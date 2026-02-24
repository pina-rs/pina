use std::fs;
use std::path::Path;

use codama_nodes::RootNode;
use pina_cli::generate_idl;
use serde_json::Value;

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(|p| p.parent())
		.unwrap_or_else(|| Path::new("."))
}

fn example_programs(root: &Path) -> Vec<String> {
	let examples_dir = root.join("examples");
	let mut examples = fs::read_dir(examples_dir)
		.unwrap_or_else(|e| panic!("failed to read examples directory: {e}"))
		.filter_map(|entry| entry.ok())
		.filter_map(|entry| {
			let file_name = entry.file_name();
			let name = file_name.to_str()?;
			if entry.path().is_dir() {
				Some(name.to_owned())
			} else {
				None
			}
		})
		.collect::<Vec<_>>();

	examples.sort();
	examples
}

fn read_fixture(fixture_path: &Path) -> Value {
	let fixture_json = fs::read_to_string(fixture_path)
		.unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", fixture_path.display()));
	let root: RootNode = serde_json::from_str(&fixture_json)
		.unwrap_or_else(|e| panic!("invalid Codama fixture {}: {e}", fixture_path.display()));
	serde_json::to_value(root).unwrap_or_else(|e| {
		panic!(
			"failed to normalize fixture JSON {}: {e}",
			fixture_path.display()
		)
	})
}

#[test]
fn codama_idl_fixtures_match_generated_output() {
	let root = workspace_root();
	let examples = example_programs(root);
	assert!(
		!examples.is_empty(),
		"expected at least one example program"
	);

	let fixture_dir = root.join("codama").join("idls");
	let fixture_count = fs::read_dir(&fixture_dir)
		.unwrap_or_else(|e| {
			panic!(
				"failed to read fixture directory {}: {e}",
				fixture_dir.display()
			)
		})
		.filter_map(|entry| entry.ok())
		.filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
		.count();
	assert_eq!(
		fixture_count,
		examples.len(),
		"expected fixture count to match examples count"
	);

	for example_name in examples {
		let example_path = root.join("examples").join(&example_name);
		let fixture_path = fixture_dir.join(format!("{example_name}.json"));

		assert!(
			fixture_path.is_file(),
			"missing fixture for {}: {}",
			example_name,
			fixture_path.display()
		);

		let generated_root = generate_idl(&example_path, None).unwrap_or_else(|e| {
			panic!(
				"IDL generation failed for {} ({}): {e}",
				example_name,
				example_path.display()
			)
		});
		let generated_json = serde_json::to_value(generated_root).unwrap_or_else(|e| {
			panic!(
				"failed to serialize generated IDL for {}: {e}",
				example_name
			)
		});
		let fixture_json = read_fixture(&fixture_path);

		let generated_root: RootNode = serde_json::from_value(generated_json.clone())
			.unwrap_or_else(|e| {
				panic!(
					"failed to deserialize generated IDL for {}: {e}",
					example_name
				)
			});
		for instruction in &generated_root.program.instructions {
			assert!(
				!instruction.discriminators.is_empty(),
				"instruction '{:?}' in {} is missing discriminator metadata",
				instruction.name,
				example_name
			);
		}
		for account in &generated_root.program.accounts {
			assert!(
				!account.discriminators.is_empty(),
				"account '{:?}' in {} is missing discriminator metadata",
				account.name,
				example_name
			);
		}

		assert_eq!(
			generated_json, fixture_json,
			"IDL fixture drift detected for {}. Run `scripts/generate-codama-idls.sh` and commit \
			 the updated fixture.",
			example_name,
		);
	}
}
