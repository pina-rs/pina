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
fn compact_accounts_program_cli_snapshot() {
	insta::assert_snapshot!(
		"compact_accounts_program_cli",
		render_fixture_files("compact_accounts_program")
	);
}

#[test]
fn rejects_idl_names_that_break_rust_identifiers() {
	let program_json = |name: &str, instruction: &str| {
		format!(
			concat!(
				r#"{{"kind":"rootNode","standard":"codama","version":"1.0.0","#,
				r#""program":{{"kind":"programNode","name":"{name}","#,
				r#""publicKey":"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS","version":"0.0.0","#,
				r#""instructions":[{{"kind":"instructionNode","name":"{instruction}","accounts":[],"#,
				r#""arguments":[]}}],"accounts":[],"pdas":[]}}}}"#
			),
			name = name,
			instruction = instruction,
		)
	};
	let model_from = |json: &str| {
		let root: codama_nodes::RootNode =
			serde_json::from_str(json).unwrap_or_else(|error| panic!("parse idl: {error}"));
		super::model::CliModel::from_root(&root)
	};

	let keyword = model_from(&program_json("counter_program", "match"))
		.unwrap_err()
		.to_string();
	assert!(
		keyword.contains("Rust keyword `match`"),
		"unexpected error: {keyword}"
	);

	let leading_digit = model_from(&program_json("3proxy", "do_work"))
		.unwrap_err()
		.to_string();
	assert!(
		leading_digit.contains("invalid identifier `3proxy`"),
		"unexpected error: {leading_digit}"
	);

	let instruction_keyword = model_from(&program_json("counter_program", "self"))
		.unwrap_err()
		.to_string();
	assert!(
		instruction_keyword.contains("Rust keyword `self`"),
		"unexpected error: {instruction_keyword}"
	);

	assert!(
		model_from(&program_json("counter_program", "increment_v2")).is_ok(),
		"ordinary names must keep rendering"
	);
}

#[test]
fn rendering_without_a_scaffold_writes_and_tracks_sources_only() {
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let temp_root = fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize temp dir: {error}"));
	let root = temp_root.join("cli");
	let fixture_root = read_root_node(&fixture("counter_program"))
		.unwrap_or_else(|error| panic!("fixture: {error}"));
	let mut sources_only = config();
	sources_only.scaffold = false;

	render_root_node(&fixture_root, &root, &sources_only)
		.unwrap_or_else(|error| panic!("render without a scaffold: {error}"));

	let record = fs::read_to_string(root.join(".pina-generated.json"))
		.unwrap_or_else(|error| panic!("read record: {error}"));
	assert!(root.join("src/main.rs").is_file());
	assert!(!root.join("Cargo.toml").exists());
	assert!(record.contains("src/main.rs"));
	assert!(
		!record.contains("Cargo.toml"),
		"a scaffold that was never written is not tracked: {record}"
	);
}

