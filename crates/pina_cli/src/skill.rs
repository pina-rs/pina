//! Serve the agent skill from the CLI.
//!
//! The skill ships in the binary so an agent that has only the toolkit — no
//! npm package, no network, no cloned repository — can still read the same
//! guidance a skill installation would give it. `read` prints raw Markdown on
//! stdout: agents parse the file itself, so there is deliberately no terminal
//! rendering on this path (`pina docs` owns the human-facing rendering).
//!
//! The embedded bytes are a committed copy of `packages/pina__skill` kept in
//! step by `scripts/docs/sync-skill.mjs`; `verify:docs` fails when the copy
//! drifts.

use std::path::Path;
use std::path::PathBuf;

use owo_colors::OwoColorize;

/// The entrypoint document agents load first.
const SKILL_ENTRY: &str = include_str!("../skill/SKILL.md");

/// A skill document resolved by topic name.
struct Topic {
	/// Name accepted on the command line.
	name: &'static str,
	/// Path the document carries in an installation, slash-separated.
	file: &'static str,
	/// Description shown by the listing.
	description: &'static str,
	content: &'static str,
}

/// Every topic the CLI can serve, entrypoint first.
const TOPICS: &[Topic] = &[
	Topic {
		name: "pina",
		file: "SKILL.md",
		description: "entrypoint: routing, invariants, and where each task is covered",
		content: SKILL_ENTRY,
	},
	Topic {
		name: "cli-and-codegen",
		file: "references/cli-and-codegen.md",
		description: "CLI discovery, diagnostics, keys, IDL and client generation, profiling",
		content: include_str!("../skill/references/cli-and-codegen.md"),
	},
	Topic {
		name: "migrations",
		file: "references/migrations.md",
		description: "version envelopes, the create/check loop, transitions, budget failures",
		content: include_str!("../skill/references/migrations.md"),
	},
	Topic {
		name: "program-authoring",
		file: "references/program-authoring.md",
		description: "accounts, instructions, validation, PDAs, CPI, resize, and close",
		content: include_str!("../skill/references/program-authoring.md"),
	},
	Topic {
		name: "project-setup",
		file: "references/project-setup.md",
		description: "project creation, features, and workspace layout",
		content: include_str!("../skill/references/project-setup.md"),
	},
	Topic {
		name: "testing",
		file: "references/testing.md",
		description: "unit, Mollusk, SBF, and generated-artifact checks",
		content: include_str!("../skill/references/testing.md"),
	},
];

/// What a skill operation did, for the caller to report.
#[derive(Debug)]
pub enum SkillOutcome {
	/// The listing was printed to stdout.
	Listed,
	/// One document was printed verbatim to stdout.
	Read {
		/// The topic that was served.
		topic: &'static str,
	},
	/// The skill was written to a destination.
	Installed {
		/// Directory the tree was written under.
		destination: PathBuf,
		/// Number of documents written.
		files: usize,
	},
}

/// Why a skill operation failed.
#[derive(Debug)]
pub enum SkillError {
	/// `read` was given a topic the CLI does not bundle.
	UnknownTopic {
		/// The name that was requested.
		requested: String,
		/// Names that would have been accepted.
		available: Vec<&'static str>,
	},
	/// `install` was given no destination.
	MissingDestination,
	/// The destination already holds a skill and `--force` was not passed.
	DestinationExists {
		/// The directory that already holds a `SKILL.md`.
		destination: PathBuf,
	},
	/// Writing the tree failed.
	Write {
		/// The directory that was being written.
		destination: PathBuf,
		/// The underlying I/O error.
		source: std::io::Error,
	},
}

impl SkillError {
	/// The human-readable message, printed on stderr.
	pub fn message(&self) -> String {
		match self {
			Self::UnknownTopic {
				requested,
				available,
			} => {
				let mut message = format!("Topic `{requested}` is not bundled. Bundled topics:");
				for name in available {
					message.push('\n');
					message.push_str("  ");
					message.push_str(name);
				}
				message
			}
			Self::MissingDestination => {
				[
					"No destination. Pass --dir <directory>, for example:",
					"  pina skill install --dir ~/.claude/skills/pina",
					"  pina skill install --dir ./.claude/skills/pina",
				]
				.join("\n")
			}
			Self::DestinationExists { destination } => {
				format!(
					"{} already contains a skill. Pass --force to replace it.",
					destination.display()
				)
			}
			Self::Write {
				destination,
				source,
			} => format!("Failed to write into {}: {source}", destination.display()),
		}
	}
}

