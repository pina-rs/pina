use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::RenderConfig;
use super::RenderMode;
use super::emit::render_files;
use super::emit::render_scaffold;
use super::read_root_node;
use super::render_root_node;

fn fixture(name: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
		.join("codama/idls")
		.join(format!("{name}.json"))
}

fn config() -> RenderConfig {
	RenderConfig {
		mode: RenderMode::Create,
		scaffold: true,
		client_package: "fixture-client".to_string(),
		client_path: "../../rust/fixture".to_string(),
	}
}

fn render_fixture_files(name: &str) -> String {
	let root = read_root_node(&fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
	let model =
		super::model::CliModel::from_root(&root).unwrap_or_else(|error| panic!("{name}: {error}"));
	let mut files =
		render_files(&model, "fixture-client").unwrap_or_else(|error| panic!("{name}: {error}"));
	files.extend(render_scaffold(
		&model,
		"fixture-client",
		"../../rust/fixture",
	));
	files
		.iter()
		.map(|(path, contents)| format!("==== {path} ====\n{contents}"))
		.collect::<Vec<_>>()
		.join("\n")
}

#[test]
fn counter_program_cli_snapshot() {
	insta::assert_snapshot!(
		"counter_program_cli",
		render_fixture_files("counter_program")
	);
}

#[test]
fn profile_program_cli_snapshot() {
	insta::assert_snapshot!(
		"profile_program_cli",
		render_fixture_files("profile_program")
	);
}

#[test]
fn escrow_program_cli_snapshot() {
	insta::assert_snapshot!("escrow_program_cli", render_fixture_files("escrow_program"));
}

#[test]
fn todo_program_cli_snapshot() {
	insta::assert_snapshot!("todo_program_cli", render_fixture_files("todo_program"));
}

#[test]
fn custom_errors_program_cli_snapshot() {
	insta::assert_snapshot!(
		"custom_errors_program_cli",
		render_fixture_files("custom_errors_program")
	);
}

#[test]
fn optional_accounts_program_cli_snapshot() {
	insta::assert_snapshot!(
		"optional_accounts_program_cli",
		render_fixture_files("optional_accounts_program")
	);
}

#[test]
fn render_root_node_enforces_modes_and_safety() {
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let root = temp.path().join("cli");
	let fixture_root = || {
		read_root_node(&fixture("counter_program"))
			.unwrap_or_else(|error| panic!("fixture: {error}"))
	};
	render_root_node(&fixture_root(), &root, &config())
		.unwrap_or_else(|error| panic!("create mode renders: {error}"));
	assert!(root.join("src/main.rs").is_file());
	assert!(root.join("Cargo.toml").is_file());

	let mut update = config();
	update.mode = RenderMode::Update;
	fs::write(root.join("Cargo.toml"), "# custom manifest")
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
	render_root_node(&fixture_root(), &root, &update)
		.unwrap_or_else(|error| panic!("update mode preserves the manifest: {error}"));
	assert_eq!(
		fs::read_to_string(root.join("Cargo.toml"))
			.unwrap_or_else(|error| panic!("read manifest: {error}")),
		"# custom manifest"
	);

	let mut create_over_existing = config();
	create_over_existing.mode = RenderMode::Create;
	assert!(
		render_root_node(&fixture_root(), &root, &create_over_existing).is_err(),
		"create mode must reject a nonempty destination"
	);

	let mut overwrite = config();
	overwrite.mode = RenderMode::Overwrite;
	render_root_node(&fixture_root(), &root, &overwrite)
		.unwrap_or_else(|error| panic!("overwrite mode replaces the crate: {error}"));
	assert!(root.join("Cargo.toml").is_file());
}

#[test]
fn scaffold_requires_client_configuration() {
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let root = read_root_node(&fixture("counter_program"))
		.unwrap_or_else(|error| panic!("fixture: {error}"));

	let mut missing_path = config();
	missing_path.client_path = String::default();
	assert!(matches!(
		render_root_node(&root, &temp.path().join("a"), &missing_path),
		Err(super::RenderError::MissingClientConfig)
	));

	let mut missing_package = config();
	missing_package.client_package = String::default();
	assert!(matches!(
		render_root_node(&root, &temp.path().join("b"), &missing_package),
		Err(super::RenderError::MissingClientConfig)
	));
}
