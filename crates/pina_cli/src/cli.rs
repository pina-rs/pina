//! Command-line interface definition and user-facing help.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use clap_complete::Shell;

/// Build, inspect, and generate artifacts for Pina Solana programs.
#[derive(Parser, Debug)]
#[command(
	name = "pina",
	bin_name = "pina",
	version,
	about = "Build, test, inspect, and generate artifacts for Pina Solana programs",
	long_about = "Build, test, inspect, and generate artifacts for Pina Solana programs.\n\nUse \
	              'pina init' to start a program, 'pina lint' for the official security lint set, \
	              'pina build' for its SBF binary and IDL, 'pina test' for SBF integration, 'pina \
	              test --unit' for the fast native/Mollusk loop, 'pina dev' for a persistent \
	              Surfpool network, 'pina generate' for selected client ecosystems, 'pina deploy' \
	              for explicit cluster deployment, and 'pina verify' for deployed-program \
	              verification. Use 'pina doctor' for agent-readable diagnostics, 'pina keys' for \
	              program identity, and 'pina completions' for shell integration. Low-level IDL, \
	              profiling, terminal documentation, and the legacy repository-wide Codama \
	              workflow remain available.",
	next_line_help = true,
	arg_required_else_help = true,
	after_help = "Examples:\n  pina init counter_program\n  cd counter_program && pina build\n  \
	              pina lint\n  pina test\n  pina test --unit\n  pina dev --yes\n  pina generate \
	              --client rust --client typescript\n  pina doctor --json\n  pina keys\n  pina \
	              idl --path ./programs/counter_program --output ./idls/counter_program.json\n  \
	              pina profile ./target/deploy/counter_program.so --json\n  pina deploy --cluster \
	              localnet --payer ~/.config/solana/id.json --upgrade-authority \
	              ~/.config/solana/id.json --dry-run\n\nAgent discovery:\n  Run 'pina <command> \
	              --help' for command-specific inputs, outputs, and examples.\n  Run 'pina docs' \
	              to list the bundled architecture and IDL reference topics.\n  For deployment \
	              verification, run 'pina verify --help' and then inspect the selected leaf \
	              command."
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
	/// Discovers the nearest pina.toml or Cargo package, runs the release SBF
	/// build, publishes the program at target/deploy/<program>.so, and writes the
	/// Codama IDL to target/idl/<program>.json. With --verify, solana-verify
	/// performs a deterministic Docker build and Pina records its inputs and hash.
	#[command(
		after_help = "Examples:\n  pina build\n  pina build --verify\n  pina build --verify \
		              --features logs --no-default-features\n  pina build --project \
		              ./programs/counter\n\nVerified builds:\n  --verify switches the compiler \
		              backend to solana-verify 0.5.1. Install that exact version and start Docker \
		              first. Pina never installs tools. This creates a deterministic build; it \
		              does not compare with an on-chain program.\n\nProject discovery:\n  Pina \
		              searches from --project (or the current directory) toward the filesystem \
		              root for pina.toml. Existing Cargo projects without pina.toml are \
		              discovered with cargo metadata. CARGO_TARGET_DIR is respected."
	)]
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

		/// Build deterministically with solana-verify and Docker instead of Cargo directly.
		#[arg(long)]
		verify: bool,

		/// solana-verify 0.5.1 executable. Pina does not install it automatically.
		#[arg(
			long,
			default_value = "solana-verify",
			hide_default_value = true,
			requires = "verify",
			value_name = "COMMAND"
		)]
		solana_verify: OsString,
	},

	/// Run Pina's official security lints against the current program.
	///
	/// Discovers the nearest pina.toml or Cargo package, prepares the
	/// pina_lint_driver binary for the active toolchain under Cargo home, and
	/// runs cargo check with the driver as RUSTC_WRAPPER. Lint levels configured
	/// in the [lints] table of pina.toml are honored. Use --fix to apply
	/// machine-applicable suggestions; review every resulting source change.
	#[command(
		after_help = "Examples:\n  pina lint\n  pina lint --fix\n  pina lint --project \
		              ./programs/counter\n\nTooling:\n  The first run installs the lint driver \
		              built from the pina_lints release matching this CLI below Cargo home; later \
		              runs reuse it. --fix applies only machine-applicable suggestions and allows \
		              Cargo to edit dirty, staged, or not-yet-versioned working trees; inspect \
		              the diff before committing."
	)]
	Lint {
		/// Directory inside the project to discover. Defaults to the current directory.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Apply machine-applicable lint suggestions to the working tree.
		#[arg(long)]
		fix: bool,
	},

	/// Generate configured clients for the current Pina program.
	///
	/// Discovers the project, refreshes its IDL, and generates only the selected
	/// client ecosystems. Repeat --client to override pina.toml. Rust-only
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

		/// Client ecosystem to generate. Repeat to override pina.toml.
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

	/// Generate, inspect, and publish canonical Codama IDLs.
	///
	/// Bare invocation preserves local generation: it parses PATH and emits a
	/// Codama root-node JSON document. The generate subcommand spells that action
	/// explicitly. Fetch, diff, and publish manage canonical `idl` metadata for a
	/// deployed program through the pinned official Program Metadata client.
	#[command(after_help = "Examples:\n  pina idl\n  pina idl --path \
	                        ./programs/counter_program\n  pina idl -p ./programs/counter_program \
	                        -o ./idls/counter_program.json\n  pina idl -p \
	                        ./programs/counter_program --compact")]
	Idl {
		/// IDL generation and canonical on-chain publication operation.
		#[command(subcommand)]
		command: Option<IdlCommands>,

		/// IDL generation options used when no subcommand is supplied.
		#[command(flatten)]
		generate: IdlGenerateArgs,
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

	/// Inspect or synchronize the current program's identity.
	///
	/// Run without a subcommand to compare the source `declare_id!` with the
	/// conventional local keypair. `pina keys sync` explicitly updates only that
	/// string literal after validating an unambiguous declaration and keypair.
	#[command(
		after_help = "Examples:\n  pina keys\n  pina keys show --json\n  pina keys --path \
		              ./programs/counter\n  pina keys sync\n  pina keys sync --keypair \
		              ./keys/counter-keypair.json\n  pina keys new\n  pina keys new --force"
	)]
	Keys {
		/// Inspect or synchronize a specific program crate. Defaults to the nearest project.
		#[arg(
			short,
			long,
			global = true,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		path: PathBuf,

		/// Use KEYPAIR instead of `target/deploy/<program>-keypair.json`.
		#[arg(long, global = true, value_name = "KEYPAIR")]
		keypair: Option<PathBuf>,

		/// Emit a stable machine-readable JSON document.
		#[arg(long, global = true)]
		json: bool,

		#[command(subcommand)]
		command: Option<KeysCommands>,
	},

	/// Diagnose the current project and development toolchain.
	///
	/// Checks project discovery, source identity, conventional artifact paths,
	/// and relevant Rust, Solana, Surfpool, and configured client tools. Missing
	/// required prerequisites or a project produce an unsuccessful exit.
	#[command(
		after_help = "Examples:\n  pina doctor\n  pina doctor --json\n  pina doctor --path \
		              ./programs/counter\n\nAgent use:\n  `pina doctor --json` emits \
		              schemaVersion, status, project, tools, checks, and findings without color \
		              or progress output."
	)]
	Doctor {
		/// Diagnose the nearest program package at or above DIR.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		path: PathBuf,

		/// Emit a stable machine-readable JSON document.
		#[arg(long)]
		json: bool,
	},

	/// Generate shell completion scripts.
	///
	/// Writes the selected completion script to stdout so it can be redirected,
	/// sourced, or installed by a package manager without temporary files.
	#[command(after_help = "Examples:\n  pina completions bash > \
	                        ~/.local/share/bash-completion/completions/pina\n  pina completions \
	                        zsh > ~/.zfunc/_pina\n  pina completions fish > \
	                        ~/.config/fish/completions/pina.fish")]
	Completions {
		/// Shell whose completion script should be generated.
		#[arg(value_enum, value_name = "SHELL")]
		shell: Shell,
	},

	/// Test a Pina program with fast native tests or an isolated Surfpool instance.
	///
	/// The default workflow builds the actual SBF shared object, verifies that it
	/// exists, and runs the project's isolated `tests/surfpool` test package. That
	/// package owns an embedded Surfpool instance, so parallel runs use separate
	/// ports and teardown remains deterministic. Use --unit for native Rust and
	/// Mollusk tests only.
	#[command(
		after_help = "Examples:\n  pina test\n  pina test --filter initialize\n  pina test \
		              --unit\n  pina test --unit --filter rejects_wrong_owner\n\nTest layers:\n  \
		              --unit keeps the fast native/Mollusk loop and does not build SBF.\n  The \
		              default builds SBF and runs the ignored test in the isolated \
		              `tests/surfpool` package.\n\nSafety:\n  Embedded Surfpool tests allocate \
		              isolated ports and must stop their instance before returning. Missing SBF \
		              artifacts and incomplete Surfpool test packages are hard failures."
	)]
	Test {
		/// Project directory or a directory below it. Defaults to the current directory.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Run native Rust and Mollusk tests without building SBF or starting Surfpool.
		#[arg(long)]
		unit: bool,

		/// Run only tests whose names contain FILTER.
		#[arg(short, long, value_name = "FILTER")]
		filter: Option<String>,
	},

	/// Start a persistent Surfpool development network with SBF watch and redeploy.
	///
	/// Builds the current SBF artifact once, then delegates the persistent process,
	/// file watching, and redeployment to `surfpool start --watch`. The network is
	/// offline by default; opt into an upstream cluster or RPC URL explicitly.
	#[command(
		after_help = "Examples:\n  pina dev --yes  # first run: allow txtx.yml creation\n  pina \
		              dev\n  pina dev --network devnet\n  pina dev \
		              --rpc-url https://api.mainnet-beta.solana.com\n\nBehavior:\n  Pina builds \
		              the SBF artifact, then runs Surfpool in the foreground with --watch. Build \
		              again in another terminal to trigger redeployment. Surfpool inherits terminal \
		              input, output, and errors; Ctrl-C is handled by that foreground process.\n\nRunbook safety:\n  Surfpool can create or \
		              update txtx.yml. Pina refuses a missing runbook unless --yes explicitly \
		              authorizes first-run changes. Commit and review txtx.yml before subsequent \
		              runs.\n\nNetwork safety:\n  The default is --offline. Pina never opts into \
		              Surfpool's upstream mainnet datasource implicitly."
	)]
	Dev {
		/// Project directory or a directory below it. Defaults to the current directory.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Fork from mainnet, devnet, or testnet instead of using the offline default.
		#[arg(
			long,
			value_enum,
			conflicts_with = "rpc_url",
			hide_possible_values = true,
			value_name = "CLUSTER"
		)]
		network: Option<SurfpoolCluster>,

		/// Fork from a credential-free HTTP(S) RPC URL with a host.
		///
		/// User information, query parameters, fragments, and control characters are rejected.
		/// Surfpool receives the URL in its child-process arguments, so never put a secret anywhere
		/// in the URL, including its host or path. Prefer `--network` when possible.
		#[arg(long, conflicts_with = "network", value_name = "URL")]
		rpc_url: Option<String>,

		/// Allow Surfpool to create or update txtx.yml without prompting.
		#[arg(long)]
		yes: bool,
	},

	/// Profile compute-unit costs in a compiled SBF program.
	///
	/// Performs static analysis of an SBF shared object. Text is written to
	/// stdout by default. Use --json for machine-readable output and --output
	/// to write either format to a file.
	#[command(
		after_help = "Examples:\n  pina profile\n  pina profile --json\n  pina profile --project \
		              ./programs/counter_program\n  pina profile \
		              ./target/deploy/counter_program.so --json\n  pina profile \
		              ./target/deploy/counter_program.so --json -o ./profile.json"
	)]
	Profile {
		/// Compiled SBF shared object. Omit to discover the current project's artifact.
		#[arg(value_name = "PROGRAM.SO")]
		path: Option<PathBuf>,

		/// Project used for artifact discovery when PROGRAM.SO is omitted.
		#[arg(
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Emit machine-readable JSON instead of the text report.
		#[arg(long)]
		json: bool,

		/// Write the report to FILE instead of stdout.
		#[arg(short, long, value_name = "FILE")]
		output: Option<PathBuf>,
	},

	/// Verify deployed programs and publish source-build records.
	///
	/// Uses the official solana-verify 0.5.1 executable. `check` is read-only;
	/// `record` is the only subcommand that writes on-chain state. Remote
	/// submission and status use the mainnet verifier service.
	#[command(after_help = r#"Examples:
  pina verify check --program-id <ADDRESS> --cluster devnet
  pina verify record --program-id <ADDRESS> --cluster devnet \
    --build-record ./target/pina/verifiable/my_program-<HASH>.json \
    --authority ./authority.json --yes
  pina verify submit --program-id <ADDRESS> --uploader <ADDRESS>
  pina verify status --program-id <ADDRESS>"#)]
	Verify {
		#[command(subcommand)]
		command: VerifyCommands,

		/// solana-verify 0.5.1 executable. Pina never installs it.
		#[arg(
			long,
			default_value = "solana-verify",
			hide_default_value = true,
			value_name = "COMMAND"
		)]
		solana_verify: OsString,
	},

	/// Deploy a compiled Pina program through the Solana CLI.
	///
	/// Resolves the program artifact from a Cargo program crate or an explicit
	/// --program path, validates every keypair, prints the complete plan,
	/// and invokes `solana program deploy`. A cluster or RPC URL is always
	/// required. Remote writes require confirmation or --yes; mainnet also
	/// requires --allow-mainnet. Custom remote URLs require the same acknowledgement
	/// because their cluster identity cannot be proven.
	#[command(after_help = r"Examples:
  pina deploy --cluster localnet \
    --payer ~/.config/solana/id.json \
    --upgrade-authority ~/.config/solana/id.json
  pina deploy --build --cluster devnet \
    --payer ./keys/devnet-payer.json \
    --upgrade-authority ./keys/devnet-authority.json
  pina deploy --program ./artifacts/my_program.so \
    --program-keypair ./keys/my_program.json \
    --cluster http://127.0.0.1:8899 \
    --payer ./keys/local-payer.json \
    --upgrade-authority ./keys/local-authority.json --dry-run --json

