//! The `pina skill` surface, exercised through the spawned binary.
//!
//! The unit tests in `src/skill.rs` call the operations directly; these tests
//! own what only a real process can show — stdout that an agent captures
//! verbatim, and the exit status a script reads.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_file;
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(|path| path.parent())
		.unwrap_or_else(|| Path::new("."))
}

/// Scratch space inside the workspace, removed before each use.
fn scratch(name: &str) -> PathBuf {
	let path = workspace_root()
		.join("target")
		.join("pina-cli-skill-temp")
		.join(name);
	let _ = fs::remove_dir_all(&path);
	fs::create_dir_all(&path).unwrap_or_else(|error| {
		panic!(
			"failed to create skill scratch directory {}: {error}",
			path.display()
		)
	});
	path
}

fn pina(args: &[&str]) -> std::process::Output {
	Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(args)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina {args:?}: {error}"))
}

fn stdout(output: &std::process::Output) -> String {
	String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
	String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn bare_skill_lists_every_bundled_topic() {
	let output = pina(&["skill"]);
	assert!(output.status.success(), "stderr: {}", stderr(&output));
	let listing = stdout(&output);
	for topic in [
		"pina",
		"cli-and-codegen",
		"migrations",
		"program-authoring",
		"project-setup",
		"testing",
	] {
		assert!(
			listing.contains(topic),
			"`{topic}` must be listed: {listing}"
		);
	}
	assert!(listing.contains("pina skill read <topic>"), "{listing}");
	assert!(listing.contains("pina skill install --dir"), "{listing}");
}

#[test]
fn read_writes_the_document_itself_with_no_rendering() {
	let output = pina(&["skill", "read", "migrations"]);
	assert!(output.status.success(), "stderr: {}", stderr(&output));
	let document = stdout(&output);

	// The frontmatter and headings an agent parses must survive untouched:
	// terminal rendering would strip or restyle them.
	assert!(
		document.starts_with('#'),
		"{}",
		&document[..40.min(document.len())]
	);
	assert!(document.contains("abiVersion"));
	assert!(document.contains("--envelope-ack"));
}

#[test]
fn read_without_a_topic_lists_topics_and_succeeds() {
	let output = pina(&["skill", "read"]);
	assert!(output.status.success(), "stderr: {}", stderr(&output));
	assert!(stdout(&output).contains("migrations"));
}

#[test]
fn read_of_an_unknown_topic_fails_with_the_bundled_names() {
	let output = pina(&["skill", "read", "anchor"]);
	assert_eq!(output.status.code(), Some(1));
	let reported = stderr(&output);
	assert!(reported.contains("`anchor` is not bundled"), "{reported}");
	assert!(reported.contains("migrations"), "{reported}");
}

#[test]
fn install_writes_the_tree_and_reports_it() {
	let destination = scratch("install").join("pina");
	let target = destination.to_str().unwrap_or_else(|| panic!("utf-8 path"));
	let output = pina(&["skill", "install", "--dir", target]);
	assert!(output.status.success(), "stderr: {}", stderr(&output));
	let reported = stdout(&output);
	assert!(reported.contains("Installed 6 files into"), "{reported}");
	assert!(
		reported.contains("skill named `pina`"),
		"the report must say how to point a runtime at it: {reported}"
	);

	for file in [
		"SKILL.md",
		"references/cli-and-codegen.md",
		"references/migrations.md",
		"references/program-authoring.md",
		"references/project-setup.md",
		"references/testing.md",
	] {
		assert!(
			destination.join(file).is_file(),
			"`{file}` must be installed"
		);
	}
}

#[test]
fn install_refuses_to_overwrite_and_force_replaces() {
	let destination = scratch("clobber").join("pina");
	let target = destination.to_str().unwrap_or_else(|| panic!("utf-8 path"));

	let first = pina(&["skill", "install", "--dir", target]);
	assert!(first.status.success(), "stderr: {}", stderr(&first));

	let refused = pina(&["skill", "install", "--dir", target]);
	assert_eq!(refused.status.code(), Some(1));
	let reported = stderr(&refused);
	assert!(reported.contains("contains a skill"), "{reported}");
	assert!(reported.contains("--force"), "{reported}");

	let forced = pina(&["skill", "install", "--dir", target, "--force"]);
	assert!(forced.status.success(), "stderr: {}", stderr(&forced));
}

#[cfg(any(unix, windows))]
#[test]
fn install_rejects_linked_documents_including_dangling_links_even_with_force() {
	let root = scratch("linked-documents");

	for force in [false, true] {
		for dangling in [false, true] {
			for document in ["SKILL.md", "references/migrations.md"] {
				let case = tempfile::tempdir_in(&root).unwrap_or_else(|e| panic!("case: {e}"));
				let destination = case.path().join("pina");
				let external = case.path().join("external.json");
				fs::create_dir_all(destination.join("references")).unwrap();
				let earlier = destination.join("references/cli-and-codegen.md");

				if force {
					fs::write(&earlier, "curated reference").unwrap();
				}

				if !dangling {
					fs::write(&external, "external sentinel").unwrap();
				}

				symlink_file(&external, destination.join(document)).unwrap();
				let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
				command
					.args(["skill", "install", "--dir"])
					.arg(&destination);

				if force {
					command.arg("--force");
				}

				let output = command.output().unwrap();
				assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
				assert!(
					fs::symlink_metadata(destination.join(document))
						.unwrap()
						.is_symlink()
				);

				if force {
					assert_eq!(fs::read_to_string(&earlier).unwrap(), "curated reference");
				} else {
					assert!(!earlier.exists());
				}

				if dangling {
					assert!(!external.exists());
				} else {
					assert_eq!(fs::read_to_string(&external).unwrap(), "external sentinel");
				}
			}
		}
	}
}

#[cfg(any(unix, windows))]
#[test]
fn install_rejects_linked_directories_and_ancestors_even_with_force() {
	let root = scratch("linked-directories");

	for force in [false, true] {
		for linked in ["pina", "ancestor", "pina/references"] {
			let case = tempfile::tempdir_in(&root).unwrap();
			let external = case.path().join("external");
			let link = case.path().join(linked);
			fs::create_dir(&external).unwrap();
			fs::write(external.join("sentinel"), "external sentinel").unwrap();
			fs::create_dir_all(link.parent().unwrap()).unwrap();
			symlink_dir(&external, &link).unwrap();
			let destination = if linked == "ancestor" {
				link.join("pina")
			} else {
				case.path().join("pina")
			};
			let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
			command
				.args(["skill", "install", "--dir"])
				.arg(&destination);

			if force {
				command.arg("--force");
			}

			let output = command.output().unwrap();
			assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
			assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
			assert_eq!(fs::read_dir(&external).unwrap().count(), 1);
			assert_eq!(
				fs::read_to_string(external.join("sentinel")).unwrap(),
				"external sentinel"
			);
		}
	}
}

#[test]
fn partial_install_requires_force_and_preserves_external_hard_links() {
	let root = scratch("hard-linked-document");
	let destination = root.join("pina");
	let external = root.join("external.json");
	let reference = destination.join("references/migrations.md");
	fs::create_dir_all(reference.parent().unwrap()).unwrap();
	fs::write(&external, "external sentinel").unwrap();
	fs::hard_link(&external, &reference).unwrap();
	let target = destination.to_str().unwrap();

	let refused = pina(&["skill", "install", "--dir", target]);
	assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
	assert!(stderr(&refused).contains("--force"), "{}", stderr(&refused));
	assert_eq!(fs::read_to_string(&reference).unwrap(), "external sentinel");
	assert!(!destination.join("SKILL.md").exists());
	assert!(!destination.join("references/cli-and-codegen.md").exists());

	let forced = pina(&["skill", "install", "--dir", target, "--force"]);
	assert!(forced.status.success(), "{}", stderr(&forced));
	assert_eq!(fs::read_to_string(&external).unwrap(), "external sentinel");
	assert!(
		fs::read_to_string(&reference)
			.unwrap()
			.contains("abiVersion")
	);
}

#[test]
fn install_rejects_unexpected_output_types_before_replacing_documents() {
	let root = scratch("unexpected-output-types");

	for blocked in ["references", "references/migrations.md", "SKILL.md"] {
		let case = tempfile::tempdir_in(&root).unwrap();
		let destination = case.path().join("pina");
		fs::create_dir_all(destination.join("references")).unwrap();

		if blocked == "references" {
			fs::remove_dir(destination.join("references")).unwrap();
			fs::write(destination.join(blocked), "not a directory").unwrap();
		} else {
			fs::create_dir(destination.join(blocked)).unwrap();
		}

		let target = destination.to_str().unwrap();
		let output = pina(&["skill", "install", "--dir", target, "--force"]);
		assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
		assert!(!destination.join("references/cli-and-codegen.md").exists());
	}
}

#[test]
fn install_without_a_destination_prints_the_runtime_directories() {
	// The failure has to name the paths an operator can paste, and it must
	// list the runtimes found on this machine rather than a generic hint.
	// Windows resolves the home directory from USERPROFILE, so both names are
	// set to keep the assertion platform-independent.
	let home = scratch("home");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["skill", "install"])
		.env("HOME", &home)
		.env("USERPROFILE", &home)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina skill install: {error}"));

	assert_eq!(output.status.code(), Some(1));
	let reported = stderr(&output);
	assert!(reported.contains("--dir"), "{reported}");
	assert!(
		reported.contains("found runtime directory"),
		"the failure must list the runtimes it knows about: {reported}"
	);
	assert!(
		reported.contains(home.to_str().unwrap_or_default()),
		"the suggestions must be rooted at the home directory: {reported}"
	);
}

