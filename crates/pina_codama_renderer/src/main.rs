use std::path::PathBuf;

use clap::Parser;
use pina_codama_renderer::RenderConfig;
use pina_codama_renderer::RenderError;
use pina_codama_renderer::read_root_node;
use pina_codama_renderer::render_root_node;

#[derive(Debug, Parser)]
#[command(
	name = "pina_codama_renderer",
	about = "Generate Pina-style Rust clients from Codama IDLs"
)]
struct Args {
	/// A single IDL file path. Can be provided multiple times.
	#[arg(long = "idl")]
	idls: Vec<PathBuf>,

	/// A directory containing `*.json` IDLs.
	#[arg(long = "idl-dir")]
	idl_dir: Option<PathBuf>,

	/// Root output directory where `<program>/src/generated` will be written.
	#[arg(long)]
	output: PathBuf,
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
	let config = RenderConfig::default();

	for idl_path in &idl_paths {
		let root = read_root_node(idl_path)?;
		let output_crate_dir = args.output.join(file_stem(idl_path)?);
		render_root_node(&root, &output_crate_dir, &config)?;
	}

	Ok(())
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
			let is_json = path
				.extension()
				.and_then(|ext| ext.to_str())
				.is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
			if is_json {
				idl_paths.push(path);
			}
		}
	}

	idl_paths.sort();
	idl_paths.dedup();

	if idl_paths.is_empty() {
		return Err(RenderError::UnsupportedValue {
			context: "cli arguments".to_string(),
			kind: "args",
			reason: "provide at least one --idl or --idl-dir".to_string(),
		});
	}

	Ok(idl_paths)
}

fn file_stem(path: &PathBuf) -> Result<String, RenderError> {
	let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
		return Err(RenderError::UnsupportedValue {
			context: format!("path `{}`", path.display()),
			kind: "path",
			reason: "expected a UTF-8 file stem".to_string(),
		});
	};
	Ok(stem.to_string())
}
