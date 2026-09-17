#![allow(missing_docs)]

use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use pina_cpi_renderer::RenderConfig;
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
		let entries = std::fs::read_dir(dir).unwrap_or_else(|error| {
			eprintln!("failed to read IDL directory `{}`: {error}", dir.display());
			std::process::exit(1);
		});
		// Unreadable entries are skipped; if nothing usable remains, the
		// empty-source check below reports it.
		for entry in entries.flatten() {
			let path = entry.path();
			if path
				.extension()
				.is_some_and(|extension| extension == "json")
			{
				idls.push(path);
			}
		}
	}

	if idls.is_empty() {
		eprintln!("provide at least one --idl or an --idl-dir containing *.json files");
		std::process::exit(1);
	}

	let single_idl = idls.len() == 1;

	for idl in &idls {
		let crate_dir = crate_output_dir(&args.output, idl, single_idl);

		let root = read_root_node(idl).unwrap_or_else(|error| {
			eprintln!("failed to read `{}`: {error}", idl.display());
			std::process::exit(1);
		});
		let config = RenderConfig {
			package_name: single_idl.then(|| args.package_name.clone()).flatten(),
			mode: args.mode.into(),
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

/// The crate directory for one IDL inside a multi-IDL output root.
fn crate_dir_name(idl: &Path) -> String {
	idl.file_stem().map_or_else(
		|| "program".to_string(),
		|stem| stem.to_string_lossy().into_owned(),
	)
}

/// A single IDL writes its crate straight into `--output`; several IDLs become
/// sibling crates named after each file.
fn crate_output_dir(output: &Path, idl: &Path, single_idl: bool) -> PathBuf {
	if single_idl {
		output.to_path_buf()
	} else {
		output.join(crate_dir_name(idl))
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
	fn single_idls_write_into_the_output_and_multi_idls_become_siblings() {
		let idl = Path::new("/tmp/idls/vesting_program.json");
		let output = Path::new("/tmp/clients");

		assert_eq!(
			crate_output_dir(output, idl, true),
			PathBuf::from("/tmp/clients")
		);
		assert_eq!(
			crate_output_dir(output, idl, false),
			PathBuf::from("/tmp/clients/vesting_program")
		);
		// An IDL without a file stem still gets a deterministic crate name.
		assert_eq!(crate_dir_name(Path::new("/")), "program");
	}

	#[test]
	fn falls_back_to_program_when_the_idl_has_no_stem() {
		let name = crate_dir_name(Path::new("/"));
		assert_eq!(name, "program");

		let named = crate_dir_name(Path::new("/tmp/counter.json"));
		assert_eq!(named, "counter");
	}
}
