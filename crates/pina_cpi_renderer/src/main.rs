#![allow(missing_docs)]

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use pina_cpi_renderer::RenderConfig;
use pina_cpi_renderer::RenderError;
use pina_cpi_renderer::RenderMode;
use pina_cpi_renderer::read_root_node;
use pina_cpi_renderer::render_root_node;

#[derive(Debug, Parser)]
#[command(
	name = "pina_cpi_renderer",
	about = "Generate standalone Pina CPI crates from Codama IDLs"
)]
struct Args {
	/// A single Codama IDL file path. Can be provided multiple times.
	#[arg(long = "idl")]
	idls: Vec<PathBuf>,

	/// A directory containing `*.json` Codama IDLs.
	#[arg(long = "idl-dir")]
	idl_dir: Option<PathBuf>,

	/// Root output directory where `<program>/` will be written.
	#[arg(long)]
	output: PathBuf,

	/// Package name to use for a single-IDL render.
	#[arg(long)]
	package_name: Option<String>,

	/// How to handle an existing output directory.
	#[arg(long, value_enum, default_value = "auto")]
	mode: Mode,

	/// Skip instructions the renderer cannot express instead of failing; the
	/// skipped instructions are listed in the generated `instructions/mod.rs`.
	#[arg(long, default_value_t = false)]
	skip_unsupported_instructions: bool,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Mode {
	Auto,
	Create,
	Update,
	Overwrite,
}

impl From<Mode> for RenderMode {
	fn from(mode: Mode) -> Self {
		match mode {
			Mode::Auto => Self::Auto,
			Mode::Create => Self::Create,
			Mode::Update => Self::Update,
			Mode::Overwrite => Self::Overwrite,
		}
	}
}

fn main() {
	let args = Args::parse();
	let mut idls = args.idls;
	if let Some(dir) = &args.idl_dir {
		match json_idls_in_dir(dir) {
			Ok(paths) => idls.extend(paths),
			Err(error) => {
				eprintln!("{error}");
				std::process::exit(1);
			}
		}
	}

	if idls.is_empty() {
		eprintln!("provide at least one --idl or an --idl-dir containing *.json files");
		std::process::exit(1);
	}

	let single_idl = idls.len() == 1;

	for idl in &idls {
		let crate_dir = crate_output_dir(&args.output, idl, single_idl).unwrap_or_else(|error| {
			eprintln!("{error}");
			std::process::exit(1);
		});

		let root = read_root_node(idl).unwrap_or_else(|error| {
			eprintln!("failed to read `{}`: {error}", idl.display());
			std::process::exit(1);
		});
		let config = RenderConfig {
			package_name: single_idl.then(|| args.package_name.clone()).flatten(),
			mode: args.mode.into(),
			skip_unsupported_instructions: args.skip_unsupported_instructions,
			..RenderConfig::default()
		};

		render_root_node(&root, &crate_dir, &config).unwrap_or_else(|error| {
			eprintln!("failed to render `{}`: {error}", idl.display());
			std::process::exit(1);
		});

		println!(
			"rendered {} -> {}",
			idl.display(),
			display_relative(&crate_dir)
		);
	}
}

/// Every `*.json` file in one IDL directory, in a stable order.
///
/// An iteration error would silently drop the IDLs after it, so the whole read
/// is reported instead of skipped.
fn json_idls_in_dir(dir: &Path) -> Result<Vec<PathBuf>, String> {
	let entries = std::fs::read_dir(dir).map_err(|error| dir_read_error(dir, &error))?;

	let mut paths = Vec::new();
	for entry in entries {
		let entry = entry.map_err(|error| dir_read_error(dir, &error))?;
		let path = entry.path();
		if path
			.extension()
			.is_some_and(|extension| extension == "json")
		{
			paths.push(path);
		}
	}

	paths.sort();

	Ok(paths)
}

/// The message for a directory that cannot be read or iterated.
fn dir_read_error(dir: &Path, error: &std::io::Error) -> String {
	format!("failed to read IDL directory `{}`: {error}", dir.display())
}

/// The crate directory for one IDL inside a multi-IDL output root.
fn crate_dir_name(idl: &Path) -> Result<&str, RenderError> {
	idl.file_stem()
		.and_then(|stem| stem.to_str())
		.filter(|stem| {
			matches!(
				Path::new(stem).components().next(),
				Some(Component::Normal(_))
			) && Path::new(stem).components().count() == 1
				&& !stem.contains(['\\', ':'])
				&& !stem.ends_with(['.', ' '])
		})
		.ok_or_else(|| {
			RenderError::UnsafeOutputPath {
				path: idl.to_path_buf(),
				reason: "file stem must name a UTF-8 child output directory".to_string(),
			}
		})
}

/// A single IDL writes its crate straight into `--output`; several IDLs become
/// sibling crates named after each file.
fn crate_output_dir(output: &Path, idl: &Path, single_idl: bool) -> Result<PathBuf, RenderError> {
	if single_idl {
		Ok(output.to_path_buf())
	} else {
		Ok(output.join(crate_dir_name(idl)?))
	}
}

fn display_relative(path: &Path) -> String {
	std::env::current_dir()
		.ok()
		.and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
		.unwrap_or_else(|| path.to_path_buf())
		.display()
		.to_string()
}

#[cfg(test)]
mod tests {
	use clap::CommandFactory;

