use std::fs;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use comfy_table::Table;
use owo_colors::OwoColorize;

#[derive(Parser, Debug)]
#[command(
	name = "pina",
	version,
	about = "CLI tool for Pina Solana programs",
	after_help = "🤖 Agent Note: To understand the IDL extraction rules or architecture, run \
	              `pina docs <topic>`. Topics are derived from the project's MDT templates."
)]
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
	/// Show a documentation topic from the project templates.
	Docs {
		/// The documentation topic (e.g., `pina-idl`, `pina-overview`).
		topic: String,
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
	/// Static CU profiler for compiled SBF programs.
	Profile {
		/// Path to the compiled SBF `.so` file.
		path: PathBuf,

		/// Output as JSON instead of text.
		#[arg(long, default_value_t = false)]
		json: bool,

		/// Write output to a file instead of stdout.
		#[arg(short, long)]
		output: Option<PathBuf>,
	},
	/// Codama generation workflows.
	Codama {
		#[command(subcommand)]
		command: CodamaCommands,
	},
}

#[derive(Subcommand, Debug)]
enum CodamaCommands {
	/// Generate IDLs and Rust/JS clients for one or more examples.
	Generate {
		/// Directory containing example program crates.
		#[arg(long, default_value = "examples")]
		examples_dir: PathBuf,

		/// Output directory for generated IDL JSON files.
		#[arg(long, default_value = "codama/idls")]
		idls_dir: PathBuf,

		/// Output directory for generated Rust clients.
		#[arg(long, default_value = "codama/clients/rust")]
		rust_out: PathBuf,

		/// Output directory for generated JS clients.
		#[arg(long, default_value = "codama/clients/js")]
		js_out: PathBuf,

		/// Example name filter. Repeat to generate a subset.
		#[arg(long = "example")]
		examples: Vec<String>,

		/// Executable used to invoke npx.
		#[arg(long, default_value = "npx")]
		npx: String,
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
		Commands::Docs { topic } => run_docs(&topic),
		Commands::Init { name, path, force } => run_init(name.as_str(), path.as_deref(), force),
		Commands::Profile { path, json, output } => run_profile(&path, json, output.as_deref()),
		Commands::Codama { command } => {
			match command {
				CodamaCommands::Generate {
					examples_dir,
					idls_dir,
					rust_out,
					js_out,
					examples,
					npx,
				} => run_codama_generate(examples_dir, idls_dir, rust_out, js_out, examples, npx),
			}
		}
	}
}

fn run_idl(path: &Path, output: Option<&Path>, name: Option<&str>, pretty: bool) {
	let root = match pina_cli::generate_idl(path, name) {
		Ok(r) => r,
		Err(e) => {
			eprintln!("{} {}", "Error".red().bold(), e);
			std::process::exit(1);
		}
	};

	// Print Summary Table
	let mut table = Table::new();
	table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);
	table.set_header(vec!["Component", "Count"]);
	table.add_row(vec![
		"Instructions",
		&root.program.instructions.len().to_string(),
	]);
	table.add_row(vec!["Accounts", &root.program.accounts.len().to_string()]);
	table.add_row(vec!["PDAs", &root.program.pdas.len().to_string()]);
	table.add_row(vec!["Errors", &root.program.errors.len().to_string()]);

	println!("\n{} ──", "✨ Generation Complete".green().bold());
	println!("{table}");

	let json = if pretty {
		serde_json::to_string_pretty(&root)
	} else {
		serde_json::to_string(&root)
	};

	let json = match json {
		Ok(j) => j,
		Err(e) => {
			eprintln!("{} JSON serialization error: {}", "Error".red().bold(), e);
			std::process::exit(1);
		}
	};

	if let Some(output) = output {
		if let Err(e) = fs::write(output, &json) {
			eprintln!(
				"{} Failed to write {}: {}",
				"Error".red().bold(),
				output.display(),
				e
			);
			std::process::exit(1);
		}

		return;
	}

	println!("\n{json}");
}