Safety:
  No cluster is selected by default. --cluster accepts a named cluster or explicit RPC URL.
  Custom URL user information, queries, and fragments are rejected. Accepted hosts and paths
  remain visible in plans and process listings, so never put a secret anywhere in the URL.
  Prefer a named cluster when possible.
  Remote deployment prompts for the word deploy; use --yes only in reviewed automation.
  Mainnet and custom remote deployment additionally require --allow-mainnet.
  --dry-run never builds or deploys and --json is available only with --dry-run.")]
	Deploy {
		/// Directory used for pina.toml or Cargo metadata project discovery.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Existing SBF shared object. Conflicts with --build.
		#[arg(long, value_name = "PROGRAM.SO", conflicts_with = "build")]
		program: Option<PathBuf>,

		/// Run Pina's project-aware SBF build before final deployment planning.
		#[arg(long, conflicts_with = "dry_run")]
		build: bool,

		/// Keypair whose public key is the deployed program address.
		#[arg(long, value_name = "KEYPAIR")]
		program_keypair: Option<PathBuf>,

		/// Keypair authorized to upgrade the deployed program.
		#[arg(long, value_name = "KEYPAIR")]
		upgrade_authority: PathBuf,

		/// Keypair that pays the deployment transaction fees.
		#[arg(long, value_name = "KEYPAIR")]
		payer: PathBuf,

		/// Named cluster or explicit HTTP(S) RPC URL. No target is selected by default.
		#[arg(long, value_name = "CLUSTER|URL")]
		cluster: String,

		/// Print the resolved deployment plan without building or deploying.
		#[arg(long, conflicts_with = "yes")]
		dry_run: bool,

		/// Emit the dry-run plan as JSON. Requires --dry-run.
		#[arg(long, requires = "dry_run")]
		json: bool,

		/// Skip the remote deployment confirmation prompt.
		#[arg(long)]
		yes: bool,

		/// Acknowledge that mainnet-beta or a custom remote endpoint can affect real assets.
		#[arg(long, conflicts_with = "dry_run")]
		allow_mainnet: bool,
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

/// Arguments shared by `pina idl` and `pina idl generate`.
#[derive(clap::Args, Debug)]
pub(crate) struct IdlGenerateArgs {
	/// Program crate directory containing Cargo.toml and src/lib.rs. Defaults to the current directory.
	#[arg(
		short,
		long,
		default_value = ".",
		hide_default_value = true,
		value_name = "DIR"
	)]
	pub(crate) path: PathBuf,

	/// Write the JSON document to FILE instead of stdout.
	#[arg(short, long, value_name = "FILE")]
	pub(crate) output: Option<PathBuf>,

	/// Override the program name inferred from Cargo.toml.
	#[arg(short, long, value_name = "NAME")]
	pub(crate) name: Option<String>,

	/// Emit compact JSON instead of the default pretty-printed JSON.
	#[arg(long)]
	pub(crate) compact: bool,

	/// Preserve compatibility with the former explicit pretty-print flag.
	#[arg(long, hide = true, conflicts_with = "compact")]
	pub(crate) pretty: bool,
}