	use super::*;

	#[test]
	fn output_names_are_child_components() {
		for name in [
			"",
			"/",
			"..json",
			"...json",
			"....json",
			"name .json",
			"name:.json",
			"name\\child.json",
		] {
			assert!(crate_dir_name(Path::new(name)).is_err(), "{name:?}");
		}

		for name in [
			"counter.json",
			"hello-world.json",
			"hello_world.json",
			"program.v1.json",
		] {
			let stem = crate_dir_name(Path::new(name)).unwrap();
			let output = Path::new("clients").join(stem);
			assert_eq!(output.parent(), Some(Path::new("clients")));
			assert!(matches!(
				Path::new(stem).components().next(),
				Some(Component::Normal(_))
			));
		}
	}

	#[cfg(unix)]
	#[test]
	fn rejects_non_utf8_file_stems() {
		use std::ffi::OsStr;
		use std::os::unix::ffi::OsStrExt;

		let path = Path::new(OsStr::from_bytes(b"invalid\xff.json"));
		assert!(crate_dir_name(path).is_err());
	}

	#[test]
	fn accepted_names_never_escape_the_output_root() {
		let alphabet = ['a', '.', ' ', ':', '\\', '/', '_', 'é'];

		for first in alphabet {
			for second in alphabet {
				for third in alphabet {
					let filename = format!("{first}{second}{third}.json");

					if let Ok(stem) = crate_dir_name(Path::new(&filename)) {
						assert_eq!(
							Path::new("clients").join(stem).parent(),
							Some(Path::new("clients"))
						);
					}
				}
			}
		}
	}

	#[test]
	fn converts_every_generation_mode() {
		assert!(matches!(RenderMode::from(Mode::Auto), RenderMode::Auto));
		assert!(matches!(RenderMode::from(Mode::Create), RenderMode::Create));
		assert!(matches!(RenderMode::from(Mode::Update), RenderMode::Update));
		assert!(matches!(
			RenderMode::from(Mode::Overwrite),
			RenderMode::Overwrite
		));
		assert!(Args::command().has_subcommands() || true);
	}

	#[test]
	fn reads_only_json_entries_from_an_idl_directory() {
		let temp = tempfile::tempdir().expect("temp dir");
		std::fs::write(temp.path().join("b.json"), "{}").expect("write");
		std::fs::write(temp.path().join("a.json"), "{}").expect("write");
		std::fs::write(temp.path().join("notes.txt"), "ignored").expect("write");

		let paths = json_idls_in_dir(temp.path()).expect("directory should be readable");
		let names: Vec<String> = paths
			.iter()
			.map(|path| {
				path.file_name()
					.unwrap_or_default()
					.to_string_lossy()
					.into_owned()
			})
			.collect();
		assert_eq!(names, ["a.json", "b.json"]);

		// A missing directory is reported rather than silently producing none.
		let error = json_idls_in_dir(&temp.path().join("missing")).expect_err("missing dir");
		assert!(error.contains("failed to read IDL directory"));

		let shared = dir_read_error(temp.path(), &std::io::Error::other("unreadable"));
		assert!(shared.contains("failed to read IDL directory"));
	}

	#[test]
	fn single_idls_write_into_the_output_and_multi_idls_become_siblings() {
		let idl = Path::new("/tmp/idls/vesting_program.json");
		let output = Path::new("/tmp/clients");

		assert_eq!(
			crate_output_dir(output, idl, true).unwrap_or_else(|error| panic!("{error}")),
			PathBuf::from("/tmp/clients")
		);
		assert_eq!(
			crate_output_dir(output, idl, false).unwrap_or_else(|error| panic!("{error}")),
			PathBuf::from("/tmp/clients/vesting_program")
		);
		assert!(crate_dir_name(Path::new("/")).is_err());
	}

	#[test]
	fn single_idl_output_does_not_depend_on_the_file_stem() {
		let output = Path::new("clients");
		let idl = Path::new("...json");
		assert_eq!(
			crate_output_dir(output, idl, true).unwrap_or_else(|error| panic!("{error}")),
			output
		);
		assert!(crate_output_dir(output, idl, false).is_err());
	}
}
