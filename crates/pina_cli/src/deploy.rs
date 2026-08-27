//! Safe planning and execution for `pina deploy`.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use ed25519_dalek::SigningKey;
use serde::Serialize;
use serde::Serializer;
use sha2::Digest as _;
use sha2::Sha256;
use thiserror::Error;
use url::Host;
use url::Url;

use crate::project::Project;

const LOCALNET_URL: &str = "http://127.0.0.1:8899";
const DEVNET_URL: &str = "https://api.devnet.solana.com";
const TESTNET_URL: &str = "https://api.testnet.solana.com";
const MAINNET_URL: &str = "https://api.mainnet-beta.solana.com";
const MAX_KEYPAIR_FILE_BYTES: u64 = 4 * 1024;

/// A named Solana cluster or an explicit RPC endpoint.
#[derive(Clone, Eq, PartialEq)]
pub enum DeploymentTarget {
	/// A well-known Solana cluster.
	Cluster(Cluster),
	/// A caller-supplied RPC endpoint.
	RpcUrl(String),
}

impl fmt::Debug for DeploymentTarget {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Cluster(cluster) => formatter.debug_tuple("Cluster").field(cluster).finish(),
			Self::RpcUrl(url) => {
				formatter
					.debug_tuple("RpcUrl")
					.field(&redact_rpc_for_diagnostic(url))
					.finish()
			}
		}
	}
}

impl DeploymentTarget {
	/// Parse the value accepted by `pina deploy --cluster`.
	#[must_use]
	pub fn from_cluster_arg(value: &str) -> Self {
		match value {
			"localnet" => Self::Cluster(Cluster::Localnet),
			"devnet" => Self::Cluster(Cluster::Devnet),
			"testnet" => Self::Cluster(Cluster::Testnet),
			"mainnet-beta" => Self::Cluster(Cluster::MainnetBeta),
			url => Self::RpcUrl(url.to_owned()),
		}
	}
}

/// Well-known Solana deployment clusters.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cluster {
	/// A validator running on the local machine.
	Localnet,
	/// Solana devnet.
	Devnet,
	/// Solana testnet.
	Testnet,
	/// Solana mainnet beta.
	MainnetBeta,
}

impl Cluster {
	fn name(self) -> &'static str {
		match self {
			Self::Localnet => "localnet",
			Self::Devnet => "devnet",
			Self::Testnet => "testnet",
			Self::MainnetBeta => "mainnet-beta",
		}
	}

	fn rpc_url(self) -> &'static str {
		match self {
			Self::Localnet => LOCALNET_URL,
			Self::Devnet => DEVNET_URL,
			Self::Testnet => TESTNET_URL,
			Self::MainnetBeta => MAINNET_URL,
		}
	}
}

/// Inputs used to resolve a deployment plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentRequest {
	/// Project directory or a directory below it.
	pub project: PathBuf,
	/// Explicit existing SBF artifact, if conventional discovery is not desired.
	pub program: Option<PathBuf>,
	/// Explicit program keypair, if the conventional keypair is not desired.
	pub program_keypair: Option<PathBuf>,
	/// Required upgrade-authority keypair.
	pub upgrade_authority: PathBuf,
	/// Required deployment fee-payer keypair.
	pub payer: PathBuf,
	/// Explicit cluster or RPC target.
	pub target: DeploymentTarget,
}

/// A command that will be executed as part of a deployment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandPlan {
	/// Executable selected by Pina.
	pub program: String,
	/// Arguments passed directly to the executable without a shell.
	pub args: Vec<String>,
}

/// Complete, immutable deployment plan.
#[derive(Clone, Eq, PartialEq)]
pub struct DeploymentPlan {
	project_root: String,
	program_dir: String,
	library_name: String,
	program: String,
	program_keypair: String,
	upgrade_authority: String,
	payer: String,
	program_id: String,
	target: ResolvedTarget,
	input_fingerprint: InputFingerprint,
}

#[derive(Clone, Eq, PartialEq)]
struct InputFingerprint {
	program: [u8; 32],
	program_keypair: [u8; 32],
	upgrade_authority: [u8; 32],
	payer: [u8; 32],
}

#[derive(Serialize)]
struct SerializableDeploymentPlan<'a> {
	project_root: &'a str,
	program: &'a str,
	program_keypair: &'a str,
	upgrade_authority: &'a str,
	payer: &'a str,
	program_id: &'a str,
	cluster: &'a str,
	rpc_url: &'a str,
	is_local: bool,
	requires_mainnet_acknowledgement: bool,
	commands: Vec<CommandPlan>,
}

impl Serialize for DeploymentPlan {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		self.serializable().serialize(serializer)
	}
}

impl fmt::Debug for DeploymentPlan {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("DeploymentPlan")
			.field("project_root", &self.project_root())
			.field("program", &self.program())
			.field("program_keypair", &self.program_keypair())
			.field("upgrade_authority", &self.upgrade_authority())
			.field("payer", &self.payer())
			.field("program_id", &self.program_id())
			.field("cluster", &self.cluster())
			.field("rpc_url", &self.rpc_url())
			.field("is_local", &self.is_local())
			.field(
				"requires_mainnet_acknowledgement",
				&self.requires_mainnet_acknowledgement(),
			)
			.field("commands", &self.commands())
			.finish_non_exhaustive()
	}
}

impl DeploymentPlan {
	/// Canonical project root used for discovery and command execution.
	#[must_use]
	pub fn project_root(&self) -> &str {
		&self.project_root
	}

	/// SBF artifact that will be deployed.
	#[must_use]
	pub fn program(&self) -> &str {
		&self.program
	}

	/// Keypair defining the program address.
	#[must_use]
	pub fn program_keypair(&self) -> &str {
		&self.program_keypair
	}

	/// Keypair authorized to upgrade the program.
	#[must_use]
	pub fn upgrade_authority(&self) -> &str {
		&self.upgrade_authority
	}

	/// Keypair that pays deployment transaction fees.
	#[must_use]
	pub fn payer(&self) -> &str {
		&self.payer
	}

	/// Program address declared by the source and verified against the program keypair.
	#[must_use]
	pub fn program_id(&self) -> &str {
		&self.program_id
	}

	/// Named cluster, or `custom` for an explicit URL.
	#[must_use]
	pub fn cluster(&self) -> &str {
		&self.target.cluster
	}

	/// Exact operator-supplied RPC URL exposed in the plan and Solana CLI arguments.
	#[must_use]
	pub fn rpc_url(&self) -> &str {
		&self.target.rpc_url
	}

	/// Whether the endpoint is provably local.
	#[must_use]
	pub fn is_local(&self) -> bool {
		self.target.policy == TargetPolicy::Local
	}

	/// Whether execution requires the explicit mainnet-risk acknowledgement.
	#[must_use]
	pub fn requires_mainnet_acknowledgement(&self) -> bool {
		self.target.policy == TargetPolicy::MainnetOrUnknown
	}

	/// Exact modeled command derived from the validated plan state.
	#[must_use]
	pub fn commands(&self) -> Vec<CommandPlan> {
		vec![deploy_command(
			self.program(),
			self.program_keypair(),
			self.upgrade_authority(),
			self.payer(),
			self.rpc_url(),
		)]
	}