/// IDL generation and Program Metadata workflows.
#[derive(Subcommand, Debug)]
pub(crate) enum IdlCommands {
	/// Generate a Codama IDL from local Pina source.
	#[command(
		after_help = "Examples:\n  pina idl generate\n  pina idl generate --path \
		              ./programs/counter --output ./target/idl/counter.json\n\nCompatibility:\n  \
		              Bare 'pina idl [OPTIONS]' remains equivalent to this command."
	)]
	Generate(IdlGenerateArgs),

	/// Fetch a direct zlib/UTF-8 canonical on-chain IDL for a deployed program.
	#[command(
		after_help = "Examples:\n  pina idl fetch --cluster devnet --program-id <ADDRESS>\n  pina \
		              idl fetch --cluster mainnet-beta --program-id <ADDRESS> --output \
		              ./idl.json\n\nRequirements:\n  Uses the pinned official \
		              @solana-program/program-metadata package through npx. The canonical \
		              metadata seed is always 'idl'. URL, external-account, and other encoding \
		              formats fail closed in this initial workflow."
	)]
	Fetch {
		/// Solana cluster moniker or HTTP(S) RPC URL. This is always explicit.
		#[arg(long, value_name = "CLUSTER")]
		cluster: String,

		/// Deployed program address. Inferred from the local IDL when omitted.
		#[arg(long, value_name = "ADDRESS")]
		program_id: Option<String>,

		/// Directory inside the local project used for program ID inference.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Write the fetched IDL to FILE instead of stdout.
		#[arg(short, long, value_name = "FILE")]
		output: Option<PathBuf>,

		/// Emit a machine-readable result envelope.
		#[arg(long, conflicts_with = "output")]
		json: bool,

		/// npx-compatible runner for the pinned official package. May download version 0.9.0.
		#[arg(
			long,
			default_value = "npx",
			hide_default_value = true,
			value_name = "COMMAND"
		)]
		npx: String,
	},

	/// Compare a local IDL with canonical on-chain metadata.
	#[command(
		after_help = "Examples:\n  pina idl diff --cluster devnet --program-id <ADDRESS>\n  pina \
		              idl diff --cluster mainnet-beta --file ./target/idl/program.json \
		              --program-id <ADDRESS> --json\n\nComparison:\n  JSON object key order and \
		              whitespace are ignored. A difference exits with status 2, making the \
		              command useful in CI."
	)]
	Diff {
		/// Solana cluster moniker or HTTP(S) RPC URL. This is always explicit.
		#[arg(long, value_name = "CLUSTER")]
		cluster: String,

		/// Deployed program address. Inferred from the local IDL when omitted.
		#[arg(long, value_name = "ADDRESS")]
		program_id: Option<String>,

		/// Directory inside the local project used for IDL generation and discovery.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Compare FILE instead of generating the local project IDL.
		#[arg(long, value_name = "FILE")]
		file: Option<PathBuf>,

		/// Emit a machine-readable comparison result.
		#[arg(long)]
		json: bool,

		/// npx-compatible runner for the pinned official package. May download version 0.9.0.
		#[arg(
			long,
			default_value = "npx",
			hide_default_value = true,
			value_name = "COMMAND"
		)]
		npx: String,
	},

	/// Create or update the canonical on-chain IDL metadata account.
	#[command(
		after_help = "Examples:\n  pina idl publish --cluster devnet --authority \
		              ~/.config/solana/id.json --yes\n  pina idl publish --cluster mainnet-beta \
		              --program-id <ADDRESS> --file ./idl.json --authority \
		              ./upgrade-authority.json --export --output ./idl-plan.txt\n  pina idl \
		              publish --cluster mainnet-beta --program-id <ADDRESS> --file ./idl.json \
		              --export <MULTISIG> --output ./idl-update.txt\n\nSafety:\n  Publication is \
		              canonical, uses compressed inline JSON under the fixed 'idl' seed, and \
		              requires explicit confirmation. --export plans every required transaction \
		              without submitting any. An optional --export authority supports multisigs."
	)]
	Publish {
		/// Solana cluster moniker or HTTP(S) RPC URL. This is always explicit.
		#[arg(long, value_name = "CLUSTER")]
		cluster: String,

		/// Deployed program address. Inferred from the local IDL when omitted.
		#[arg(long, value_name = "ADDRESS")]
		program_id: Option<String>,

		/// Directory inside the local project used for IDL generation and discovery.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,

		/// Publish FILE instead of generating the local project IDL.
		#[arg(long, value_name = "FILE")]
		file: Option<PathBuf>,

		/// Upgrade-authority keypair for direct publication.
		#[arg(long, value_name = "KEYPAIR")]
		authority: Option<PathBuf>,

		/// Fee and rent payer keypair. Defaults to --authority.
		#[arg(long, value_name = "KEYPAIR", requires = "authority")]
		payer: Option<PathBuf>,

		/// Export every planned transaction instead of submitting. Optionally use ADDRESS as a noop multisig authority.
		#[arg(
			long,
			value_name = "ADDRESS",
			num_args = 0..=1,
			default_missing_value = "__pina_local_export__"
		)]
		export: Option<ExportArg>,

		/// Write every exported transaction to FILE instead of stdout.
		#[arg(short, long, value_name = "FILE", requires = "export")]
		output: Option<PathBuf>,

		/// Encoding for an exported multisig transaction.
		#[arg(long, value_enum, default_value = "base64", requires = "export")]
		export_encoding: IdlExportEncodingArg,

		/// Skip the interactive publication confirmation.
		#[arg(long, conflicts_with = "export")]
		yes: bool,

		/// Emit a machine-readable result envelope.
		#[arg(long, conflicts_with = "export")]
		json: bool,

		/// Priority fee in micro-lamports per compute unit.
		#[arg(long, default_value_t = 100_000, value_name = "MICROLAMPORTS")]
		priority_fee: u64,

		/// npx-compatible runner for the pinned official package. May download version 0.9.0.
		#[arg(
			long,
			default_value = "npx",
			hide_default_value = true,
			value_name = "COMMAND"
		)]
		npx: String,
	},
}

