use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;

#[derive(Parser, Debug)]
#[command(name = "pina", version, about = "CLI tool for Pina Solana programs")]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
	/// Generate Codama IDL JSON from a Pina program crate.
	Idl {
		/// Program crate directory (must contain Cargo.toml and src/lib.rs).
		#[arg(short, long, default_value = ".")]
		path: PathBuf,

		/// Output file. Writes to stdout when omitted.
		#[arg(short, long)]
		output: Option<PathBuf>,

		/// Override the program name (defaults to the package name from
		/// Cargo.toml).
		#[arg(short, long)]
		name: Option<String>,

		/// Pretty-print the JSON output.
		#[arg(long, default_value_t = true)]
		pretty: bool,
	},
	/// Initialize a new Pina program project.
	Init {
		/// Package name for the new project (for example: `my_program`).
		name: String,

		/// Target directory for the generated project.
		///
		/// Defaults to `./<name>`.
		#[arg(short, long)]
		path: Option<PathBuf>,

		/// Overwrite scaffold files when they already exist.
		#[arg(long, default_value_t = false)]
		force: bool,
	},
}

fn main() {
	let cli = Cli::parse();

	match cli.command {
		Commands::Idl {
			path,
			output,
			name,
			pretty,
		} => run_idl(path.as_path(), output.as_deref(), name.as_deref(), pretty),
		Commands::Init { name, path, force } => run_init(name.as_str(), path.as_deref(), force),
	}
}

fn run_idl(
	path: &std::path::Path,
	output: Option<&std::path::Path>,
	name: Option<&str>,
	pretty: bool,
) {
	let root = match pina_cli::generate_idl(path, name) {
		Ok(r) => r,
		Err(e) => {
			eprintln!("Error: {e}");
			std::process::exit(1);
		}
	};

	let json = if pretty {
		serde_json::to_string_pretty(&root)
	} else {
		serde_json::to_string(&root)
	};

	let json = match json {
		Ok(j) => j,
		Err(e) => {
			eprintln!("JSON serialization error: {e}");
			std::process::exit(1);
		}
	};

	if let Some(output) = output {
		if let Err(e) = std::fs::write(output, &json) {
			eprintln!("Failed to write {}: {e}", output.display());
			std::process::exit(1);
		}
	} else {
		println!("{json}");
	}
}

fn run_init(name: &str, path: Option<&std::path::Path>, force: bool) {
	let project_path = path.map_or_else(|| PathBuf::from(name), PathBuf::from);

	if let Err(err) = pina_cli::init_project(&project_path, name, force) {
		eprintln!("Error: {err}");
		std::process::exit(1);
	}

	println!("Initialized new Pina project at {}", project_path.display());
}