#[test]
fn install_without_a_home_still_reports_the_flag_guidance() {
	// The runtime suggestions come from the home directory. Without one the
	// failure must still name `--dir` rather than suggest a path rooted
	// nowhere.
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["skill", "install"])
		.env_remove("HOME")
		.env_remove("USERPROFILE")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina skill install: {error}"));

	assert_eq!(output.status.code(), Some(1));
	let reported = stderr(&output);
	assert!(reported.contains("--dir"), "{reported}");
	assert!(
		!reported.contains("found runtime directory"),
		"there is no runtime to suggest without a home directory: {reported}"
	);
}

#[test]
fn install_reports_a_destination_it_cannot_write() {
	// A regular file where the skill directory must go fails the write, and
	// the error must name the destination instead of panicking.
	let blocked = scratch("blocked").join("pina");
	fs::write(&blocked, "not a directory")
		.unwrap_or_else(|error| panic!("failed to create blocking file: {error}"));
	let target = blocked.to_str().unwrap_or_else(|| panic!("utf-8 path"));

	let output = pina(&["skill", "install", "--dir", target]);
	assert_eq!(output.status.code(), Some(1));
	let reported = stderr(&output);
	assert!(reported.contains("Failed to write into"), "{reported}");
	assert!(reported.contains("pina"), "{reported}");
}