/// Supported official transaction export encodings.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum IdlExportEncodingArg {
	Base58,
	Base64,
}

/// Authority selection for no-submit transaction export.
#[derive(Debug, Clone)]
pub(crate) enum ExportArg {
	Local,
	Authority(String),
}

impl std::str::FromStr for ExportArg {
	type Err = String;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		if value == "__pina_local_export__" {
			Ok(Self::Local)
		} else {
			Ok(Self::Authority(value.to_owned()))
		}
	}
}

impl ExportArg {
	pub(crate) fn authority(&self) -> Option<&str> {
		match self {
			Self::Local => None,
			Self::Authority(authority) => Some(authority),
		}
	}
}

/// Client ecosystems supported by project-aware generation.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ClientArg {
	Rust,
	Typescript,
	Dart,
}

/// Program identity operations.
#[derive(Subcommand, Debug)]
pub(crate) enum KeysCommands {
	/// Show the source and local keypair program IDs without changing files.
	Show,

	/// Update the source `declare_id!` from a validated local keypair.
	///
	/// It fails before writing when the keypair is missing or malformed, the
	/// address is invalid, or source contains zero or multiple declarations.
	Sync,

	/// Generate a local keypair and synchronize the source program ID.
	///
	/// Existing keypairs are preserved unless --force explicitly authorizes
	/// identity rotation. Platforms where private permissions cannot be guaranteed
	/// fail before creating secret material.
	New {
		/// Replace an existing keypair and rotate the source program ID.
		#[arg(long)]
		force: bool,
	},
}