	fn serializable(&self) -> SerializableDeploymentPlan<'_> {
		SerializableDeploymentPlan {
			project_root: self.project_root(),
			program: self.program(),
			program_keypair: self.program_keypair(),
			upgrade_authority: self.upgrade_authority(),
			payer: self.payer(),
			program_id: self.program_id(),
			cluster: self.cluster(),
			rpc_url: self.rpc_url(),
			is_local: self.is_local(),
			requires_mainnet_acknowledgement: self.requires_mainnet_acknowledgement(),
			commands: self.commands(),
		}
	}

	fn revalidate(&self) -> Result<(), DeployError> {
		let program = Path::new(self.program());
		let program_keypair = Path::new(self.program_keypair());
		let upgrade_authority = Path::new(self.upgrade_authority());
		let payer = Path::new(self.payer());
		canonical_file(program, "program", None)?;
		canonical_file(program_keypair, "program keypair", Some("program keypair"))?;
		canonical_file(
			upgrade_authority,
			"upgrade authority",
			Some("upgrade authority"),
		)?;
		canonical_file(payer, "fee payer", Some("fee payer"))?;
		let current_program_id =
			crate::generate_idl(Path::new(&self.program_dir), Some(&self.library_name))
				.map_err(|error| {
					DeployError::Project {
						path: PathBuf::from(self.project_root()),
						reason: format!("could not revalidate the declared program ID: {error}"),
					}
				})?
				.program
				.public_key;
		let program_keypair_bytes = read_keypair(program_keypair, "program keypair")?;
		let keypair_program_id = bs58::encode(&program_keypair_bytes[32..]).into_string();
		let fingerprint = InputFingerprint {
			program: file_digest(program, "program")?,
			program_keypair: file_digest(program_keypair, "program keypair")?,
			upgrade_authority: file_digest(upgrade_authority, "upgrade authority")?,
			payer: file_digest(payer, "fee payer")?,
		};

		if current_program_id != self.program_id
			|| keypair_program_id != self.program_id
			|| fingerprint != self.input_fingerprint
		{
			return Err(DeployError::InputsChanged);
		}

		Ok(())
	}

	/// Render a stable human-readable deployment plan.
	#[must_use]
	pub fn render_text(&self) -> String {
		let mut output = String::new();
		output.push_str("Deployment plan\n");
		let _ = writeln!(
			output,
			"  Project:           {}",
			diagnostic_quote(self.project_root())
		);
		let _ = writeln!(
			output,
			"  Program:           {}",
			diagnostic_quote(self.program())
		);
		let _ = writeln!(
			output,
			"  Program keypair:   {}",
			diagnostic_quote(self.program_keypair())
		);
		let _ = writeln!(
			output,
			"  Upgrade authority: {}",
			diagnostic_quote(self.upgrade_authority())
		);
		let _ = writeln!(
			output,
			"  Fee payer:         {}",
			diagnostic_quote(self.payer())
		);
		let _ = writeln!(
			output,
			"  Program ID:        {}",
			diagnostic_quote(self.program_id())
		);
		let _ = writeln!(
			output,
			"  Cluster:           {}",
			diagnostic_quote(self.cluster())
		);
		let _ = writeln!(
			output,
			"  RPC URL:           {}",
			diagnostic_quote(self.rpc_url())
		);
		output.push_str("  Commands:\n");

		for command in self.commands() {
			output.push_str("    ");
			output.push_str(&diagnostic_quote(&command.program));

			for arg in command.args {
				output.push(' ');
				output.push_str(&diagnostic_quote(&arg));
			}

			output.push('\n');
		}

		output
	}
}

/// Result of a child process invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandStatus {
	/// Whether the child exited successfully.
	pub success: bool,
	/// Child exit code, if one was available.
	pub code: Option<i32>,
}

/// Boundary used to execute planned commands without a shell.
pub trait CommandRunner {
	/// Run one executable with its exact argument vector.
	fn run(
		&mut self,
		program: &OsStr,
		args: &[OsString],
		current_dir: &Path,
	) -> io::Result<CommandStatus>;
}

/// Boundary used to request consequential remote-deployment confirmation.
pub trait DeploymentConfirmer {
	/// Return `true` only when the operator explicitly confirms the prompt.
	fn confirm(&mut self, prompt: &str) -> io::Result<bool>;
}

/// Read the exact interactive confirmation word from a terminal stream.
pub fn read_deployment_confirmation(
	prompt: &str,
	is_terminal: bool,
	input: &mut impl BufRead,
	output: &mut impl Write,
) -> io::Result<bool> {
	if !is_terminal {
		return Ok(false);
	}

	write!(output, "{prompt}")?;
	output.flush()?;
	let mut response = String::new();
	input.read_line(&mut response)?;

	Ok(response.trim() == "deploy")
}

/// Real child-process implementation used by the CLI.
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
	fn run(
		&mut self,
		program: &OsStr,
		args: &[OsString],
		current_dir: &Path,
	) -> io::Result<CommandStatus> {
		let status = Command::new(program)
			.args(args)
			.current_dir(current_dir)
			.stdin(Stdio::null())
			.status()?;

		Ok(CommandStatus {
			success: status.success(),
			code: status.code(),
		})
	}
}

/// Deployment planning or execution failure.
#[derive(Debug, Error)]
pub enum DeployError {
	/// Project discovery did not find a usable Cargo program crate.
	#[error("could not resolve a Pina program project from {path:?}: {reason:?}")]
	Project { path: PathBuf, reason: String },

	/// A required path was absent or did not identify a regular file.
	#[error("{kind} is not a readable regular file: {path:?}")]
	InvalidFile { kind: &'static str, path: PathBuf },

	/// A keypair file was not a valid Solana keypair JSON array.
	#[error("{kind} is not a valid 64-byte Solana keypair JSON file: {path:?}")]
	InvalidKeypair { kind: &'static str, path: PathBuf },

	/// A Unix keypair file is readable by its group or other users.
	#[cfg(unix)]
	#[error(
		"{kind} keypair has unsafe Unix permissions {mode:#o}: {path:?}; remove all group and \
		 other permissions (for example, chmod 600)"
	)]
	InsecureKeypairPermissions {
		kind: &'static str,
		path: PathBuf,
		mode: u32,
	},

	/// The validated program keypair does not match the source declaration.
	#[error("declared program ID {declared:?} does not match program keypair address {keypair:?}")]
	ProgramIdMismatch { declared: String, keypair: String },

	/// An explicit RPC URL was invalid or unsafe to pass through.
	#[error("invalid RPC URL {url:?}: {reason}")]
	InvalidRpcUrl { url: String, reason: String },

	/// A path cannot be represented in the machine-readable plan.
	#[error("{kind} path is not valid UTF-8: {path:?}")]
	NonUtf8Path { kind: &'static str, path: PathBuf },

	/// Mainnet execution was not explicitly acknowledged.
	#[error(
		"mainnet and custom remote deployment require --allow-mainnet in addition to confirmation \
		 or --yes"
	)]
	MainnetNotAllowed,

	/// Mainnet acknowledgement was supplied for another target.
	#[error("--allow-mainnet is valid only for mainnet-beta or a custom remote endpoint")]
	UnexpectedMainnetAcknowledgement,

	/// A validated input changed after the plan was displayed.
	#[error("deployment inputs changed after planning; review a new plan before deploying")]
	InputsChanged,

	/// A consequential remote deployment was rejected or could not be confirmed.
	#[error(
		"remote deployment was not confirmed; rerun interactively or pass --yes after reviewing \
		 the plan"
	)]
	ConfirmationRequired,

	/// A child process could not be started.
	#[error("failed to start {program:?}: {source}")]
	CommandStart {
		program: String,
		#[source]
		source: io::Error,
	},

	/// A child process exited unsuccessfully.
	#[error("{program:?} exited unsuccessfully with status {status}")]
	CommandFailed { program: String, status: String },

	/// The project-aware SBF build failed before deployment planning.
	#[error("deployment build failed: {0}")]
	Build(#[from] crate::build::BuildError),
}