#[test]
fn requests_the_client_limit_only_for_measured_instructions()
-> Result<(), Box<dyn std::error::Error>> {
	let command_for = |plugins: &str| -> Result<String, Box<dyn std::error::Error>> {
		let json = format!(
			concat!(
				r#"{{"kind":"rootNode","standard":"codama","version":"1.0.0","#,
				r#""program":{{"kind":"programNode","name":"counterProgram","#,
				r#""publicKey":"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS","version":"0.0.0","#,
				r#""instructions":[{{"kind":"instructionNode","name":"makeOffer","accounts":[],"#,
				r#""arguments":[],"plugins":[{plugins}]}}],"accounts":[],"pdas":[]}}}}"#
			),
			plugins = plugins,
		);
		let root: codama_nodes::RootNode = serde_json::from_str(&json)?;
		let model = super::model::CliModel::from_root(&root)?;
		let mut files = render_files(&model, "fixture-client")?;

		Ok(files
			.remove("src/commands/make_offer.rs")
			.ok_or("the instruction renders a command")?)
	};

	let measured = command_for(
		r#"{"kind":"pluginNode","name":"pinaComputeUnits","payload":{"measured":379,"limit":800}}"#,
	)?;
	assert!(
		measured.contains(
			"context.send(accounts.instruction(data), \
			 Some(fixture_client::instructions::MAKE_OFFER_COMPUTE_UNIT_LIMIT))"
		),
		"{measured}"
	);

	let unmeasured = command_for(r#"{"kind":"pluginNode","name":"anchor"}"#)?;
	assert!(
		unmeasured.contains("context.send(accounts.instruction(data), None)"),
		"{unmeasured}"
	);

	Ok(())
}

#[test]
fn render_root_node_enforces_modes_and_safety() {
	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let temp_root = fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize temp dir: {error}"));
	let root = temp_root.join("cli");
	let fixture_root = || {
		read_root_node(&fixture("counter_program"))
			.unwrap_or_else(|error| panic!("fixture: {error}"))
	};
	render_root_node(&fixture_root(), &root, &config())
		.unwrap_or_else(|error| panic!("create mode renders: {error}"));
	assert!(root.join("src/main.rs").is_file());
	assert!(root.join("Cargo.toml").is_file());
	assert!(
		root.join(super::MARKER_FILE).is_file(),
		"created crates carry the regeneration marker"
	);

	// An existing tree without the marker was not generated by this renderer,
	// so update mode must refuse it instead of clobbering user edits.
	let foreign = temp_root.join("foreign");
	fs::create_dir_all(&foreign).unwrap_or_else(|error| panic!("mkdir: {error}"));
	fs::write(foreign.join("notes.txt"), "hand-written")
		.unwrap_or_else(|error| panic!("write: {error}"));
	let mut update = config();
	update.mode = RenderMode::Update;
	let update = update;
	assert!(
		render_root_node(&fixture_root(), &foreign, &update).is_err(),
		"update mode must refuse trees without the marker"
	);
	assert!(
		foreign.join("notes.txt").is_file(),
		"refused destinations are left untouched"
	);

	let mut overwrite_foreign = config();
	overwrite_foreign.mode = RenderMode::Overwrite;
	let refusal = render_root_node(&fixture_root(), &foreign, &overwrite_foreign)
		.expect_err("overwrite must refuse an untracked nonempty destination");
	assert!(
		matches!(refusal, super::RenderError::InvalidGenerationState { .. }),
		"the refusal names the remedy: {refusal}"
	);
	assert!(foreign.join("notes.txt").is_file());

	let mut update_generated = config();
	update_generated.mode = RenderMode::Update;
	fs::write(root.join("Cargo.toml"), "# custom manifest")
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
	render_root_node(&fixture_root(), &root, &update_generated)
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

	fs::write(root.join("keep.txt"), "keep")
		.unwrap_or_else(|error| panic!("write keep file: {error}"));
	let mut overwrite = config();
	overwrite.mode = RenderMode::Overwrite;
	render_root_node(&fixture_root(), &root, &overwrite)
		.unwrap_or_else(|error| panic!("overwrite mode replaces the crate: {error}"));
	assert_ne!(
		fs::read_to_string(root.join("Cargo.toml"))
			.unwrap_or_else(|error| panic!("read manifest: {error}")),
		"# custom manifest",
		"the scaffold stays tracked through an update, so overwrite regenerates it"
	);
	assert!(root.join(super::MARKER_FILE).is_file());
	assert!(
		root.join("keep.txt").is_file(),
		"untracked files survive the manifest-bounded overwrite"
	);
}

#[cfg(unix)]
#[test]
fn tracked_overwrite_still_refuses_repository_trees() {
	use std::os::unix::fs::symlink;

	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp: {error}"));
	let temp_root = fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize temp dir: {error}"));
	let root = read_root_node(&fixture("counter_program"))
		.unwrap_or_else(|error| panic!("fixture: {error}"));

	// A tracked tree that contains a repository is refused even though
	// deletion is manifest-bounded.
	let git_tree = temp_root.join("git-tree");
	fs::create_dir_all(git_tree.join(".git")).unwrap_or_else(|error| panic!("mkdir: {error}"));
	super::generation_manifest::write(
		&git_tree,
		&std::collections::BTreeSet::from([std::path::PathBuf::from("src/main.rs")]),
	)
	.unwrap_or_else(|error| panic!("manifest write: {error}"));

	let mut overwrite = config();
	overwrite.mode = RenderMode::Overwrite;
	let refusal = render_root_node(&root, &git_tree, &overwrite)
		.expect_err("overwrite must refuse a repository tree");
	assert!(
		matches!(refusal, super::RenderError::UnsafeOutputPath { .. }),
		"the guards still apply to tracked trees: {refusal}"
	);
	assert!(git_tree.join(".git").is_dir());
}

#[cfg(unix)]
#[test]
fn overwrite_rejects_a_symlinked_destination_ancestor() {
	use std::os::unix::fs::symlink;

	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let temp_root = fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize temp dir: {error}"));
	let real = temp_root.join("real");
	let link = temp_root.join("link");
	let external_crate = real.join("generated");
	let sentinel = external_crate.join("sentinel.txt");
	fs::create_dir_all(&external_crate).unwrap_or_else(|error| panic!("mkdir: {error}"));
	fs::write(&sentinel, "preserve").unwrap_or_else(|error| panic!("write: {error}"));
	symlink(&real, &link).unwrap_or_else(|error| panic!("symlink: {error}"));

	let root = read_root_node(&fixture("counter_program"))
		.unwrap_or_else(|error| panic!("fixture: {error}"));
	let mut overwrite = config();
	overwrite.mode = RenderMode::Overwrite;

	assert!(matches!(
		render_root_node(&root, &link.join("generated"), &overwrite),
		Err(super::RenderError::UnsafeOutputPath { .. })
	));
	assert!(sentinel.is_file());

	let blocked = temp_root.join("blocked");
	fs::write(&blocked, "not a directory").unwrap_or_else(|error| panic!("write: {error}"));
	assert!(render_root_node(&root, &blocked.join("generated"), &overwrite).is_err());
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

#[cfg(unix)]
#[test]
fn only_top_level_root_owned_links_are_trusted() {
	use std::os::unix::fs::symlink;

	let temp = tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let root = fs::canonicalize(temp.path())
		.unwrap_or_else(|error| panic!("canonicalize temp dir: {error}"));
	let link = root.join("link");
	symlink(&root, &link).unwrap_or_else(|error| panic!("symlink failed: {error}"));
	let link_metadata =
		fs::symlink_metadata(&link).unwrap_or_else(|error| panic!("metadata failed: {error}"));
	let directory_metadata =
		fs::symlink_metadata(&root).unwrap_or_else(|error| panic!("metadata failed: {error}"));

	// Below the top level every link is untrusted, whoever owns it, so a
	// renderer running as root still rejects a project's own links.
	assert!(super::is_untrusted_link(&link_metadata, false));
	assert_eq!(
		super::is_untrusted_link(&link_metadata, true),
		!super::is_owned_by_root(&link_metadata)
	);
	assert!(!super::is_untrusted_link(&directory_metadata, true));
	assert!(!super::is_untrusted_link(&directory_metadata, false));
	assert!(matches!(
		super::validate_output_path_components(&link.join("generated")),
		Err(super::RenderError::UnsafeOutputPath { .. })
	));
}

#[cfg(unix)]
#[test]
fn system_aliases_below_the_filesystem_root_are_trusted() {
	let aliases = fs::read_dir("/")
		.unwrap_or_else(|error| panic!("read root failed: {error}"))
		.map(|entry| entry.unwrap_or_else(|error| panic!("root entry failed: {error}")))
		.map(|entry| entry.path())
		.filter(|path| {
			fs::symlink_metadata(path).is_ok_and(|metadata| {
				super::is_link_like(&metadata) && super::is_owned_by_root(&metadata)
			}) && path.is_dir()
		})
		.collect::<Vec<_>>();

	// macOS resolves every temporary directory through `/var` or `/tmp`.
	#[cfg(target_os = "macos")]
	for expected in ["/var", "/tmp"] {
		assert!(
			aliases.iter().any(|alias| alias == Path::new(expected)),
			"{expected} should be a root-owned system alias"
		);
	}

	for alias in aliases {
		let metadata =
			fs::symlink_metadata(&alias).unwrap_or_else(|error| panic!("metadata failed: {error}"));
		assert!(!super::is_untrusted_link(&metadata, true));
		super::validate_output_path_components(&alias.join("pina-missing-directory/generated"))
			.unwrap_or_else(|error| panic!("{} should be trusted: {error}", alias.display()));
	}
}