/// Deployed-program verification operations.
#[derive(Subcommand, Debug)]
pub(crate) enum VerifyCommands {
	/// Compare a local SBF executable with a deployed program.
	///
	/// Uses solana-verify's trailing-zero-aware executable and deployed-program
	/// hashes. Exits 0 on a match, 2 on a mismatch, and 1 on an operational error.
	#[command(after_help = r#"Examples:
  pina verify check --program-id <ADDRESS> --cluster devnet
  pina verify check --program-id <ADDRESS> --cluster mainnet-beta \
    --program ./target/deploy/my_program.so
  pina verify check --program-id <ADDRESS> --cluster https://rpc.example.com \
    --project ./programs/my_program

Output and status:
  Exit 0 means the executables match. Exit 2 means the comparison completed but
  hashes differ. Exit 1 means validation, RPC, or tool failure. Custom RPC URLs
  must be credential-free origins because RPC URLs are visible in process argv."#)]
	Check {
		/// Deployed program address.
		#[arg(long, value_name = "ADDRESS")]
		program_id: String,

		/// Solana cluster alias or HTTP(S) RPC URL. Always explicit.
		#[arg(long, value_name = "CLUSTER")]
		cluster: String,

		/// Local SBF executable. Conflicts with --project discovery.
		#[arg(long, value_name = "PROGRAM.SO", conflicts_with = "project")]
		program: Option<PathBuf>,

		/// Directory used to discover the executable. Defaults to the current directory.
		#[arg(
			short,
			long,
			default_value = ".",
			hide_default_value = true,
			value_name = "DIR"
		)]
		project: PathBuf,
	},

	/// Build from an immutable repository revision and record verification metadata.
	///
	/// This writes on-chain state after the official repository build matches the
	/// deployed executable. The authority signs and pays transaction fees because
	/// solana-verify 0.5.1 does not support a separate payer.
	#[command(after_help = r#"Examples:
  pina verify record --program-id <ADDRESS> --cluster devnet \
    --build-record ./target/pina/verifiable/my_program-<HASH>.json \
    --authority ./upgrade-authority.json --yes

  pina verify record --program-id <ADDRESS> --cluster mainnet-beta \
    --build-record ./target/pina/verifiable/my_program-<HASH>.json \
    --authority ./upgrade-authority.json --yes --acknowledge-mainnet

  pina verify record --program-id <ADDRESS> --cluster mainnet-beta \
    --build-record ./target/pina/verifiable/my_program-<HASH>.json \
    --export <MULTISIG_ADDRESS> --output ./verification.tx \
    --export-encoding base64