/// Resolve and validate an inspectable deployment plan without executing it.
pub fn prepare_deployment(request: &DeploymentRequest) -> Result<DeploymentPlan, DeployError> {
	let project = Project::discover(&request.project).map_err(|error| {
		DeployError::Project {
			path: request.project.clone(),
			reason: error.to_string(),
		}
	})?;
	let deploy_dir = project.target_dir.join("deploy");
	let program_path = request.program.as_deref().map_or_else(
		|| {
			canonical_file(
				&deploy_dir.join(format!("{}.so", project.library_name)),
				"program",
				None,
			)
		},
		|path| canonical_file(path, "program", None),
	)?;
	let program_keypair_path = request.program_keypair.as_deref().map_or_else(
		|| {
			canonical_file(
				&deploy_dir.join(format!("{}-keypair.json", project.library_name)),
				"program keypair",
				Some("program keypair"),
			)
		},
		|path| canonical_file(path, "program keypair", Some("program keypair")),
	)?;
	let upgrade_authority_path = canonical_file(
		&request.upgrade_authority,
		"upgrade authority",
		Some("upgrade authority"),
	)?;
	let payer_path = canonical_file(&request.payer, "fee payer", Some("fee payer"))?;
	let declared_program_id =
		crate::generate_idl(&project.program_dir, Some(&project.library_name))
			.map_err(|error| {
				DeployError::Project {
					path: project.root.clone(),
					reason: format!("could not resolve the declared program ID: {error}"),
				}
			})?
			.program
			.public_key;
	let program_keypair_bytes = read_keypair(&program_keypair_path, "program keypair")?;
	let keypair_program_id = bs58::encode(&program_keypair_bytes[32..]).into_string();

	if declared_program_id != keypair_program_id {
		return Err(DeployError::ProgramIdMismatch {
			declared: declared_program_id,
			keypair: keypair_program_id,
		});
	}

	let target = ResolvedTarget::resolve(&request.target)?;
	let program = path_string(&program_path, "program")?;
	let program_keypair = path_string(&program_keypair_path, "program keypair")?;
	let upgrade_authority = path_string(&upgrade_authority_path, "upgrade authority")?;
	let payer = path_string(&payer_path, "fee payer")?;
	let project_root = path_string(&project.root, "project root")?;
	let program_dir = path_string(&project.program_dir, "program directory")?;
	let input_fingerprint = InputFingerprint {
		program: file_digest(&program_path, "program")?,
		program_keypair: file_digest(&program_keypair_path, "program keypair")?,
		upgrade_authority: file_digest(&upgrade_authority_path, "upgrade authority")?,
		payer: file_digest(&payer_path, "fee payer")?,
	};

	Ok(DeploymentPlan {
		project_root,
		program_dir,
		library_name: project.library_name,
		program,
		program_keypair,
		upgrade_authority,
		payer,
		program_id: declared_program_id,
		target,
		input_fingerprint,
	})
}

/// Validate an explicit deployment target before project discovery, building, or execution.
pub fn validate_deployment_target(target: &DeploymentTarget) -> Result<(), DeployError> {
	ResolvedTarget::resolve(target).map(|_| ())
}

/// Build the project before resolving and displaying its final deployment plan.
pub fn build_deployment_program(project: &Path) -> Result<(), DeployError> {
	build_deployment_program_with(project, crate::build::build_project)
}

fn build_deployment_program_with(
	project: &Path,
	build: impl FnOnce(&Path) -> Result<crate::build::BuildOutput, crate::build::BuildError>,
) -> Result<(), DeployError> {
	build(project).map(|_| ()).map_err(DeployError::Build)
}

fn deploy_command(
	program: &str,
	program_keypair: &str,
	upgrade_authority: &str,
	payer: &str,
	rpc_url: &str,
) -> CommandPlan {
	CommandPlan {
		program: "solana".to_owned(),
		args: vec![
			"program".to_owned(),
			"deploy".to_owned(),
			program.to_owned(),
			"--program-id".to_owned(),
			program_keypair.to_owned(),
			"--upgrade-authority".to_owned(),
			upgrade_authority.to_owned(),
			"--fee-payer".to_owned(),
			payer.to_owned(),
			"--url".to_owned(),
			rpc_url.to_owned(),
		],
	}
}

/// Execute an already-reviewed deployment plan.
pub fn execute_deployment(
	plan: &DeploymentPlan,
	yes: bool,
	allow_mainnet: bool,
	runner: &mut impl CommandRunner,
	confirmer: &mut impl DeploymentConfirmer,
) -> Result<(), DeployError> {
	if allow_mainnet && !plan.requires_mainnet_acknowledgement() {
		return Err(DeployError::UnexpectedMainnetAcknowledgement);
	}

	if plan.requires_mainnet_acknowledgement() && !allow_mainnet {
		return Err(DeployError::MainnetNotAllowed);
	}

	if !plan.is_local() && !yes {
		let prompt = format!(
			"Type `deploy` to deploy {} to {} ({}): ",
			diagnostic_quote(plan.program()),
			diagnostic_quote(plan.cluster()),
			diagnostic_quote(plan.rpc_url())
		);
		let approved = confirmer
			.confirm(&prompt)
			.map_err(|_| DeployError::ConfirmationRequired)?;

		if !approved {
			return Err(DeployError::ConfirmationRequired);
		}
	}

	plan.revalidate()?;
	let command = deploy_command(
		plan.program(),
		plan.program_keypair(),
		plan.upgrade_authority(),
		plan.payer(),
		plan.rpc_url(),
	);
	run_command(&command, Path::new(plan.project_root()), runner)
}

fn run_command(
	command: &CommandPlan,
	current_dir: &Path,
	runner: &mut impl CommandRunner,
) -> Result<(), DeployError> {
	let args = command.args.iter().map(OsString::from).collect::<Vec<_>>();
	let status = runner
		.run(OsStr::new(&command.program), &args, current_dir)
		.map_err(|source| {
			DeployError::CommandStart {
				program: command.program.clone(),
				source,
			}
		})?;

	if !status.success {
		return Err(DeployError::CommandFailed {
			program: command.program.clone(),
			status: status.code.map_or_else(
				|| "terminated by signal".to_owned(),
				|code| code.to_string(),
			),
		});
	}

	Ok(())
}

#[derive(Clone, Eq, PartialEq)]
struct ResolvedTarget {
	cluster: String,
	rpc_url: String,
	policy: TargetPolicy,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum TargetPolicy {
	Local,
	KnownRemote,
	MainnetOrUnknown,
}

impl ResolvedTarget {
	fn resolve(target: &DeploymentTarget) -> Result<Self, DeployError> {
		match target {
			DeploymentTarget::Cluster(cluster) => {
				Ok(Self {
					cluster: cluster.name().to_owned(),
					rpc_url: cluster.rpc_url().to_owned(),
					policy: match cluster {
						Cluster::Localnet => TargetPolicy::Local,
						Cluster::Devnet | Cluster::Testnet => TargetPolicy::KnownRemote,
						Cluster::MainnetBeta => TargetPolicy::MainnetOrUnknown,
					},
				})
			}
			DeploymentTarget::RpcUrl(value) => Self::custom(value),
		}
	}

	fn custom(value: &str) -> Result<Self, DeployError> {
		let diagnostic_url = redact_rpc_for_diagnostic(value);
		if value.chars().any(char::is_control) {
			return Err(DeployError::InvalidRpcUrl {
				url: diagnostic_url,
				reason: "control characters are not accepted".to_owned(),
			});
		}

		let parsed = Url::parse(value).map_err(|error| {
			DeployError::InvalidRpcUrl {
				url: diagnostic_url.clone(),
				reason: error.to_string(),
			}
		})?;

		if parsed.scheme() != "http" && parsed.scheme() != "https" {
			return Err(DeployError::InvalidRpcUrl {
				url: diagnostic_url.clone(),
				reason: "only http and https endpoints are accepted".to_owned(),
			});
		}

		if !parsed.username().is_empty() || parsed.password().is_some() {
			return Err(DeployError::InvalidRpcUrl {
				url: diagnostic_url.clone(),
				reason: "embedded credentials are not accepted".to_owned(),
			});
		}

		let host = require_rpc_host(parsed.host(), &diagnostic_url)?;

		if parsed.fragment().is_some() {
			return Err(DeployError::InvalidRpcUrl {
				url: diagnostic_url,
				reason: "URL fragments are not accepted".to_owned(),
			});
		}

		if parsed.query().is_some() {
			return Err(DeployError::InvalidRpcUrl {
				url: diagnostic_url,
				reason: "query parameters are rejected because RPC URLs are visible in process \
				         arguments"
					.to_owned(),
			});
		}

		let is_local = match host {
			Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
			Host::Ipv4(address) => address.is_loopback(),
			Host::Ipv6(address) => address.is_loopback(),
		};

		let rpc_url = parsed.to_string();

		Ok(Self {
			cluster: "custom".to_owned(),
			rpc_url,
			policy: if is_local {
				TargetPolicy::Local
			} else {
				TargetPolicy::MainnetOrUnknown
			},
		})
	}
}

fn require_rpc_host<'a>(
	host: Option<Host<&'a str>>,
	diagnostic_url: &str,
) -> Result<Host<&'a str>, DeployError> {
	host.ok_or_else(|| {
		DeployError::InvalidRpcUrl {
			url: diagnostic_url.to_owned(),
			reason: "an endpoint host is required".to_owned(),
		}
	})
}

