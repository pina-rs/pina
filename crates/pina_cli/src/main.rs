mod cli;

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use comfy_table::Table;
use owo_colors::OwoColorize;

use crate::cli::Cli;
use crate::cli::ClientArg;
use crate::cli::CodamaCommands;
use crate::cli::Commands;

fn main() {
	let cli = Cli::parse();

	match cli.command {
		Commands::Build {
			project,
			features,
			no_default_features,
			verify,
			solana_verify,
		} => {
			run_build(
				project,
				features,
				no_default_features,
				verify,
				solana_verify,
			);
		}
		Commands::Generate {
			project,
			clients,
			output,
			npx,
		} => run_generate(project, clients, output, npx),
		Commands::Idl {
			path,
			output,
			name,
			compact,
			pretty: _,
		} => run_idl(path.as_path(), output.as_deref(), name.as_deref(), !compact),
		Commands::Docs { topic } => run_docs(topic.as_deref()),
		Commands::Init { name, path, force } => run_init(name.as_str(), path.as_deref(), force),
		Commands::Profile { path, json, output } => run_profile(&path, json, output.as_deref()),
		Commands::Codama { command } => {
			match command {
				CodamaCommands::Generate {
					examples_dir,
					idls_dir,
					rust_out,
					js_out,
					dart_out,
					examples,
					npx,
				} => {
					run_codama_generate(
						examples_dir,
						idls_dir,
						rust_out,
						js_out,
						dart_out,
						examples,
						npx,
					);
				}
			}
		}
	}
}

fn run_build(
	project: PathBuf,
	features: Vec<String>,
	no_default_features: bool,
	verify: bool,
	solana_verify: std::ffi::OsString,
) {
	let options = pina_cli::build::BuildOptions {
		project_dir: project,
		features,
		no_default_features,
	};
	let output = if verify {
		pina_cli::build::build_project_verified_with_options(
			&options,
			&pina_cli::build::VerifyBuildOptions {
				executable: solana_verify,
			},
		)
		.map(|verified| {
			(
				verified.build,
				Some((verified.verifiable_artifact, verified.verification_manifest)),
			)
		})
	} else {
		pina_cli::build::build_project_with_options(&options).map(|build| (build, None))
	};
	let (output, verification_manifest) = match output {
		Ok(output) => output,
		Err(error) => {
			eprintln!("{} {}", "Error".red().bold(), error);
			std::process::exit(1);
		}
	};

	println!("{} Built {}", "✔".green(), output.package_name);
	println!("  SBF  {}", output.sbf_artifact.display());
	println!("  IDL  {}", output.idl.display());
	if let Some((artifact, manifest)) = verification_manifest {
		println!("  Verified SBF {}", artifact.display());
		println!("  Build record {}", manifest.display());
	}
}

fn run_generate(project: PathBuf, clients: Vec<ClientArg>, output: Option<PathBuf>, npx: String) {
	let clients = clients
		.into_iter()
		.map(|client| {
			match client {
				ClientArg::Rust => pina_cli::project::ClientLanguage::Rust,
				ClientArg::Typescript => pina_cli::project::ClientLanguage::Typescript,
				ClientArg::Dart => pina_cli::project::ClientLanguage::Dart,
			}
		})
		.collect();
	let options = pina_cli::ProjectGenerateOptions {
		project_dir: project,
		clients,
		output,
		npx,
	};
	let generated = match pina_cli::generate_project_clients(&options) {
		Ok(generated) => generated,
		Err(error) => {
			eprintln!("{} {}", "Error".red().bold(), error);
			std::process::exit(1);
		}
	};
	let clients = generated
		.clients
		.iter()
		.map(|client| client.as_str())
		.collect::<Vec<_>>()
		.join(", ");

	println!(
		"{} Generated {} client(s) for {}: {}",
		"✔".green(),
		generated.clients.len(),
		generated.package_name,
		clients
	);
	println!("  IDL     {}", generated.idl.display());
	println!("  Clients {}", generated.clients_dir.display());
}

fn run_idl(path: &Path, output: Option<&Path>, name: Option<&str>, pretty: bool) {
	let root = match pina_cli::generate_idl(path, name) {
		Ok(r) => r,
		Err(e) => {
			eprintln!("{} {}", "Error".red().bold(), e);
			std::process::exit(1);
		}
	};

	let mut table = Table::new();
	table.load_style(comfy_table::presets::UTF8_FULL_CONDENSED);
	table.set_header(vec!["Component", "Count"]);
	table.add_row(vec![
		"Instructions",
		&root.program.instructions.len().to_string(),
	]);
	table.add_row(vec!["Accounts", &root.program.accounts.len().to_string()]);
	table.add_row(vec!["PDAs", &root.program.pdas.len().to_string()]);
	table.add_row(vec!["Errors", &root.program.errors.len().to_string()]);

	eprintln!("{}", "IDL generation complete".green().bold());
	eprintln!("{table}");

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

		eprintln!("Wrote {}", output.display());

		return;
	}

	println!("{json}");
}

fn run_docs(topic: Option<&str>) {
	let Some(topic) = topic else {
		println!("Bundled documentation topics:");

		for (name, description) in BUNDLED_DOC_TOPICS {
			println!("  {name:<14} {description}");
		}

		println!("\nRun `pina docs <topic>` to open a topic.");
		println!("Set PINA_TEMPLATES_DIR to load additional `<topic>.t.md` files.");

		return;
	};

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

	eprintln!("{} Topic `{}` not found.", "Error".red().bold(), topic);
	eprintln!("Bundled topics:");

	for (name, description) in BUNDLED_DOC_TOPICS {
		eprintln!("  {name:<14} {description}");
	}

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

const BUNDLED_DOC_TOPICS: &[(&str, &str)] = &[
	("pina-idl", "IDL extraction rules and supported Rust shapes"),
	(
		"pina-overview",
		"framework concepts, crates, features, and workflows",
	),
];

fn bundled_docs(topic: &str) -> Option<&'static str> {
	match topic {
		"pina-idl" => Some(include_str!("../templates/pina-idl.md")),
		"pina-overview" => Some(include_str!("../templates/pina-overview.md")),
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
	if let Some(output_path) = output
		&& output_path == path
	{
		eprintln!(
			"{} Refusing to overwrite input binary {}",
			"Error".red().bold(),
			path.display()
		);
		std::process::exit(1);
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
	dart_out: PathBuf,
	examples: Vec<String>,
	npx: String,
) {
	let options = pina_cli::CodamaGenerateOptions {
		examples_dir,
		idls_dir,
		rust_out,
		js_out,
		dart_out,
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
		"{} Generated Codama IDLs and Rust/JavaScript/Dart clients for {} example(s): {}",
		"✔".green(),
		generated_examples.len(),
		generated_examples.join(", "),
	);
}
