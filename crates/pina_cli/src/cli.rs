//! Command-line interface definition and user-facing help.

use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;

/// Build, inspect, and generate artifacts for Pina Solana programs.
#[derive(Parser, Debug)]
#[command(
	name = "pina",
	version,
	about = "Build, inspect, and generate artifacts for Pina Solana programs",
	long_about = "Build, inspect, and generate artifacts for Pina Solana programs.\n\nUse 'pina \
	              init' to start a program, 'pina idl' to inspect its public interface, 'pina \
	              codama generate' to render clients, and 'pina profile' to inspect a compiled \
	              SBF binary.",
	next_line_help = true,
	arg_required_else_help = true,
	after_help = "Examples:\n  pina init counter_program\n  pina idl --path \
	              ./programs/counter_program --output ./idls/counter_program.json\n  pina codama \
	              generate --example counter_program\n  pina profile \
	              ./target/deploy/counter_program.so --json\n\nAgent discovery:\n  Run 'pina \
	              <command> --help' for command-specific inputs, outputs, and examples.\n  Run \
	              'pina docs' to list the bundled architecture and IDL reference topics."
)]
pub(crate) struct Cli {
	#[command(subcommand)]
	pub(crate) command: Commands,
}

/// Operations supported by the Pina toolchain.
#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
	/// Generate a Codama IDL from a Pina program crate.
	///
	/// Parses the crate rooted at PATH, follows Rust modules from src/lib.rs,
	/// and emits a Codama root-node JSON document. JSON is written to stdout
	/// unless OUTPUT is supplied; progress and the extraction summary are
	/// written to stderr.
	#[command(after_help = "Examples:\n  pina idl\n  pina idl --path \
	                        ./programs/counter_program\n  pina idl -p ./programs/counter_program \
	                        -o ./idls/counter_program.json\n  pina idl -p \
	                        ./programs/counter_program --compact")]
	Idl {
		/// Program crate directory containing Cargo.toml and src/lib.rs. Defaults to the current directory.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		path: PathBuf,

		/// Write the JSON document to FILE instead of stdout.
		#[arg(short, long, value_name = "FILE")]
		output: Option<PathBuf>,

		/// Override the program name inferred from Cargo.toml.
		#[arg(short, long, value_name = "NAME")]
		name: Option<String>,

		/// Emit compact JSON instead of the default pretty-printed JSON.
		#[arg(long)]
		compact: bool,

		/// Preserve compatibility with the former explicit pretty-print flag.
		#[arg(long, hide = true, conflicts_with = "compact")]
		pretty: bool,
	},

	/// Read bundled Pina reference documentation in the terminal.
	///
	/// Run without TOPIC to list every bundled topic. Set `PINA_TEMPLATES_DIR`
	/// to a directory containing `<topic>.t.md` files to override or add topics.
	#[command(
		after_help = "Examples:\n  pina docs\n  pina docs pina-overview\n  pina docs pina-idl\n  \
		              PINA_TEMPLATES_DIR=./templates pina docs team-conventions"
	)]
	Docs {
		/// Topic to render. Omit to list bundled topics.
		#[arg(value_name = "TOPIC")]
		topic: Option<String>,
	},

	/// Initialize a new Pina program project.
	///
	/// Creates a buildable program scaffold with Pina dependencies, source
	/// layout, and a starter instruction. Existing scaffold files are preserved
	/// unless --force is supplied.
	#[command(
		after_help = "Examples:\n  pina init counter_program\n  pina init counter_program --path \
		              ./programs/counter_program\n  pina init counter_program --path \
		              ./programs/counter_program --force"
	)]
	Init {
		/// Rust package name for the new program, for example `counter_program`.
		#[arg(value_name = "NAME")]
		name: String,

		/// Target directory. Defaults to `./<name>`.
		#[arg(short, long, value_name = "DIR")]
		path: Option<PathBuf>,

		/// Overwrite scaffold files that already exist.
		#[arg(long)]
		force: bool,
	},

	/// Profile compute-unit costs in a compiled SBF program.
	///
	/// Performs static analysis of an SBF shared object. Text is written to
	/// stdout by default. Use --json for machine-readable output and --output
	/// to write either format to a file.
	#[command(
		after_help = "Examples:\n  pina profile ./target/deploy/counter_program.so\n  pina \
		              profile ./target/deploy/counter_program.so --json\n  pina profile \
		              ./target/deploy/counter_program.so --json -o ./profile.json"
	)]
	Profile {
		/// Compiled SBF shared object to analyze.
		#[arg(value_name = "PROGRAM.SO")]
		path: PathBuf,

		/// Emit machine-readable JSON instead of the text report.
		#[arg(long)]
		json: bool,

		/// Write the report to FILE instead of stdout.
		#[arg(short, long, value_name = "FILE")]
		output: Option<PathBuf>,
	},

	/// Run Codama IDL and client-generation workflows.
	///
	/// Use the generate subcommand to extract every selected program IDL and
	/// render the corresponding Rust, JavaScript, and Dart clients.
	Codama {
		#[command(subcommand)]
		command: CodamaCommands,
	},
}

/// Codama-related generation workflows.
#[derive(Subcommand, Debug)]
pub(crate) enum CodamaCommands {
	/// Generate IDLs and Rust, JavaScript, and Dart clients.
	///
	/// Discovers Pina programs below `EXAMPLES_DIR`, optionally filters them with
	/// repeatable --example arguments, writes IDLs, and renders all three client
	/// targets. The command fails when a requested example does not exist or a
	/// renderer exits unsuccessfully.
	#[command(
		after_help = "Examples:\n  pina codama generate\n  pina codama generate --example \
		              counter_program --example todo_program\n  pina codama generate \
		              --examples-dir ./programs --idls-dir ./idls \\\n                --rust-out \
		              ./clients/rust --js-out ./clients/js --dart-out \
		              ./clients/dart\n\nRequirements:\n  The selected --npx executable, or the \
		              default pnpm fallback, must be available when JavaScript and Dart client \
		              rendering runs."
	)]
	Generate {
		/// Directory containing Pina program crates. Defaults to `examples`.
		#[arg(
			long,
			default_value = "examples",
			hide_default_value = true,
			value_name = "DIR"
		)]
		examples_dir: PathBuf,

		/// Directory for generated Codama IDL JSON files. Defaults to `codama/idls`.
		#[arg(
			long,
			default_value = "codama/idls",
			hide_default_value = true,
			value_name = "DIR"
		)]
		idls_dir: PathBuf,

		/// Directory for generated Rust client crates. Defaults to `codama/clients/rust`.
		#[arg(
			long,
			default_value = "codama/clients/rust",
			hide_default_value = true,
			value_name = "DIR"
		)]
		rust_out: PathBuf,

		/// Directory for generated JavaScript client packages. Defaults to `codama/clients/js`.
		#[arg(
			long,
			default_value = "codama/clients/js",
			hide_default_value = true,
			value_name = "DIR"
		)]
		js_out: PathBuf,

		/// Directory for the generated Dart client package. Defaults to `codama/clients/dart`.
		#[arg(
			long,
			default_value = "codama/clients/dart",
			hide_default_value = true,
			value_name = "DIR"
		)]
		dart_out: PathBuf,

		/// Program name to generate. Repeat to select multiple programs.
		/// When omitted, every program below --examples-dir is generated.
		#[arg(long = "example", value_name = "NAME")]
		examples: Vec<String>,

		/// Executable used to invoke the Codama JavaScript renderers. Defaults to `npx`.
		#[arg(
			long,
			default_value = "npx",
			hide_default_value = true,
			value_name = "COMMAND"
		)]
		npx: String,
	},
}