fn canonical_file(
	path: &Path,
	kind: &'static str,
	keypair_kind: Option<&'static str>,
) -> Result<PathBuf, DeployError> {
	let canonical = fs::canonicalize(path).map_err(|_| {
		DeployError::InvalidFile {
			kind,
			path: path.to_path_buf(),
		}
	})?;
	if !canonical.is_file() {
		return Err(DeployError::InvalidFile {
			kind,
			path: path.to_path_buf(),
		});
	}

	if let Some(keypair_kind) = keypair_kind {
		validate_keypair(&canonical, keypair_kind)?;
	}

	Ok(canonical)
}

fn validate_keypair(path: &Path, kind: &'static str) -> Result<(), DeployError> {
	read_keypair(path, kind).map(|_| ())
}

fn file_digest(path: &Path, kind: &'static str) -> Result<[u8; 32], DeployError> {
	let file = fs::File::open(path).map_err(|_| {
		DeployError::InvalidFile {
			kind,
			path: path.to_path_buf(),
		}
	})?;
	digest_reader(file).map_err(|_| {
		DeployError::InvalidFile {
			kind,
			path: path.to_path_buf(),
		}
	})
}

fn digest_reader(mut reader: impl Read) -> io::Result<[u8; 32]> {
	let mut digest = Sha256::new();
	let mut buffer = [0_u8; 8 * 1024];

	loop {
		let read = reader.read(&mut buffer)?;
		if read == 0 {
			break;
		}
		digest.update(&buffer[..read]);
	}

	Ok(digest.finalize().into())
}

fn read_keypair(path: &Path, kind: &'static str) -> Result<Vec<u8>, DeployError> {
	let file = fs::File::open(path).map_err(|_| {
		DeployError::InvalidKeypair {
			kind,
			path: path.to_path_buf(),
		}
	})?;
	let metadata = keypair_metadata(file.metadata(), path, kind)?;

	if !metadata.is_file() || metadata.len() > MAX_KEYPAIR_FILE_BYTES {
		return Err(DeployError::InvalidKeypair {
			kind,
			path: path.to_path_buf(),
		});
	}

	#[cfg(unix)]
	validate_keypair_permissions(path, kind, &metadata)?;

	let bytes = read_keypair_content(file, path, kind)?;
	let keypair = serde_json::from_slice::<Vec<u8>>(&bytes).map_err(|_| {
		DeployError::InvalidKeypair {
			kind,
			path: path.to_path_buf(),
		}
	})?;

	if keypair.len() != 64 {
		return Err(DeployError::InvalidKeypair {
			kind,
			path: path.to_path_buf(),
		});
	}
	let mut secret = [0_u8; 32];
	secret.copy_from_slice(&keypair[..32]);
	let public = SigningKey::from_bytes(&secret).verifying_key().to_bytes();

	if keypair[32..] != public {
		return Err(DeployError::InvalidKeypair {
			kind,
			path: path.to_path_buf(),
		});
	}

	Ok(keypair)
}

fn keypair_metadata(
	metadata: io::Result<fs::Metadata>,
	path: &Path,
	kind: &'static str,
) -> Result<fs::Metadata, DeployError> {
	metadata.map_err(|_| {
		DeployError::InvalidKeypair {
			kind,
			path: path.to_path_buf(),
		}
	})
}

fn read_keypair_content(
	mut reader: impl Read,
	path: &Path,
	kind: &'static str,
) -> Result<Vec<u8>, DeployError> {
	let mut bytes = Vec::with_capacity(MAX_KEYPAIR_FILE_BYTES as usize);
	reader
		.by_ref()
		.take(MAX_KEYPAIR_FILE_BYTES + 1)
		.read_to_end(&mut bytes)
		.map_err(|_| {
			DeployError::InvalidKeypair {
				kind,
				path: path.to_path_buf(),
			}
		})?;

	if bytes.len() as u64 > MAX_KEYPAIR_FILE_BYTES {
		return Err(DeployError::InvalidKeypair {
			kind,
			path: path.to_path_buf(),
		});
	}

	Ok(bytes)
}

#[cfg(unix)]
fn validate_keypair_permissions(
	path: &Path,
	kind: &'static str,
	metadata: &fs::Metadata,
) -> Result<(), DeployError> {
	use std::os::unix::fs::PermissionsExt as _;

	let mode = metadata.permissions().mode() & 0o777;
	if mode & 0o077 != 0 {
		return Err(DeployError::InsecureKeypairPermissions {
			kind,
			path: path.to_path_buf(),
			mode,
		});
	}

	Ok(())
}

fn redact_rpc_for_diagnostic(value: &str) -> String {
	let without_userinfo = if let Some(scheme_end) = value.find("://") {
		let authority_start = scheme_end + 3;
		let authority_end = value[authority_start..]
			.find(['/', '?', '#'])
			.map_or(value.len(), |offset| authority_start + offset);
		let authority = &value[authority_start..authority_end];

		if let Some(at) = authority.rfind('@') {
			format!(
				"{}<redacted>@{}{}",
				&value[..authority_start],
				&authority[at + 1..],
				&value[authority_end..]
			)
		} else {
			value.to_owned()
		}
	} else {
		value.to_owned()
	};
	let query_start = without_userinfo.find('?');
	let fragment_start = without_userinfo.find('#');

	match (query_start, fragment_start) {
		(Some(query), Some(fragment)) if query < fragment => {
			format!("{}?<redacted>#<redacted>", &without_userinfo[..query])
		}
		(Some(query), _) => format!("{}?<redacted>", &without_userinfo[..query]),
		(None, Some(fragment)) => format!("{}#<redacted>", &without_userinfo[..fragment]),
		(None, None) => without_userinfo,
	}
}

fn diagnostic_quote(value: &str) -> String {
	format!("{value:?}")
}

fn path_string(path: &Path, kind: &'static str) -> Result<String, DeployError> {
	path.to_str().map(str::to_owned).ok_or_else(|| {
		DeployError::NonUtf8Path {
			kind,
			path: path.to_path_buf(),
		}
	})
}

#[cfg(test)]
mod tests {
	use std::collections::VecDeque;
	use std::fs;
	use std::io;
	use std::path::Path;

	use tempfile::TempDir;

	use super::*;

	struct Fixture {
		_temp: TempDir,
		root: PathBuf,
		program: PathBuf,
		program_keypair: PathBuf,
		authority: PathBuf,
		payer: PathBuf,
	}

	impl Fixture {
		fn new() -> Self {
			let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
			let root = temp.path().join("project");
			let deploy = root.join("target/deploy");
			let program = deploy.join("example_program.so");
			let program_keypair = deploy.join("example_program-keypair.json");
			let authority = root.join("authority.json");
			let payer = root.join("payer.json");
			fs::create_dir_all(&deploy).unwrap_or_else(|error| panic!("create fixture: {error}"));
			fs::create_dir_all(root.join("src"))
				.unwrap_or_else(|error| panic!("create source fixture: {error}"));
			fs::write(
				root.join("Cargo.toml"),
				"[package]\nname = \"example-program\"\nversion = \"0.1.0\"\n",
			)
			.unwrap_or_else(|error| panic!("write manifest: {error}"));
			fs::write(&program, b"SBF").unwrap_or_else(|error| panic!("write program: {error}"));
			write_keypair(&program_keypair);
			write_keypair(&authority);
			write_keypair(&payer);
			let keypair = read_keypair(&program_keypair, "fixture")
				.unwrap_or_else(|error| panic!("read fixture keypair: {error}"));
			let program_id = bs58::encode(&keypair[32..]).into_string();
			fs::write(
				root.join("src/lib.rs"),
				format!("use pina::prelude::*;\ndeclare_id!(\"{program_id}\");\n"),
			)
			.unwrap_or_else(|error| panic!("write source: {error}"));

			Self {
				_temp: temp,
				root,
				program,
				program_keypair,
				authority,
				payer,
			}
		}

