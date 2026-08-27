//! Command-line interface definition and user-facing help.

use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;

/// Build, inspect, and generate artifacts for Pina Solana programs.
#[derive(Parser, Debug)]
#[command(
	name = "pina",
	bin_name = "pina",
	version,
	about = "Build, inspect, and generate artifacts for Pina Solana programs",
	long_about = "Build, inspect, and generate artifacts for Pina Solana programs.\n\nUse 'pina \
	              init' to start a program, 'pina build' for its SBF binary and IDL, and 'pina \
	              generate' for selected client ecosystems. Low-level IDL, profiling, terminal \
	              documentation, and the legacy repository-wide Codama workflow remain available.",
	next_line_help = true,
	arg_required_else_help = true,
	after_help = "Examples:\n  pina init counter_program\n  cd counter_program && pina build\n  \
	              pina generate --client rust --client typescript\n  pina idl --path \
	              ./programs/counter_program --output ./idls/counter_program.json\n  pina profile \
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
	/// Build the current Pina program for SBF and generate its IDL.
	///
	/// Discovers the nearest Pina.toml or Cargo package, runs the release SBF
	/// build, publishes the program at target/deploy/<program>.so, and writes the
	/// Codama IDL to target/idl/<program>.json. Cargo output is streamed directly.
	#[command(after_help = "Examples:\n  pina build\n  pina build --project \
	                        ./programs/counter\n\nProject discovery:\n  Pina searches from \
	                        --project (or the current directory) toward the filesystem root for \
	                        Pina.toml. Existing Cargo projects without Pina.toml are discovered \
	                        with cargo metadata. CARGO_TARGET_DIR is respected.")]
	Build {
		/// Directory inside the project to discover. Defaults to the current directory.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Program features to enable in addition to bpf-entrypoint. Repeat or use commas.
		#[arg(long, value_delimiter = ',', value_name = "FEATURE")]
		features: Vec<String>,

		/// Disable the program crate's default features for the SBF build.
		#[arg(long)]
		no_default_features: bool,
	},

	/// Generate configured clients for the current Pina program.
	///
	/// Discovers the project, refreshes its IDL, and generates only the selected
	/// client ecosystems. Repeat --client to override Pina.toml. Rust-only
	/// generation does not invoke Node.js.
	#[command(
		after_help = "Examples:\n  pina generate\n  pina generate --client rust\n  pina generate \
		              --client typescript --client dart\n  pina generate --project \
		              ./programs/counter --output ./generated\n\nConfiguration:\n  [clients]\n  \
		              output = \"clients\"\n  languages = [\"rust\", \"typescript\"]"
	)]
	Generate {
		/// Directory inside the project to discover. Defaults to the current directory.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Client ecosystem to generate. Repeat to override Pina.toml.
		#[arg(long = "client", value_enum, value_name = "LANGUAGE")]
		clients: Vec<ClientArg>,

		/// Override the configured client output directory.
		#[arg(short, long, value_name = "DIR")]
		output: Option<PathBuf>,

		/// Executable used to invoke JavaScript or Dart renderers. Defaults to npx.
		#[arg(
			long,
			default_value = "npx",
			hide_default_value = true,
			value_name = "COMMAND"
		)]
		npx: String,
	},

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

/// Client ecosystems supported by project-aware generation.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ClientArg {
	Rust,
	Typescript,
	Dart,
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