/// Run a `pina skill` subcommand.
///
/// Everything returns a result so the failure paths are testable; the caller
/// owns printing and exit codes.
pub fn run(action: SkillAction) -> Result<SkillOutcome, SkillError> {
	match action {
		SkillAction::List => {
			list();
			Ok(SkillOutcome::Listed)
		}
		SkillAction::Read { topic } => read(topic.as_deref()),
		SkillAction::Install { destination, force } => install(destination, force),
	}
}

/// A `pina skill` subcommand.
pub enum SkillAction {
	/// Describe the bundled topics.
	List,
	/// Print one document verbatim.
	Read {
		/// Topic name, or `None` for the listing.
		topic: Option<String>,
	},
	/// Write the whole skill into a directory.
	Install {
		/// Target skill directory, or `None` to fail with usage help.
		destination: Option<PathBuf>,
		/// Replace an existing skill when one is present.
		force: bool,
	},
}

fn list() {
	println!("The Pina agent skill is bundled in this CLI. Read a topic:");
	for topic in TOPICS {
		println!("  {:<16} {}", topic.name, topic.description);
	}
	println!();
	println!("  pina skill read <topic>               # raw Markdown on stdout");
	println!("  pina skill install --dir <directory>  # install for an agent runtime");
}

fn read(topic: Option<&str>) -> Result<SkillOutcome, SkillError> {
	let Some(topic) = topic else {
		list();
		return Ok(SkillOutcome::Listed);
	};

	let Some(matched) = TOPICS.iter().find(|candidate| candidate.name == topic) else {
		return Err(SkillError::UnknownTopic {
			requested: topic.to_owned(),
			available: TOPICS.iter().map(|candidate| candidate.name).collect(),
		});
	};

	print!("{}", matched.content);
	Ok(SkillOutcome::Read {
		topic: matched.name,
	})
}

fn install(destination: Option<PathBuf>, force: bool) -> Result<SkillOutcome, SkillError> {
	let Some(destination) = destination else {
		return Err(SkillError::MissingDestination);
	};
	if destination.join("SKILL.md").exists() && !force {
		return Err(SkillError::DestinationExists { destination });
	}
	let files = write_tree(&destination).map_err(|source| {
		SkillError::Write {
			destination: destination.clone(),
			source,
		}
	})?;
	Ok(SkillOutcome::Installed { destination, files })
}

/// Write every bundled document under `directory`, returning the file count.
fn write_tree(directory: &Path) -> std::io::Result<usize> {
	// The install paths come from the static table, never from input, so these
	// joins cannot escape the destination.
	std::fs::create_dir_all(directory.join("references"))?;
	for topic in TOPICS {
		std::fs::write(directory.join(topic.file), topic.content)?;
	}
	Ok(TOPICS.len())
}

/// The agent runtimes this machine is known to use, in the order to print them.
const RUNTIME_SKILL_DIRS: &[&str] = &[".claude", ".codex", ".agents"];

/// Skill directories under `home` for the runtimes in the table above.
///
/// Pure so both the present and absent-home cases are testable on any platform.
fn skill_dirs_under(home: &Path) -> Vec<PathBuf> {
	RUNTIME_SKILL_DIRS
		.iter()
		.map(|runtime| home.join(runtime).join("skills").join("pina"))
		.collect()
}

/// Skill directories of the agent runtimes this machine is known to use.
///
/// `pina skill install` prints these when it is called without `--dir`, so the
/// caller can copy the right path instead of memorizing each runtime's layout.
/// Windows names the home directory `USERPROFILE` rather than `HOME`.
pub fn suggest_destinations() -> Vec<PathBuf> {
	let home = std::env::var_os("HOME")
		.or_else(|| std::env::var_os("USERPROFILE"))
		.filter(|value| !value.is_empty())
		.map(PathBuf::from);
	match home {
		Some(home) => skill_dirs_under(&home),
		None => Vec::new(),
	}
}

/// Total number of bundled documents.
pub fn bundled_count() -> usize {
	TOPICS.len()
}