		fn request(&self, target: DeploymentTarget) -> DeploymentRequest {
			DeploymentRequest {
				project: self.root.clone(),
				program: None,
				program_keypair: None,
				upgrade_authority: self.authority.clone(),
				payer: self.payer.clone(),
				target,
			}
		}
	}

	fn write_keypair(path: &Path) {
		let signing_key = SigningKey::from_bytes(&[7u8; 32]);
		let mut bytes = signing_key.to_bytes().to_vec();
		bytes.extend_from_slice(&signing_key.verifying_key().to_bytes());
		fs::write(
			path,
			serde_json::to_vec(&bytes).unwrap_or_else(|error| panic!("serialize keypair: {error}")),
		)
		.unwrap_or_else(|error| panic!("write keypair: {error}"));
		make_keypair_private(path);
	}

	#[cfg(unix)]
	fn make_keypair_private(path: &Path) {
		use std::os::unix::fs::PermissionsExt as _;

		fs::set_permissions(path, fs::Permissions::from_mode(0o600))
			.unwrap_or_else(|error| panic!("protect keypair: {error}"));
	}

	#[cfg(not(unix))]
	fn make_keypair_private(_: &Path) {}

	#[derive(Default)]
	struct FakeRunner {
		calls: Vec<(String, Vec<String>, PathBuf)>,
		results: VecDeque<io::Result<CommandStatus>>,
	}

	struct FailingReader;

