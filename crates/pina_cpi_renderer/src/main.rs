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
	let single_idl = args.idls.len() == 1;

	let mut idls = args.idls;
	if let Some(dir) = &args.idl_dir {
		let entries = std::fs::read_dir(dir).unwrap_or_else(|error| {
			eprintln!("failed to read IDL directory `{}`: {error}", dir.display());
			std::process::exit(1);
		});
		for entry in entries {
			let path = entry
				.unwrap_or_else(|error| {
					eprintln!("failed to read IDL entry: {error}");
					std::process::exit(1);
				})
				.path();
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

	for idl in &idls {
		let crate_dir = if idls.len() == 1 && single_idl {
			args.output.clone()
		} else {
			let name = idl
				.file_stem()
				.map(|stem| stem.to_string_lossy().into_owned())
				.unwrap_or_else(|| "program".to_string());
			args.output.join(name)
		};

		let root = read_root_node(idl).unwrap_or_else(|error| {
			eprintln!("failed to read `{}`: {error}", idl.display());
			std::process::exit(1);
		});
		let config = RenderConfig {
			package_name: if single_idl {
				args.package_name.clone()
			} else {
				None
			},
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

fn display_relative(path: &Path) -> String {
	std::env::current_dir()
		.ok()
		.and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
		.unwrap_or_else(|| path.to_path_buf())
		.display()
		.to_string()
}