Safety:
  The build record binds the validated artifact hash to its public repository,
  full revision, build paths, library, and Cargo features. Interactive recording
  requires typing `record`; automation requires --yes. Submissions to mainnet and
  unknown remote RPC origins additionally require --acknowledge-mainnet. Export
  never submits or rebuilds the repository; remote verification begins only after
  the exported transaction is submitted. Export does not require that acknowledgement."#)]
	Record {
		/// Deployed program address.
		#[arg(long, value_name = "ADDRESS")]
		program_id: String,

		/// Solana cluster alias or HTTP(S) RPC URL. Always explicit.
		#[arg(long, value_name = "CLUSTER")]
		cluster: String,

		/// Content-addressed record produced by `pina build --verify`.
		#[arg(long, value_name = "FILE")]
		build_record: PathBuf,

		/// Upgrade-authority keypair. It also pays fees with solana-verify 0.5.1.
		#[arg(long, value_name = "KEYPAIR", required_unless_present = "export")]
		authority: Option<PathBuf>,

		/// Export without submitting. Optionally provide a public authority address.
		#[arg(
			long,
			num_args = 0..=1,
			default_missing_value = "",
			requires = "output",
			value_name = "AUTHORITY"
		)]
		export: Option<String>,

		/// Write only the validated encoded transaction payload to FILE.
		#[arg(long, value_name = "FILE", requires = "export")]
		output: Option<PathBuf>,

		/// Encoding for the exported transaction: base64 or base58.
		#[arg(
			long,
			value_enum,
			value_name = "ENCODING",
			requires = "export",
			hide_possible_values = true
		)]
		export_encoding: Option<ExportEncodingArg>,

		/// Confirm recording without an interactive prompt. Has no effect on export.
		#[arg(long, conflicts_with = "export")]
		yes: bool,

		/// Acknowledge submission to mainnet or an unknown remote endpoint.
		#[arg(long)]
		acknowledge_mainnet: bool,
	},

	/// Submit an existing on-chain verification record to the mainnet remote verifier.
	#[command(
		after_help = "Example:\n  pina verify submit --program-id <ADDRESS> --uploader \
		              <ADDRESS>\n\n\\
		              This command always targets the official mainnet remote verifier. UPLOADER is the \
		              \\
		              public address that created the on-chain verification record; it is not a keypair."
	)]
	Submit {
		/// Program address whose recorded build should be verified.
		#[arg(long, value_name = "ADDRESS")]
		program_id: String,

		/// Public address that uploaded the verification record.
		#[arg(long, value_name = "ADDRESS")]
		uploader: String,
	},

	/// Read the mainnet remote-verifier status for a program.
	#[command(
		after_help = "Example:\n  pina verify status --program-id <ADDRESS>\n\nThis command is \\
		              read-only and always queries the official mainnet remote verifier."
	)]
	Status {
		/// Program address to inspect.
		#[arg(long, value_name = "ADDRESS")]
		program_id: String,
	},
}

/// Transaction encoding supported by solana-verify exports.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ExportEncodingArg {
	Base64,
	Base58,
}

/// Named upstream clusters supported by Surfpool.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum SurfpoolCluster {
	Mainnet,
	Devnet,
	Testnet,
}

impl SurfpoolCluster {
	pub(crate) const fn as_str(self) -> &'static str {
		match self {
			Self::Mainnet => "mainnet",
			Self::Devnet => "devnet",
			Self::Testnet => "testnet",
		}
	}
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