/// Report an outcome or exit with an error, the CLI's only exit path.
pub fn report(outcome: Result<SkillOutcome, SkillError>) {
	match outcome {
		// The listing and the document already went to stdout.
		Ok(SkillOutcome::Listed | SkillOutcome::Read { .. }) => {}
		Ok(SkillOutcome::Installed { destination, files }) => {
			println!(
				"{} Installed {files} file{} into {}",
				"✔".green(),
				if files == 1 { "" } else { "s" },
				destination.display()
			);
			println!("Point the agent runtime at that directory as a skill named `pina`.");
		}
		Err(error) => {
			eprintln!("{} {}", "Error".red().bold(), error.message());
			if matches!(error, SkillError::MissingDestination) {
				for suggestion in suggest_destinations() {
					eprintln!("  found runtime directory: {}", suggestion.display());
				}
			}
			std::process::exit(1);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_topic_is_bundled_with_substantive_content() {
		assert_eq!(TOPICS.len(), 6, "entrypoint plus five references");
		assert_eq!(bundled_count(), TOPICS.len());
		for topic in TOPICS {
			if topic.name == "pina" {
				assert_eq!(
					topic.file, "SKILL.md",
					"the entrypoint installs as SKILL.md"
				);
			} else {
				let stem = Path::new(topic.file)
					.file_stem()
					.and_then(|stem| stem.to_str());
				assert_eq!(
					Some(topic.name),
					stem,
					"topic name must match its file stem"
				);
			}
			assert!(
				topic.content.len() > 1000,
				"`{}` looks truncated",
				topic.file
			);
		}
	}

	#[test]
	fn entrypoint_is_the_bundled_skill_md() {
		// Git checks text out with CRLF on Windows, so assert on the logical
		// lines rather than the byte sequence an LF checkout happens to have.
		let mut lines = SKILL_ENTRY.lines();
		assert_eq!(lines.next(), Some("---"));
		assert_eq!(lines.next(), Some("name: pina"));
		assert!(SKILL_ENTRY.contains("Task routing"));
	}

	#[test]
	fn read_serves_every_bundled_topic() {
		// The document itself goes to stdout, so the spawned-binary suite in
		// `tests/skill_command.rs` grades the bytes; here the contract is the
		// outcome, and it must name the topic it served.
		for topic in TOPICS {
			let action = SkillAction::Read {
				topic: Some(topic.name.to_owned()),
			};
			let served = run(action);
			assert!(
				matches!(served, Ok(SkillOutcome::Read { .. })),
				"`{}` must read: {served:?}",
				topic.name
			);
		}
	}

	#[test]
	fn read_without_a_topic_lists_every_topic() {
		let outcome = run(SkillAction::Read { topic: None });
		assert!(
			matches!(outcome, Ok(SkillOutcome::Listed)),
			"a bare read lists: {outcome:?}"
		);
	}

	#[test]
	fn report_prints_one_outcome_per_success_variant() {
		// `Err` exits the process, so the error path is covered by the spawned
		// CLI tests instead. Every success variant prints and returns.
		report(Ok(SkillOutcome::Listed));
		report(Ok(SkillOutcome::Read { topic: "pina" }));
		report(Ok(SkillOutcome::Installed {
			destination: PathBuf::from("/tmp/skill"),
			files: 1,
		}));
		report(Ok(SkillOutcome::Installed {
			destination: PathBuf::from("/tmp/skill"),
			files: 6,
		}));
	}

	#[test]
	fn read_rejects_an_unknown_topic_with_the_available_names() {
		let action = SkillAction::Read {
			topic: Some("anchor".to_owned()),
		};
		let error = run(action).err().unwrap_or(SkillError::MissingDestination);
		assert!(
			matches!(error, SkillError::UnknownTopic { .. }),
			"got {error:?}"
		);
		let message = error.message();
		assert!(message.contains("`anchor` is not bundled"), "{message}");
		assert_eq!(
			message
				.lines()
				.filter(|line| line.starts_with("  "))
				.count(),
			6
		);
	}

	#[test]
	fn install_writes_every_document_into_the_target_tree() {
		let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let destination = root.path().join("skills").join("pina");
		let action = SkillAction::Install {
			destination: Some(destination.clone()),
			force: false,
		};

		let outcome = run(action);
		assert!(
			matches!(outcome, Ok(SkillOutcome::Installed { files: 6, .. })),
			"got {outcome:?}"
		);
		for topic in TOPICS {
			let path = destination.join(topic.file);
			let written = std::fs::read_to_string(&path);
			assert_eq!(
				written.ok().as_deref(),
				Some(topic.content),
				"{}",
				topic.file
			);
		}
	}

	#[test]
	fn install_covers_the_destination_that_already_holds_a_skill() {
		let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let destination = root.path().join("pina");
		let fresh = SkillAction::Install {
			destination: Some(destination.clone()),
			force: false,
		};

		let refused = run(fresh);
		assert!(
			matches!(refused, Ok(SkillOutcome::Installed { .. })),
			"got {refused:?}"
		);

		let again = SkillAction::Install {
			destination: Some(destination.clone()),
			force: false,
		};
		let error = run(again).err().unwrap_or(SkillError::MissingDestination);
		assert!(
			matches!(error, SkillError::DestinationExists { .. }),
			"got {error:?}"
		);
		assert!(error.message().contains("--force"), "{}", error.message());
	}

	#[test]
	fn install_refuses_to_replace_a_curated_skill_without_force() {
		let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let destination = root.path().join("pina");
		std::fs::create_dir_all(&destination).unwrap_or_else(|error| panic!("create: {error}"));
		let seeded = destination.join("SKILL.md");
		std::fs::write(&seeded, "curated by the operator").unwrap_or_else(|e| panic!("seed: {e}"));

		let blocked = SkillAction::Install {
			destination: Some(destination.clone()),
			force: false,
		};
		let error = run(blocked).err().unwrap_or(SkillError::MissingDestination);
		assert!(
			matches!(error, SkillError::DestinationExists { .. }),
			"got {error:?}"
		);
		let surviving = std::fs::read_to_string(&seeded);
		assert_eq!(surviving.ok().as_deref(), Some("curated by the operator"));

		let forced = SkillAction::Install {
			destination: Some(destination.clone()),
			force: true,
		};
		assert!(matches!(run(forced), Ok(SkillOutcome::Installed { .. })));
		let overwritten = std::fs::read_to_string(&seeded);
		let first_line = overwritten
			.unwrap_or_default()
			.lines()
			.next()
			.map(str::to_owned);
		assert_eq!(first_line.as_deref(), Some("---"));
	}

	#[test]
	fn install_reports_a_write_failure_with_the_destination() {
		let root = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		// A regular file where the skill directory must go fails the first
		// `create_dir_all`, so the error path carries the destination.
		let destination = root.path().join("blocked");
		std::fs::write(&destination, "not a directory").unwrap_or_else(|e| panic!("block: {e}"));

		let action = SkillAction::Install {
			destination: Some(destination.clone()),
			force: false,
		};
		let error = run(action).err().unwrap_or(SkillError::MissingDestination);
		assert!(matches!(error, SkillError::Write { .. }), "got {error:?}");
		assert!(
			error.message().contains("Failed to write into"),
			"{}",
			error.message()
		);
	}

	#[test]
	fn install_without_a_destination_reports_usable_guidance() {
		let action = SkillAction::Install {
			destination: None,
			force: false,
		};
		let error = run(action).err().unwrap_or(SkillError::Write {
			destination: PathBuf::new(),
			source: std::io::Error::other("unreachable"),
		});
		assert!(matches!(error, SkillError::MissingDestination));
		let message = error.message();
		assert!(message.contains("--dir"));
		assert!(message.contains("pina skill install"));
	}

	#[test]
	fn skill_dirs_are_named_pina_under_known_runtimes() {
		let home = Path::new("/home/operator");
		let suggested = skill_dirs_under(home);
		assert_eq!(suggested.len(), 3, "one directory per known runtime");
		for path in &suggested {
			assert_eq!(
				path.file_name().and_then(|name| name.to_str()),
				Some("pina")
			);
			assert!(path.starts_with(home), "{}", path.display());
		}
	}

	#[test]
	fn suggest_destinations_answers_for_this_platform() {
		// Windows exports USERPROFILE where Unix exports HOME, so the runner
		// decides which branch is live here rather than the test.
		let has_home = ["HOME", "USERPROFILE"]
			.iter()
			.any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()));
		let suggested = suggest_destinations();
		let expected = if has_home { 3 } else { 0 };
		assert_eq!(
			suggested.len(),
			expected,
			"one destination per known runtime when a home exists, none otherwise"
		);
		for path in &suggested {
			assert_eq!(
				path.file_name().and_then(|name| name.to_str()),
				Some("pina")
			);
			assert!(
				path.ends_with(Path::new("skills").join("pina")),
				"{} must sit under a skills directory",
				path.display()
			);
		}
	}
}
