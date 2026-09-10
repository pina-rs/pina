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

	for idl_path in &idl_paths {
		let root = read_root_node(idl_path)?;
		let stem = idl_path
			.file_stem()
			.and_then(|stem| stem.to_str())
			.ok_or_else(|| {
				RenderError::ReadFile {
					path: idl_path.clone(),
					source: std::io::Error::other("invalid file name"),
				}
			})?
			.to_owned();
		let client_path = match &args.client_path {
			Some(path) => join_client_path(path, &stem),
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
		render_root_node(&root, &args.output.join(&stem), &config)?;
	}

	Ok(())
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
