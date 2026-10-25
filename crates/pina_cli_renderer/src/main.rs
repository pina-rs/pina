#![allow(missing_docs)]

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use pina_cli_renderer::RenderConfig;
use pina_cli_renderer::RenderError;
use pina_cli_renderer::RenderMode;
use pina_cli_renderer::read_root_node;
use pina_cli_renderer::render_root_node;

#[derive(Debug, Parser)]
#[command(
	name = "pina_cli_renderer",
	about = "Generate clap-based CLI crates from Codama IDLs"
)]
struct Args {
	/// Codama IDL file to render.
	#[arg(long = "idl", value_name = "FILE")]
	idls: Vec<PathBuf>,

	/// Directory containing `*.json` IDLs.
	#[arg(long = "idl-dir", value_name = "DIR")]
	idl_dir: Option<PathBuf>,

	/// Root output directory where each CLI crate is written.
	#[arg(long, value_name = "DIR")]
	output: PathBuf,

	/// Package name of the generated Rust client crate each CLI depends on.
	#[arg(long = "client-package", value_name = "NAME")]
	client_package: Option<String>,

	/// Path prefix from each CLI crate to its generated Rust client crate.
	#[arg(long = "client-path", value_name = "PATH")]
	client_path: Option<String>,

	/// How to handle existing CLI crate destinations.
	#[arg(long, value_enum, default_value = "auto", value_name = "MODE")]
	mode: RenderMode,

	/// Generate only sources without Cargo.toml or README.
	#[arg(long)]
	no_scaffold: bool,
}

fn main() {
	if let Err(error) = run() {
		eprintln!("{error}");
		std::process::exit(1);
	}
}

fn run() -> Result<(), RenderError> {
	let args = Args::parse();
	let idl_paths = collect_idl_paths(&args)?;
	let outputs = idl_paths
		.iter()
		.map(|idl| crate_dir_name(idl).map(|stem| (idl, stem)))
		.collect::<Result<Vec<_>, _>>()?;

	for (idl_path, stem) in outputs {
		let root = read_root_node(idl_path)?;
		let client_path = match &args.client_path {
			Some(path) => join_client_path(path, stem),
			None => format!("../../rust/{stem}"),
		};
		let client_package = args
			.client_package
			.clone()
			.unwrap_or_else(|| format!("{}-client", stem.replace('_', "-")));
		let config = RenderConfig {
			mode: args.mode,
			scaffold: !args.no_scaffold,
			client_package,
			client_path,
		};
		render_root_node(&root, &args.output.join(stem), &config)?;
	}

	Ok(())
}

/// Reject path syntax and dot aliases before using an IDL stem as a child name.
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

/// Treat `--client-path` as a prefix and append the program stem.
fn join_client_path(prefix: &str, stem: &str) -> String {
	let trimmed = prefix.trim_end_matches('/');
	format!("{trimmed}/{stem}")
}

fn collect_idl_paths(args: &Args) -> Result<Vec<PathBuf>, RenderError> {
	let mut idl_paths = args.idls.clone();

	if let Some(idl_dir) = &args.idl_dir {
		let entries = std::fs::read_dir(idl_dir).map_err(|source| {
			RenderError::ReadFile {
				path: idl_dir.clone(),
				source,
			}
		})?;
		for entry in entries {
			let entry = entry.map_err(|source| {
				RenderError::ReadFile {
					path: idl_dir.clone(),
					source,
				}
			})?;
			let path = entry.path();
			if path
				.extension()
				.is_some_and(|extension| extension == "json")
			{
				idl_paths.push(path);
			}
		}
	}

	idl_paths.sort();
	Ok(idl_paths)
}

#[cfg(test)]
mod tests {
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
			#[cfg(not(windows))]
			"name\\child.json",
		] {
			assert!(crate_dir_name(Path::new(name)).is_err(), "{name:?}");
		}

		for name in [
			"counter.json",
			"hello-world.json",
			"hello_world.json",
			"program.v1.json",
			#[cfg(windows)]
			r"C:\idls\counter.json",
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
}