fn run_docs(topic: &str) {
	let mut attempted_paths = Vec::new();

	if let Ok(template_dir) = std::env::var("PINA_TEMPLATES_DIR") {
		let template_path = PathBuf::from(template_dir).join(format!("{topic}.t.md"));
		attempted_paths.push(template_path.clone());

		if template_path.is_file() {
			let content = match fs::read_to_string(&template_path) {
				Ok(c) => c,
				Err(e) => {
					eprintln!(
						"{} Failed to read template {}: {}",
						"Error".red().bold(),
						template_path.display(),
						e
					);
					std::process::exit(1);
				}
			};
			render_docs(&content);
			return;
		}
	}

	if let Some(content) = bundled_docs(topic) {
		render_docs(content);
		return;
	}

	eprintln!(
		"{} Topic `{}` not found. Available bundled topics: {}.",
		"Error".red().bold(),
		topic,
		BUNDLED_DOC_TOPICS.join(", ")
	);

	if !attempted_paths.is_empty() {
		eprintln!("Attempted template paths:");
		for path in attempted_paths {
			eprintln!("  - {}", path.display());
		}
	}

	eprintln!(
		"Set PINA_TEMPLATES_DIR to a directory containing `<topic>.t.md` to load custom docs."
	);
	std::process::exit(1);
}

const BUNDLED_DOC_TOPICS: &[&str] = &["pina-idl", "pina-overview"];

fn bundled_docs(topic: &str) -> Option<&'static str> {
	match topic {
		"pina-idl" => Some(include_str!("../templates/pina-idl.t.md")),
		"pina-overview" => Some(include_str!("../templates/pina-overview.t.md")),
		_ => None,
	}
}

fn render_docs(content: &str) {
	let skin = termimad::MadSkin::default();
	skin.print_text(content);
}

fn run_init(name: &str, path: Option<&Path>, force: bool) {
	let project_path = path.map_or_else(|| PathBuf::from(name), PathBuf::from);

	if let Err(err) = pina_cli::init_project(&project_path, name, force) {
		eprintln!("{} {}", "Error".red().bold(), err);
		std::process::exit(1);
	}

	println!(
		"{} Initialized new Pina project at {}",
		"✔".green(),
		project_path.display()
	);
	pina_cli::print_next_steps(&project_path, name);
}

fn run_profile(path: &Path, json: bool, output: Option<&Path>) {
	if let Some(output_path) = output {
		if output_path == path {
			eprintln!(
				"{} Refusing to overwrite input binary {}",
				"Error".red().bold(),
				path.display()
			);
			std::process::exit(1);
		}
	}

	let profile = match pina_profile::profile_program(path) {
		Ok(p) => p,
		Err(e) => {
			eprintln!("{} {}", "Error".red().bold(), e);
			std::process::exit(1);
		}
	};

	let format = if json {
		pina_profile::OutputFormat::Json
	} else {
		pina_profile::OutputFormat::Text
	};

	if let Some(output_path) = output {
		let mut file = match fs::File::create(output_path) {
			Ok(f) => f,
			Err(e) => {
				eprintln!(
					"{} Failed to create {}: {}",
					"Error".red().bold(),
					output_path.display(),
					e
				);
				std::process::exit(1);
			}
		};

		if let Err(e) = pina_profile::output::write_profile(&profile, format, &mut file) {
			eprintln!("{} {}", "Error".red().bold(), e);
			std::process::exit(1);
		}

		return;
	}

	let mut stdout = std::io::stdout().lock();

	if let Err(e) = pina_profile::output::write_profile(&profile, format, &mut stdout) {
		eprintln!("{} {}", "Error".red().bold(), e);
		std::process::exit(1);
	}
}

fn run_codama_generate(
	examples_dir: PathBuf,
	idls_dir: PathBuf,
	rust_out: PathBuf,
	js_out: PathBuf,
	examples: Vec<String>,
	npx: String,
) {
	let options = pina_cli::CodamaGenerateOptions {
		examples_dir,
		idls_dir,
		rust_out,
		js_out,
		examples,
		npx,
	};

	let generated_examples = match pina_cli::generate_codama(&options) {
		Ok(examples) => examples,
		Err(err) => {
			eprintln!("{} {}", "Error".red().bold(), err);
			std::process::exit(1);
		}
	};

	println!(
		"{} Generated Codama IDLs and Rust/JS clients for {} example(s): {}",
		"✔".green(),
		generated_examples.len(),
		generated_examples.join(", "),
	);
}