	impl Read for FailingReader {
		fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
			Err(io::Error::other("synthetic failure"))
		}
	}

	impl CommandRunner for FakeRunner {
		fn run(
			&mut self,
			program: &OsStr,
			args: &[OsString],
			current_dir: &Path,
		) -> io::Result<CommandStatus> {
			self.calls.push((
				program.to_string_lossy().into_owned(),
				args.iter()
					.map(|arg| arg.to_string_lossy().into_owned())
					.collect(),
				current_dir.to_path_buf(),
			));
			self.results.pop_front().unwrap_or(Ok(CommandStatus {
				success: true,
				code: Some(0),
			}))
		}
	}

	struct FakeConfirmer {
		confirmed: io::Result<bool>,
		prompts: Vec<String>,
	}

	impl DeploymentConfirmer for FakeConfirmer {
		fn confirm(&mut self, prompt: &str) -> io::Result<bool> {
			self.prompts.push(prompt.to_owned());
			match &self.confirmed {
				Ok(confirmed) => Ok(*confirmed),
				Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
			}
		}
	}

	fn rejecting_confirmer() -> FakeConfirmer {
		FakeConfirmer {
			confirmed: Ok(false),
			prompts: Vec::new(),
		}
	}

	#[test]
	fn cluster_argument_recognizes_named_targets_and_preserves_urls() {
		assert_eq!(
			DeploymentTarget::from_cluster_arg("localnet"),
			DeploymentTarget::Cluster(Cluster::Localnet)
		);
		assert_eq!(
			DeploymentTarget::from_cluster_arg("devnet"),
			DeploymentTarget::Cluster(Cluster::Devnet)
		);
		assert_eq!(
			DeploymentTarget::from_cluster_arg("testnet"),
			DeploymentTarget::Cluster(Cluster::Testnet)
		);
		assert_eq!(
			DeploymentTarget::from_cluster_arg("mainnet-beta"),
			DeploymentTarget::Cluster(Cluster::MainnetBeta)
		);
		assert_eq!(
			DeploymentTarget::from_cluster_arg("https://rpc.example.com"),
			DeploymentTarget::RpcUrl("https://rpc.example.com".to_owned())
		);
	}

	#[test]
	fn plan_resolves_conventional_project_inputs_and_exact_command() {
		let fixture = Fixture::new();
		let plan = prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Devnet)))
			.unwrap_or_else(|error| panic!("prepare deployment: {error}"));

		assert_eq!(plan.program(), canonical(&fixture.program));
		assert_eq!(plan.program_keypair(), canonical(&fixture.program_keypair));
		assert_eq!(plan.upgrade_authority(), canonical(&fixture.authority));
		assert_eq!(plan.payer(), canonical(&fixture.payer));
		assert_eq!(plan.cluster(), "devnet");
		assert_eq!(plan.rpc_url(), DEVNET_URL);
		assert!(!plan.is_local());
		assert!(!plan.requires_mainnet_acknowledgement());
		assert_eq!(plan.commands().len(), 1);
		assert_eq!(
			plan.commands()[0],
			CommandPlan {
				program: "solana".to_owned(),
				args: vec![
					"program".to_owned(),
					"deploy".to_owned(),
					canonical(&fixture.program),
					"--program-id".to_owned(),
					canonical(&fixture.program_keypair),
					"--upgrade-authority".to_owned(),
					canonical(&fixture.authority),
					"--fee-payer".to_owned(),
					canonical(&fixture.payer),
					"--url".to_owned(),
					DEVNET_URL.to_owned(),
				],
			}
		);
	}

	#[test]
	fn foundation_build_boundary_runs_before_plan_and_uses_library_name() {
		let fixture = Fixture::new();
		fs::write(
			fixture.root.join("Cargo.toml"),
			"[package]\nname = \"package-name\"\nversion = \"0.1.0\"\n\n[lib]\nname = \
			 \"program_lib\"\n",
		)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
		let lib_keypair = fixture.root.join("target/deploy/program_lib-keypair.json");
		let lib_program = fixture.root.join("target/deploy/program_lib.so");
		write_keypair(&lib_keypair);
		fs::write(&lib_program, b"built SBF")
			.unwrap_or_else(|error| panic!("write built program: {error}"));
		let request = fixture.request(DeploymentTarget::Cluster(Cluster::Localnet));
		let mut built_from = None;
		build_deployment_program_with(&fixture.root, |project| {
			built_from = Some(project.to_path_buf());
			Ok(crate::build::BuildOutput {
				package_name: "package-name".to_owned(),
				sbf_artifact: lib_program.clone(),
				idl: fixture.root.join("target/idl/program_lib.json"),
			})
		})
		.unwrap_or_else(|error| panic!("build deployment: {error}"));
		let plan = prepare_deployment(&request)
			.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let expected_program = fs::canonicalize(&lib_program)
			.unwrap_or_else(|error| panic!("canonicalize program: {error}"));

		assert_eq!(plan.program(), path(&expected_program));
		assert_eq!(plan.program_keypair(), canonical(&lib_keypair));
		assert_eq!(plan.commands().len(), 1);
		assert_eq!(built_from.as_deref(), Some(fixture.root.as_path()));
		assert_eq!(plan.commands()[0].program, "solana");
	}

	#[test]
	fn foundation_build_failures_stop_deployment_workflow() {
		let error = build_deployment_program_with(Path::new("project"), |_| {
			Err(crate::build::BuildError::MissingArtifact {
				path: PathBuf::from("missing.so"),
			})
		})
		.unwrap_err();

		assert!(matches!(error, DeployError::Build(_)));
		assert!(error.to_string().contains("missing.so"));
	}

	#[test]
	fn explicit_paths_override_conventional_paths() {
		let fixture = Fixture::new();
		let explicit_program = fixture.root.join("artifact.so");
		let explicit_keypair = fixture.root.join("program.json");
		fs::write(&explicit_program, b"SBF")
			.unwrap_or_else(|error| panic!("write program: {error}"));
		write_keypair(&explicit_keypair);
		let mut request = fixture.request(DeploymentTarget::Cluster(Cluster::Testnet));
		request.program = Some(explicit_program.clone());
		request.program_keypair = Some(explicit_keypair.clone());
		let plan = prepare_deployment(&request)
			.unwrap_or_else(|error| panic!("prepare deployment: {error}"));

		assert_eq!(plan.program(), canonical(&explicit_program));
		assert_eq!(plan.program_keypair(), canonical(&explicit_keypair));
		assert_eq!(plan.rpc_url(), TESTNET_URL);
	}

	#[test]
	fn configured_program_uses_foundation_metadata_target_directory() {
		let fixture = Fixture::new();
		let program_dir = fixture.root.join("programs/example");
		fs::create_dir_all(&program_dir)
			.unwrap_or_else(|error| panic!("create configured program: {error}"));
		fs::rename(
			fixture.root.join("Cargo.toml"),
			program_dir.join("Cargo.toml"),
		)
		.unwrap_or_else(|error| panic!("move manifest: {error}"));
		fs::rename(fixture.root.join("src"), program_dir.join("src"))
			.unwrap_or_else(|error| panic!("move source: {error}"));
		fs::write(
			fixture.root.join("Pina.toml"),
			"[project]\nprogram = \"programs/example\"\n",
		)
		.unwrap_or_else(|error| panic!("write Pina config: {error}"));
		let project = Project::discover(&fixture.root)
			.unwrap_or_else(|error| panic!("discover configured project: {error}"));
		let deploy_dir = project.target_dir.join("deploy");
		let program = deploy_dir.join("example_program.so");
		let program_keypair = deploy_dir.join("example_program-keypair.json");
		fs::create_dir_all(&deploy_dir)
			.unwrap_or_else(|error| panic!("create metadata deploy directory: {error}"));
		fs::write(&program, b"configured SBF")
			.unwrap_or_else(|error| panic!("write configured artifact: {error}"));
		write_keypair(&program_keypair);
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_or_else(|error| panic!("prepare configured deployment: {error}"));

		assert_eq!(plan.project_root(), canonical(&fixture.root));
		assert_eq!(plan.program(), canonical(&program));
		assert_eq!(plan.program_keypair(), canonical(&program_keypair));
	}

	#[test]
	fn custom_url_is_normalized_and_classified() {
		let fixture = Fixture::new();
		let local = prepare_deployment(
			&fixture.request(DeploymentTarget::RpcUrl("http://localhost:8899".to_owned())),
		)
		.unwrap_or_else(|error| panic!("prepare local deployment: {error}"));
		let remote = prepare_deployment(&fixture.request(DeploymentTarget::RpcUrl(
			"https://rpc.example.com/solana".to_owned(),
		)))
		.unwrap_or_else(|error| panic!("prepare remote deployment: {error}"));

		assert!(local.is_local());
		assert_eq!(local.cluster(), "custom");
		assert_eq!(local.rpc_url(), "http://localhost:8899/");
		assert!(!remote.is_local());
		assert!(remote.requires_mainnet_acknowledgement());
		assert_eq!(remote.rpc_url(), "https://rpc.example.com/solana");

		for loopback in ["http://127.0.0.1:8899", "http://[::1]:8899"] {
			let plan =
				prepare_deployment(&fixture.request(DeploymentTarget::RpcUrl(loopback.to_owned())))
					.unwrap_or_else(|error| panic!("prepare loopback deployment: {error}"));
			assert!(plan.is_local());
		}
	}

	#[test]
	fn accepted_custom_url_paths_remain_visible_in_plans_and_child_arguments() {
		let fixture = Fixture::new();
		let rpc_url = "https://rpc.example.com/operator-visible-path";
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::RpcUrl(rpc_url.to_owned())))
				.unwrap_or_else(|error| panic!("prepare remote deployment: {error}"));
		let json = serde_json::to_string(&plan)
			.unwrap_or_else(|error| panic!("serialize deployment plan: {error}"));
		let commands = plan.commands();

		assert_eq!(plan.rpc_url(), rpc_url);
		assert!(plan.render_text().contains(rpc_url));
		assert!(json.contains(rpc_url));
		assert!(commands[0].args.iter().any(|argument| argument == rpc_url));
	}

	#[test]
	fn custom_url_query_is_rejected_without_leaking_the_value() {
		let fixture = Fixture::new();
		let request = fixture.request(DeploymentTarget::RpcUrl(
			"https://rpc.example.com/?api-key=sentinel-secret".to_owned(),
		));
		let error = prepare_deployment(&request).unwrap_err();
		let request_debug = format!("{request:?}");
		let error_display = error.to_string();
		let error_debug = format!("{error:?}");

		for public_output in [&request_debug, &error_display, &error_debug] {
			assert!(!public_output.contains("sentinel-secret"));
		}
		assert!(error_display.contains("process arguments"));
	}

	#[test]
	fn custom_url_rejects_unsupported_schemes_and_embedded_credentials() {
		let fixture = Fixture::new();

		for url in [
			"not a URL",
			"file:///tmp/rpc",
			"https://user:secret@rpc.example.com",
			"https://rpc.example.com/#fragment",
			"https://rpc.example.com/\nstripped-by-url-parser",
		] {
			let error =
				prepare_deployment(&fixture.request(DeploymentTarget::RpcUrl(url.to_owned())))
					.unwrap_err();
			assert!(matches!(error, DeployError::InvalidRpcUrl { .. }));
		}

		let error = require_rpc_host(None, "https://<missing>").unwrap_err();
		assert!(matches!(error, DeployError::InvalidRpcUrl { .. }));
	}

	#[test]
	fn project_discovery_reports_missing_malformed_and_invalid_sources() {
		let fixture = Fixture::new();
		let nested = fixture.root.join("src");
		let mut nested_request = fixture.request(DeploymentTarget::Cluster(Cluster::Localnet));
		nested_request.project = nested;
		prepare_deployment(&nested_request)
			.unwrap_or_else(|error| panic!("discover ancestor project: {error}"));

		let missing = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let mut missing_request = fixture.request(DeploymentTarget::Cluster(Cluster::Localnet));
		missing_request.project = missing.path().to_path_buf();
		assert!(matches!(
			prepare_deployment(&missing_request),
			Err(DeployError::Project { .. })
		));

		fs::write(fixture.root.join("Cargo.toml"), "not valid TOML = [")
			.unwrap_or_else(|error| panic!("write malformed manifest: {error}"));
		assert!(matches!(
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet))),
			Err(DeployError::Project { .. })
		));

		fs::write(
			fixture.root.join("Cargo.toml"),
			"[package]\nname = \"example-program\"\nversion = \"0.1.0\"\n",
		)
		.unwrap_or_else(|error| panic!("restore manifest: {error}"));
		fs::write(
			fixture.root.join("src/lib.rs"),
			"pub fn missing_program_id() {}",
		)
		.unwrap_or_else(|error| panic!("write invalid source: {error}"));
		assert!(matches!(
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet))),
			Err(DeployError::Project { .. })
		));
	}

	#[test]
	fn project_discovery_rejects_nonexistent_start_and_directory_artifact() {
		let fixture = Fixture::new();
		let mut request = fixture.request(DeploymentTarget::Cluster(Cluster::Localnet));
		request.project = fixture.root.join("missing");
		assert!(matches!(
			prepare_deployment(&request),
			Err(DeployError::Project { .. })
		));

		fs::remove_file(&fixture.program)
			.unwrap_or_else(|error| panic!("remove artifact: {error}"));
		fs::create_dir(&fixture.program)
			.unwrap_or_else(|error| panic!("create artifact directory: {error}"));
		assert!(matches!(
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet))),
			Err(DeployError::InvalidFile { .. })
		));
	}

	#[test]
	fn invalid_rpc_error_redacts_credentials_query_and_fragment() {
		let fixture = Fixture::new();
		let error = prepare_deployment(&fixture.request(DeploymentTarget::RpcUrl(
			"https://user:sentinel-secret@rpc.example.com/?token=sentinel-secret#sentinel-secret"
				.to_owned(),
		)))
		.unwrap_err();
		let display = error.to_string();
		let debug = format!("{error:?}");

		assert!(!display.contains("sentinel-secret"));
		assert!(!debug.contains("sentinel-secret"));
		assert!(display.contains("<redacted>"));
	}

	#[test]
	fn missing_project_artifact_and_keys_fail_closed() {
		let fixture = Fixture::new();
		fs::remove_file(&fixture.program).unwrap_or_else(|error| panic!("remove program: {error}"));
		let error =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_err();
		assert!(matches!(
			error,
			DeployError::InvalidFile {
				kind: "program",
				..
			}
		));

		fs::write(&fixture.program, b"SBF")
			.unwrap_or_else(|error| panic!("restore program: {error}"));
		fs::remove_file(&fixture.program_keypair)
			.unwrap_or_else(|error| panic!("remove keypair: {error}"));
		let error =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_err();
		assert!(matches!(
			error,
			DeployError::InvalidFile {
				kind: "program keypair",
				..
			}
		));
	}

	#[test]
	fn malformed_keypairs_fail_closed() {
		let fixture = Fixture::new();
		assert!(matches!(
			read_keypair(&fixture.root.join("missing-keypair.json"), "missing"),
			Err(DeployError::InvalidKeypair { .. })
		));

		fs::write(&fixture.authority, "not JSON")
			.unwrap_or_else(|error| panic!("write invalid JSON keypair: {error}"));
		assert!(matches!(
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet))),
			Err(DeployError::InvalidKeypair { .. })
		));

		fs::write(&fixture.authority, "[1, 2, 3]")
			.unwrap_or_else(|error| panic!("write malformed keypair: {error}"));
		let error =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_err();

		assert!(matches!(
			error,
			DeployError::InvalidKeypair {
				kind: "upgrade authority",
				..
			}
		));

		let inconsistent = serde_json::to_vec(&vec![1u8; 64])
			.unwrap_or_else(|error| panic!("serialize inconsistent keypair: {error}"));
		fs::write(&fixture.authority, inconsistent)
			.unwrap_or_else(|error| panic!("write inconsistent keypair: {error}"));
		let error =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_err();
		assert!(matches!(error, DeployError::InvalidKeypair { .. }));
	}

	#[test]
	fn oversized_and_non_regular_keypairs_fail_before_json_parsing() {
		let fixture = Fixture::new();
		fs::write(
			&fixture.authority,
			vec![b' '; (MAX_KEYPAIR_FILE_BYTES + 1) as usize],
		)
		.unwrap_or_else(|error| panic!("write oversized keypair: {error}"));
		make_keypair_private(&fixture.authority);

		assert!(matches!(
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet))),
			Err(DeployError::InvalidKeypair {
				kind: "upgrade authority",
				..
			})
		));
		assert!(matches!(
			read_keypair(&fixture.root, "directory"),
			Err(DeployError::InvalidKeypair { .. })
		));
		assert!(matches!(
			keypair_metadata(
				Err(io::Error::other("synthetic metadata failure")),
				Path::new("keypair.json"),
				"synthetic",
			),
			Err(DeployError::InvalidKeypair { .. })
		));
		assert!(matches!(
			read_keypair_content(
				io::Cursor::new(vec![0_u8; (MAX_KEYPAIR_FILE_BYTES + 1) as usize]),
				Path::new("keypair.json"),
				"synthetic",
			),
			Err(DeployError::InvalidKeypair { .. })
		));
		assert!(matches!(
			read_keypair_content(FailingReader, Path::new("keypair.json"), "synthetic",),
			Err(DeployError::InvalidKeypair { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn group_or_world_accessible_keypairs_fail_closed() {
		use std::os::unix::fs::PermissionsExt as _;

		let fixture = Fixture::new();
		fs::set_permissions(&fixture.authority, fs::Permissions::from_mode(0o640))
			.unwrap_or_else(|error| panic!("weaken keypair permissions: {error}"));
		let error =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_err();

		assert!(matches!(
			error,
			DeployError::InsecureKeypairPermissions {
				kind: "upgrade authority",
				mode: 0o640,
				..
			}
		));
		assert!(error.to_string().contains("chmod 600"));
	}

	#[test]
	fn artifact_digest_streams_large_inputs_and_propagates_read_failures() {
		let bytes = vec![7_u8; 2 * 64 * 1024 + 17];
		let digest = digest_reader(io::Cursor::new(&bytes))
			.unwrap_or_else(|error| panic!("digest large input: {error}"));
		let expected: [u8; 32] = Sha256::digest(&bytes).into();

		assert_eq!(digest, expected);
		assert!(digest_reader(FailingReader).is_err());
		assert!(matches!(
			file_digest(Path::new("missing-artifact.so"), "program"),
			Err(DeployError::InvalidFile { .. })
		));
		let directory =
			tempfile::tempdir().unwrap_or_else(|error| panic!("create digest directory: {error}"));
		assert!(matches!(
			file_digest(directory.path(), "program"),
			Err(DeployError::InvalidFile { .. })
		));
	}

	#[test]
	fn program_keypair_must_match_the_declared_program_id() {
		let fixture = Fixture::new();
		let signing_key = SigningKey::from_bytes(&[9u8; 32]);
		let mut different = signing_key.to_bytes().to_vec();
		different.extend_from_slice(&signing_key.verifying_key().to_bytes());
		fs::write(
			&fixture.program_keypair,
			serde_json::to_vec(&different)
				.unwrap_or_else(|error| panic!("serialize keypair: {error}")),
		)
		.unwrap_or_else(|error| panic!("write different keypair: {error}"));
		let error =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_err();

		assert!(matches!(error, DeployError::ProgramIdMismatch { .. }));
	}

	#[test]
	fn virtual_workspace_is_rejected_as_ambiguous() {
		let fixture = Fixture::new();
		fs::write(
			fixture.root.join("Cargo.toml"),
			"[workspace]\nmembers = []\n",
		)
		.unwrap_or_else(|error| panic!("write workspace manifest: {error}"));
		let error =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_err();

		assert!(matches!(error, DeployError::Project { .. }));
	}

	#[test]
	fn local_execution_needs_no_confirmation() {
		let fixture = Fixture::new();
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let mut runner = FakeRunner::default();
		let mut confirmer = rejecting_confirmer();

		execute_deployment(&plan, false, false, &mut runner, &mut confirmer)
			.unwrap_or_else(|error| panic!("execute deployment: {error}"));

		assert_eq!(runner.calls.len(), 1);
		assert!(confirmer.prompts.is_empty());
	}

	#[test]
	fn remote_execution_requires_confirmation_unless_yes() {
		let fixture = Fixture::new();
		let plan = prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Devnet)))
			.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let mut runner = FakeRunner::default();
		let mut confirmer = rejecting_confirmer();
		let error =
			execute_deployment(&plan, false, false, &mut runner, &mut confirmer).unwrap_err();
		assert!(matches!(error, DeployError::ConfirmationRequired));
		assert!(runner.calls.is_empty());
		assert_eq!(confirmer.prompts.len(), 1);

		execute_deployment(&plan, true, false, &mut runner, &mut confirmer)
			.unwrap_or_else(|error| panic!("execute with yes: {error}"));
		assert_eq!(runner.calls.len(), 1);
	}

	#[test]
	fn unreadable_confirmation_fails_before_child_execution() {
		let fixture = Fixture::new();
		let plan = prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Devnet)))
			.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let mut runner = FakeRunner::default();
		let mut confirmer = FakeConfirmer {
			confirmed: Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed")),
			prompts: Vec::new(),
		};
		let error =
			execute_deployment(&plan, false, false, &mut runner, &mut confirmer).unwrap_err();

		assert!(matches!(error, DeployError::ConfirmationRequired));
		assert!(runner.calls.is_empty());
	}

	#[test]
	fn terminal_confirmation_requires_the_exact_word() {
		let mut output = Vec::new();
		assert!(!read_deployment_confirmation(
			"prompt",
			false,
			&mut "deploy\n".as_bytes(),
			&mut output,
		)
		.unwrap_or_else(|error| panic!("non-terminal confirmation: {error}")));
		assert!(output.is_empty());

		assert!(
			read_deployment_confirmation("prompt", true, &mut "deploy\n".as_bytes(), &mut output,)
				.unwrap_or_else(|error| panic!("terminal confirmation: {error}"))
		);
		assert_eq!(output, b"prompt");

		output.clear();
		assert!(
			!read_deployment_confirmation("prompt", true, &mut "yes\n".as_bytes(), &mut output,)
				.unwrap_or_else(|error| panic!("rejected confirmation: {error}"))
		);
	}

	#[test]
	fn positive_confirmation_allows_remote_execution() {
		let fixture = Fixture::new();
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Testnet)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let mut runner = FakeRunner::default();
		let mut confirmer = FakeConfirmer {
			confirmed: Ok(true),
			prompts: Vec::new(),
		};

		execute_deployment(&plan, false, false, &mut runner, &mut confirmer)
			.unwrap_or_else(|error| panic!("execute deployment: {error}"));
		assert_eq!(runner.calls.len(), 1);
	}

	#[test]
	fn mainnet_needs_separate_acknowledgement() {
		let fixture = Fixture::new();
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::MainnetBeta)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let mut runner = FakeRunner::default();
		let mut confirmer = rejecting_confirmer();

		let error =
			execute_deployment(&plan, true, false, &mut runner, &mut confirmer).unwrap_err();
		assert!(matches!(error, DeployError::MainnetNotAllowed));
		assert!(runner.calls.is_empty());

		execute_deployment(&plan, true, true, &mut runner, &mut confirmer)
			.unwrap_or_else(|error| panic!("execute acknowledged mainnet: {error}"));
		assert_eq!(runner.calls.len(), 1);

		let custom = prepare_deployment(&fixture.request(DeploymentTarget::RpcUrl(
			"https://rpc.example.com".to_owned(),
		)))
		.unwrap_or_else(|error| panic!("prepare custom remote: {error}"));
		let error =
			execute_deployment(&custom, true, false, &mut runner, &mut confirmer).unwrap_err();
		assert!(matches!(error, DeployError::MainnetNotAllowed));
		execute_deployment(&custom, true, true, &mut runner, &mut confirmer)
			.unwrap_or_else(|error| panic!("execute acknowledged custom remote: {error}"));
	}

	#[test]
	fn mainnet_acknowledgement_is_rejected_for_other_targets() {
		let fixture = Fixture::new();
		let plan = prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Devnet)))
			.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let mut runner = FakeRunner::default();
		let mut confirmer = rejecting_confirmer();
		let error = execute_deployment(&plan, true, true, &mut runner, &mut confirmer).unwrap_err();

		assert!(matches!(
			error,
			DeployError::UnexpectedMainnetAcknowledgement
		));
		assert!(runner.calls.is_empty());
	}

	#[test]
	fn child_start_and_failure_stop_the_plan() {
		let fixture = Fixture::new();
		let request = fixture.request(DeploymentTarget::Cluster(Cluster::Localnet));
		let plan = prepare_deployment(&request)
			.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let mut confirmer = rejecting_confirmer();
		let mut start_failure = FakeRunner {
			calls: Vec::new(),
			results: VecDeque::from([Err(io::Error::new(io::ErrorKind::NotFound, "missing"))]),
		};
		let error = execute_deployment(&plan, false, false, &mut start_failure, &mut confirmer)
			.unwrap_err();
		assert!(matches!(error, DeployError::CommandStart { .. }));
		assert_eq!(start_failure.calls.len(), 1);

		let mut child_failure = FakeRunner {
			calls: Vec::new(),
			results: VecDeque::from([Ok(CommandStatus {
				success: false,
				code: Some(7),
			})]),
		};
		let error = execute_deployment(&plan, false, false, &mut child_failure, &mut confirmer)
			.unwrap_err();
		assert!(matches!(error, DeployError::CommandFailed { .. }));
		assert_eq!(child_failure.calls.len(), 1);

		let mut signaled_child = FakeRunner {
			calls: Vec::new(),
			results: VecDeque::from([Ok(CommandStatus {
				success: false,
				code: None,
			})]),
		};
		let error = execute_deployment(&plan, false, false, &mut signaled_child, &mut confirmer)
			.unwrap_err();
		assert!(error.to_string().contains("terminated by signal"));
		assert_eq!(signaled_child.calls.len(), 1);
	}

	#[test]
	fn changed_inputs_are_rejected_immediately_before_deploy() {
		let fixture = Fixture::new();
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		fs::write(&fixture.program, b"changed SBF")
			.unwrap_or_else(|error| panic!("change program: {error}"));
		let mut runner = FakeRunner::default();
		let mut confirmer = rejecting_confirmer();
		let error =
			execute_deployment(&plan, false, false, &mut runner, &mut confirmer).unwrap_err();

		assert!(matches!(error, DeployError::InputsChanged));
		assert!(runner.calls.is_empty());

		let fixture = Fixture::new();
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		fs::remove_file(&fixture.authority)
			.unwrap_or_else(|error| panic!("remove authority: {error}"));
		let error =
			execute_deployment(&plan, false, false, &mut runner, &mut confirmer).unwrap_err();
		assert!(matches!(error, DeployError::InvalidFile { .. }));

		let fixture = Fixture::new();
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		fs::write(fixture.root.join("src/lib.rs"), "not valid Rust {")
			.unwrap_or_else(|error| panic!("change source: {error}"));
		let error =
			execute_deployment(&plan, false, false, &mut runner, &mut confirmer).unwrap_err();
		assert!(matches!(error, DeployError::Project { .. }));

		assert!(matches!(
			file_digest(&fixture.root.join("missing.so"), "program"),
			Err(DeployError::InvalidFile { .. })
		));
	}

	#[test]
	fn human_plan_lists_every_resolved_input() {
		let fixture = Fixture::new();
		let plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Localnet)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		let output = plan.render_text();

		for expected in [
			plan.project_root(),
			plan.program(),
			plan.program_keypair(),
			plan.upgrade_authority(),
			plan.payer(),
			plan.program_id(),
			plan.rpc_url(),
		] {
			assert!(output.contains(&diagnostic_quote(expected)));
		}
		assert!(format!("{:?}", DeploymentTarget::Cluster(Cluster::Localnet)).contains("Localnet"));
		assert!(format!("{plan:?}").contains("DeploymentPlan"));
	}

	#[test]
	fn diagnostic_quoting_escapes_control_characters() {
		assert_eq!(
			diagnostic_quote("line\nbreak\ttab"),
			r#""line\nbreak\ttab""#
		);

		let fixture = Fixture::new();
		let mut plan =
			prepare_deployment(&fixture.request(DeploymentTarget::Cluster(Cluster::Devnet)))
				.unwrap_or_else(|error| panic!("prepare deployment: {error}"));
		plan.program = "program\nwith-control.so".to_owned();
		let output = plan.render_text();
		let mut runner = FakeRunner::default();
		let mut confirmer = rejecting_confirmer();
		let error =
			execute_deployment(&plan, false, false, &mut runner, &mut confirmer).unwrap_err();

		assert!(matches!(error, DeployError::ConfirmationRequired));
		assert!(!output.contains("program\nwith-control"));
		assert!(output.contains(r#""program\nwith-control.so""#));
		assert!(!confirmer.prompts[0].contains("program\nwith-control"));
		assert!(confirmer.prompts[0].contains(r#""program\nwith-control.so""#));
	}

	#[test]
	fn diagnostic_redaction_handles_every_url_shape() {
		assert_eq!(redact_rpc_for_diagnostic("relative"), "relative");
		assert_eq!(
			redact_rpc_for_diagnostic("https://rpc.example.com/#sentinel"),
			"https://rpc.example.com/#<redacted>"
		);
		assert_eq!(
			redact_rpc_for_diagnostic("https://rpc.example.com/?key=sentinel#sentinel"),
			"https://rpc.example.com/?<redacted>#<redacted>"
		);
	}

	#[cfg(unix)]
	#[test]
	fn non_utf8_plan_path_fails_closed() {
		use std::ffi::OsString;
		use std::os::unix::ffi::OsStringExt;

		let non_utf8 = PathBuf::from(OsString::from_vec(vec![0xff]));
		assert!(matches!(
			path_string(&non_utf8, "program"),
			Err(DeployError::NonUtf8Path { .. })
		));
	}

	fn canonical(path: &Path) -> String {
		fs::canonicalize(path)
			.unwrap_or_else(|error| panic!("canonicalize {}: {error}", path.display()))
			.to_string_lossy()
			.into_owned()
	}

	fn path(path: &Path) -> String {
		path.to_string_lossy().into_owned()
	}
}
