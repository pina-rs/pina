//! `pina rehearse`: replay a program's real traffic against an upgrade before
//! shipping it.
//!
//! A rehearsal fetches the program's recent transactions (or the ones named
//! with `--signature`) from the cluster, read-only. It then starts a private
//! Surfpool forked from the same RPC endpoint and, on that one fork:
//!
//! 1. hydrates every account the transactions touch, and marks the ones that
//!    do not exist as offline, so the remote is never consulted again and both
//!    runs see one frozen snapshot;
//! 2. profiles each signed transaction against the deployed program. Profiling
//!    executes on a throwaway copy of the fork's state, so nothing commits;
//! 3. installs the candidate into the program's own program-data account, the
//!    way an upgrade would, and proves the runtime loaded it;
//! 4. profiles each transaction again and classifies the differences (see
//!    [`report`]).
//!
//! Surfpool hides a failed program load: it logs the error and keeps executing
//! the previously cached program. A rehearsal that trusted it could report a
//! broken candidate as "unchanged". Every program swap is therefore confirmed
//! with a sentinel: the program account is rewritten with one extra lamport,
//! which the runtime only stores when the program loads, and read back.

mod catalog;
pub mod report;
mod rpc;
mod surfnet;
mod wire;

use std::collections::BTreeSet;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine as _;
use serde_json::Value;
use serde_json::json;
use sha2::Digest as _;
use sha2::Sha256;

use self::catalog::ProgramCatalog;
pub use self::report::AccountChange;
use self::report::AccountImage;
pub use self::report::AccountSummary;
pub use self::report::ByteRange;
use self::report::Execution;
pub use self::report::FieldChange;
pub use self::report::InstructionRehearsal;
pub use self::report::InstructionUnits;
pub use self::report::LogExcerpt;
use self::report::ProgramInstruction;
pub use self::report::REPORT_SCHEMA_VERSION;
pub use self::report::RehearsalReport;
pub use self::report::RehearsalStatus;
pub use self::report::RehearsalSummary;
use self::report::ReportHeader;
pub use self::report::RunOutcome;
pub use self::report::SkipReason;
pub use self::report::SkippedTransaction;
pub use self::report::TransactionRehearsal;
pub use self::report::UnitStats;
use self::rpc::FetchedTransaction;
use self::rpc::JsonRpc;
use self::rpc::RpcError;
use self::surfnet::Surfnet;
use self::surfnet::SurfnetLaunch;
use self::surfnet::SurfnetPorts;
use self::wire::WireTransaction;
use crate::build::BuildError;
use crate::error::IdlError;
use crate::project::Project;
use crate::project::ProjectError;
use crate::workflow::SurfpoolNetwork;
use crate::workflow::WorkflowError;

/// Transactions rehearsed when `--limit` is not given.
pub const DEFAULT_LIMIT: usize = 25;
/// The most transactions one rehearsal fetches: a single
/// `getSignaturesForAddress` page, so no paging is ever needed.
pub const MAX_LIMIT: usize = 1000;
/// The oldest Surfpool whose flags and cheatcodes a rehearsal relies on.
pub const MINIMUM_SURFPOOL_VERSION: (u64, u64, u64) = (1, 6, 0);

const REMOTE_TIMEOUT: Duration = Duration::from_secs(30);
/// Profiling may fetch accounts from the remote, so local calls get longer.
const LOCAL_TIMEOUT: Duration = Duration::from_secs(120);
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Program bytes per `surfnet_writeProgram` call; hex encoding doubles them,
/// which stays well inside Surfpool's 5 MB request limit.
const WRITE_CHUNK_BYTES: usize = 1024 * 1024;
/// The `getMultipleAccounts` maximum.
const ACCOUNTS_PER_REQUEST: usize = 100;
/// Agave's `JSON_RPC_SERVER_ERROR_UNSUPPORTED_TRANSACTION_VERSION`: the one
/// `getTransaction` refusal that is a property of the transaction, returned
/// when it cannot be encoded in a version the request supports. Every other
/// JSON-RPC error, such as an unhealthy node (-32005) or a provider's rate
/// limit, says nothing about the transaction and aborts the rehearsal.
const UNSUPPORTED_TRANSACTION_VERSION: i64 = -32015;
const UPGRADEABLE_LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
/// `UpgradeableLoaderState::Program`: a `u32` tag of 2 and the program-data
/// address.
const PROGRAM_ACCOUNT_BYTES: usize = 36;
/// `UpgradeableLoaderState::ProgramData`: a `u32` tag of 3, the deployment
/// slot, and an optional upgrade authority that always reserves 32 bytes.
const PROGRAMDATA_HEADER_BYTES: usize = 45;

/// A named cluster to rehearse against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RehearseCluster {
	Mainnet,
	Devnet,
	Testnet,
}

impl RehearseCluster {
	const fn name(self) -> &'static str {
		match self {
			Self::Mainnet => "mainnet",
			Self::Devnet => "devnet",
			Self::Testnet => "testnet",
		}
	}

	const fn rpc_url(self) -> &'static str {
		match self {
			Self::Mainnet => "https://api.mainnet-beta.solana.com",
			Self::Devnet => "https://api.devnet.solana.com",
			Self::Testnet => "https://api.testnet.solana.com",
		}
	}
}

/// Where the deployed program and its traffic live.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RehearseNetwork {
	Cluster(RehearseCluster),
	/// A credential-free HTTP(S) RPC URL, validated like `pina dev --rpc-url`.
	RpcUrl(String),
}

impl RehearseNetwork {
	/// Build the network from the `--network` cluster name and `--rpc-url`
	/// flags. An RPC URL wins; exactly one of the two must be present.
	///
	/// # Errors
	///
	/// Returns an error when neither is given or the cluster name is unknown.
	pub fn from_flags(
		cluster: Option<&str>,
		rpc_url: Option<String>,
	) -> Result<Self, RehearseError> {
		if let Some(rpc_url) = rpc_url {
			return Ok(Self::RpcUrl(rpc_url));
		}

		let cluster = match cluster {
			Some("mainnet") => RehearseCluster::Mainnet,
			Some("devnet") => RehearseCluster::Devnet,
			Some("testnet") => RehearseCluster::Testnet,
			other => {
				return Err(RehearseError::UnknownNetwork {
					cluster: other.map(str::to_owned),
				});
			}
		};

		Ok(Self::Cluster(cluster))
	}
}

/// Inputs for [`rehearse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RehearseOptions {
	/// Directory in or below the project.
	pub project: PathBuf,
	pub network: RehearseNetwork,
	/// Candidate SBF artifact; defaults to the project's `target/deploy` build.
	pub program: Option<PathBuf>,
	/// Run the `pina build` build before rehearsing and use its artifact.
	pub build: bool,
	/// How many recent transactions to fetch, from 1 to [`MAX_LIMIT`].
	pub limit: usize,
	/// Rehearse exactly these transactions instead of the most recent ones.
	pub signatures: Vec<String>,
}

/// Why a rehearsal could not run to completion.
#[derive(Debug, thiserror::Error)]
pub enum RehearseError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error("could not read the program: {0}")]
	Idl(#[from] IdlError),

	#[error(transparent)]
	Build(#[from] BuildError),

	#[error(transparent)]
	Workflow(#[from] WorkflowError),

	#[error(
		"Surfpool {found} is too old; `pina rehearse` requires Surfpool {}.{}.{} or newer",
		MINIMUM_SURFPOOL_VERSION.0,
		MINIMUM_SURFPOOL_VERSION.1,
		MINIMUM_SURFPOOL_VERSION.2
	)]
	SurfpoolTooOld { found: String },

	#[error(
		"choose the cluster to rehearse against with --network mainnet|devnet|testnet or \
		 --rpc-url{}",
		cluster.as_ref().map_or_else(String::new, |cluster| format!(" (unknown network {cluster:?})"))
	)]
	UnknownNetwork { cluster: Option<String> },

	#[error("--limit must be between 1 and {MAX_LIMIT}, not {limit}")]
	InvalidLimit { limit: usize },

	#[error("invalid transaction signature {signature:?}; expected 64 base58-encoded bytes")]
	InvalidSignature { signature: String },

	#[error("the project declares an invalid program id {program_id:?}")]
	InvalidProgramId { program_id: String },

	#[error("could not read the candidate program {path}: {source}")]
	ReadCandidate { path: PathBuf, source: io::Error },

	#[error(
		"the candidate program {path} is not an ELF shared object; pass the SBF artifact `pina \
		 build` produces"
	)]
	CandidateNotElf { path: PathBuf },

	#[error("could not prepare the rehearsal's scratch space: {0}")]
	WorkDirectory(io::Error),

	#[error("could not start Surfpool: {0}")]
	StartSurfpool(io::Error),

	#[error("Surfpool exited before it was ready ({status}). Its last log lines:\n{log}")]
	SurfpoolExited { status: String, log: String },

	#[error("Surfpool was not ready within {timeout:?}. Its last log lines:\n{log}")]
	SurfpoolNotReady { timeout: Duration, log: String },

	#[error("{method} failed against {endpoint}: {reason}")]
	Rpc {
		endpoint: String,
		method: &'static str,
		reason: String,
	},

	#[error("program {program_id} does not exist on {cluster}")]
	ProgramNotFound { program_id: String, cluster: String },

	#[error(
		"program {program_id} cannot be rehearsed: {reason}; `pina rehearse` replaces programs \
		 deployed with the upgradeable loader"
	)]
	ProgramNotUpgradeable {
		program_id: String,
		reason: &'static str,
	},

	#[error(
		"Surfpool's runtime could not load the {binary} program for {program_id}; an upgrade to \
		 this ELF would be rejected the same way"
	)]
	ProgramLoad {
		program_id: String,
		binary: &'static str,
	},

	#[error(
		"the fork's program data does not hold the candidate after installation (expected sha256 \
		 {expected}, found {found})"
	)]
	CandidateMismatch { expected: String, found: String },
}

/// Rehearse an upgrade and return the report.
///
/// `progress` receives one line per stage; the CLI points it at stderr so
/// `--json` output stays clean.
///
/// # Errors
///
/// Returns an error when the inputs are invalid, the project or candidate
/// cannot be read, Surfpool is missing, too old, or fails to start, an RPC
/// request fails, or the deployed or candidate program cannot be loaded.
/// Behaviour changes are not errors: they are in the report.
pub fn rehearse(
	options: &RehearseOptions,
	progress: &mut dyn Write,
) -> Result<RehearsalReport, RehearseError> {
	let endpoint = Endpoint::resolve(&options.network)?;
	let selection = Selection::new(options)?;
	let surfpool = crate::workflow::executable("PINA_SURFPOOL", "surfpool");
	let (version, _) = crate::workflow::surfpool_version(&surfpool)?;

	if !crate::workflow::meets_minimum(&version, MINIMUM_SURFPOOL_VERSION) {
		return Err(RehearseError::SurfpoolTooOld {
			found: version.to_string(),
		});
	}

	let target = Target::load(options, progress)?;
	let remote = JsonRpc::new(&endpoint.url, REMOTE_TIMEOUT, false);
	let transactions = fetch(&remote, &endpoint, &target, &selection, progress)?;
	let directory = tempfile::tempdir().map_err(RehearseError::WorkDirectory)?;
	let ports = SurfnetPorts::allocate().map_err(RehearseError::WorkDirectory)?;
	let _ = writeln!(progress, "Starting a Surfpool fork of {}", endpoint.label);
	let surfnet = Surfnet::start(&SurfnetLaunch {
		executable: &surfpool,
		fork_url: &endpoint.url,
		ports,
		directory: directory.path(),
		ready_timeout: READY_TIMEOUT,
		request_timeout: LOCAL_TIMEOUT,
	})?;
	let report = replay(surfnet.rpc(), &endpoint, &target, &transactions, progress);
	// Stop the fork before its scratch directory is removed.
	drop(surfnet);

	report
}

/// The remote RPC endpoint and the label reports show for it.
struct Endpoint {
	url: String,
	/// A cluster name, or only the origin of a custom URL: a path can carry a
	/// provider key, and reports end up in CI logs.
	label: String,
}

impl Endpoint {
	fn resolve(network: &RehearseNetwork) -> Result<Self, RehearseError> {
		match network {
			RehearseNetwork::Cluster(cluster) => {
				Ok(Self {
					url: cluster.rpc_url().to_owned(),
					label: cluster.name().to_owned(),
				})
			}
			RehearseNetwork::RpcUrl(url) => {
				// Surfpool receives the URL as an argument, so it must pass the
				// same checks as `pina dev --rpc-url`.
				crate::workflow::validate_surfpool_network(&SurfpoolNetwork::RpcUrl(url.clone()))?;

				Ok(Self {
					url: url.clone(),
					label: crate::idl_metadata::cluster_display(url),
				})
			}
		}
	}

	fn error(&self, method: &'static str) -> impl FnOnce(RpcError) -> RehearseError + '_ {
		move |error| {
			RehearseError::Rpc {
				endpoint: self.label.clone(),
				method,
				reason: error.to_string(),
			}
		}
	}
}

/// Which transactions to rehearse.
enum Selection {
	Latest(usize),
	Signatures(Vec<String>),
}

impl Selection {
	fn new(options: &RehearseOptions) -> Result<Self, RehearseError> {
		if !options.signatures.is_empty() {
			for signature in &options.signatures {
				let decoded = bs58::decode(signature).into_vec();

				if !matches!(decoded, Ok(bytes) if bytes.len() == 64) {
					return Err(RehearseError::InvalidSignature {
						signature: signature.clone(),
					});
				}
			}

			return Ok(Self::Signatures(options.signatures.clone()));
		}

		if !(1..=MAX_LIMIT).contains(&options.limit) {
			return Err(RehearseError::InvalidLimit {
				limit: options.limit,
			});
		}

		Ok(Self::Latest(options.limit))
	}
}

/// The program being upgraded and its candidate binary.
struct Target {
	program_id: String,
	program_key: [u8; 32],
	catalog: ProgramCatalog,
	candidate: Vec<u8>,
	candidate_sha256: String,
}

impl Target {
	fn load(options: &RehearseOptions, progress: &mut dyn Write) -> Result<Self, RehearseError> {
		let project = Project::discover(&options.project)?;
		let auto = crate::migrations::manifest_auto_policy(&project.program_dir);
		let ir = crate::parse::parse_program_with_auto(&project.program_dir, None, &auto)?;
		let program_key = rpc::decode_address(&ir.public_key).map_err(|_| {
			RehearseError::InvalidProgramId {
				program_id: ir.public_key.clone(),
			}
		})?;
		let catalog = ProgramCatalog::new(
			&ir,
			crate::migrations::manifest_version_type(&project.program_dir),
		);
		let artifact = if options.build {
			let _ = writeln!(progress, "Building the candidate with `pina build`");
			crate::build::build_project(&options.project)?.sbf_artifact
		} else {
			options
				.program
				.clone()
				.unwrap_or_else(|| project.sbf_artifact())
		};
		let candidate = std::fs::read(&artifact).map_err(|source| {
			RehearseError::ReadCandidate {
				path: artifact.clone(),
				source,
			}
		})?;

		if !candidate.starts_with(b"\x7fELF") {
			return Err(RehearseError::CandidateNotElf { path: artifact });
		}

		Ok(Self {
			program_id: ir.public_key,
			program_key,
			catalog,
			candidate_sha256: executable_sha256(&candidate),
			candidate,
		})
	}
}

/// One requested transaction, decoded or with the reason it cannot be replayed.
struct Replay {
	signature: String,
	transaction: Result<Decoded, (SkipReason, String)>,
}

struct Decoded {
	wire: String,
	accounts: Vec<[u8; 32]>,
	instructions: Vec<ProgramInstruction>,
}

/// Fetch the transactions to replay from the remote cluster.
fn fetch(
	remote: &JsonRpc,
	endpoint: &Endpoint,
	target: &Target,
	selection: &Selection,
	progress: &mut dyn Write,
) -> Result<Vec<Replay>, RehearseError> {
	let signatures = match selection {
		Selection::Latest(limit) => {
			let _ = writeln!(
				progress,
				"Fetching up to {limit} recent transactions of {} from {}",
				target.program_id, endpoint.label
			);
			remote
				.call(
					"getSignaturesForAddress",
					&json!([target.program_id, { "limit": limit, "commitment": "confirmed" }]),
				)
				.and_then(|value| rpc::parse_signatures(&value))
				.map_err(endpoint.error("getSignaturesForAddress"))?
		}
		Selection::Signatures(signatures) => signatures.clone(),
	};
	let mut replays = Vec::with_capacity(signatures.len());

	for signature in signatures {
		let response = remote.call(
			"getTransaction",
			&json!([
				signature,
				{ "encoding": "base64", "maxSupportedTransactionVersion": 0, "commitment": "confirmed" }
			]),
		);
		let transaction = match response {
			Err(RpcError::Rpc {
				code: UNSUPPORTED_TRANSACTION_VERSION,
				message,
			}) => Err((SkipReason::Undecodable, message)),
			response => {
				let fetched = response
					.and_then(|value| rpc::parse_transaction(&value))
					.map_err(endpoint.error("getTransaction"))?;
				decode(target, fetched)
			}
		};
		replays.push(Replay {
			signature,
			transaction,
		});
	}

	Ok(replays)
}

fn decode(
	target: &Target,
	fetched: Option<FetchedTransaction>,
) -> Result<Decoded, (SkipReason, String)> {
	let Some(fetched) = fetched else {
		return Err((
			SkipReason::Unavailable,
			"the RPC no longer returns this transaction".to_owned(),
		));
	};
	let transaction = WireTransaction::decode(&fetched.wire)
		.map_err(|reason| (SkipReason::Undecodable, reason))?;
	let instructions = transaction
		.instructions
		.iter()
		.enumerate()
		.filter(|(_, instruction)| transaction.program_id(instruction) == Some(&target.program_key))
		.map(|(position, instruction)| {
			ProgramInstruction {
				position,
				name: target.catalog.instruction_name(&instruction.data),
			}
		})
		.collect();
	let mut accounts = transaction.account_keys;
	accounts.extend(transaction.lookup_tables);
	accounts.extend(fetched.loaded_addresses);

	Ok(Decoded {
		wire: base64::engine::general_purpose::STANDARD.encode(&fetched.wire),
		accounts,
		instructions,
	})
}

/// Replay every transaction against the deployed program, then the candidate.
fn replay(
	local: &JsonRpc,
	endpoint: &Endpoint,
	target: &Target,
	transactions: &[Replay],
	progress: &mut dyn Write,
) -> Result<RehearsalReport, RehearseError> {
	let fork = Fork { rpc: local };
	let slot = fork.hydrate(target, transactions)?;
	let deployed = fork.deployed_program(target, &endpoint.label)?;
	fork.confirm_load(target, &deployed.account, "deployed")?;
	let replayable = transactions
		.iter()
		.filter(|replay| replay.transaction.is_ok())
		.count();

	let _ = writeln!(
		progress,
		"Profiling {replayable} transactions against the deployed program"
	);
	let baseline = fork.profile_all(transactions)?;
	let _ = writeln!(
		progress,
		"Installing the candidate (sha256 {})",
		target.candidate_sha256
	);
	fork.install(target, &deployed)?;
	let _ = writeln!(
		progress,
		"Profiling {replayable} transactions against the candidate"
	);
	let candidate = fork.profile_all(transactions)?;

	let rehearsals = transactions
		.iter()
		.zip(baseline)
		.zip(candidate)
		.map(|((replay, baseline), candidate)| {
			let signature = replay.signature.clone();
			let decoded = match &replay.transaction {
				Ok(decoded) => decoded,
				Err((reason, detail)) => {
					return Ok(TransactionRehearsal::skipped(
						signature,
						&[],
						*reason,
						detail.clone(),
					));
				}
			};

			match (baseline, candidate) {
				(Some(Ok(baseline)), Some(Ok(candidate))) => {
					Ok(TransactionRehearsal::compare(
						signature,
						&decoded.instructions,
						&baseline,
						&candidate,
						&target.catalog,
						&target.program_id,
					))
				}
				// Surfpool refuses a transaction before the program runs, while
				// verifying signatures or loading accounts and lookup tables, so
				// a refusal cannot depend on the binary. The same refusal in both
				// runs comes from the forked state (a lookup table closed since,
				// for example); anything else is the environment failing.
				(Some(Err(before)), Some(Err(after))) if before == after => {
					Ok(TransactionRehearsal::skipped(
						signature,
						&decoded.instructions,
						SkipReason::NotProfiled,
						format!("Surfpool refused it in both runs: {before}"),
					))
				}
				(baseline, candidate) => {
					let refusals = [("deployed", baseline), ("candidate", candidate)]
						.into_iter()
						.filter_map(|(binary, run)| {
							run.and_then(Result::err)
								.map(|error| format!("{binary} run: {error}"))
						})
						.collect::<Vec<_>>()
						.join("; ");

					Err(RehearseError::Rpc {
						endpoint: "the Surfpool fork".to_owned(),
						method: "surfnet_profileTransaction",
						reason: format!(
							"transaction {signature} was refused inconsistently ({refusals}), which \
							 an upgrade cannot cause"
						),
					})
				}
			}
		})
		.collect::<Result<Vec<_>, _>>()?;

	Ok(RehearsalReport::new(
		ReportHeader {
			cluster: endpoint.label.clone(),
			program_id: target.program_id.clone(),
			deployed_sha256: deployed.sha256,
			candidate_sha256: target.candidate_sha256.clone(),
			slot,
		},
		rehearsals,
	))
}

/// The deployed program as the fork holds it.
struct DeployedProgram {
	account: AccountImage,
	programdata: String,
	authority: Option<String>,
	executable_len: usize,
	sha256: String,
}

/// Operations against the local fork.
struct Fork<'a> {
	rpc: &'a JsonRpc,
}

impl Fork<'_> {
	fn call(&self, method: &'static str, params: &Value) -> Result<Value, RehearseError> {
		self.rpc
			.call(method, params)
			.map_err(|error| fork_error(method, &error))
	}

	fn account(&self, address: &str) -> Result<Option<AccountImage>, RehearseError> {
		let value = self.call(
			"getAccountInfo",
			&json!([address, { "encoding": "base64" }]),
		)?;

		rpc::parse_account_info(&value).map_err(|error| fork_error("getAccountInfo", &error))
	}

	/// Load every account the transactions touch into the fork, and freeze the
	/// missing ones offline. Returns the fork's slot.
	fn hydrate(&self, target: &Target, transactions: &[Replay]) -> Result<u64, RehearseError> {
		let mut keys = BTreeSet::from([target.program_key]);

		for replay in transactions {
			if let Ok(decoded) = &replay.transaction {
				keys.extend(decoded.accounts.iter().copied());
			}
		}

		let keys = keys
			.into_iter()
			.map(|key| bs58::encode(key).into_string())
			.collect::<Vec<_>>();
		let mut slot = 0;

		for chunk in keys.chunks(ACCOUNTS_PER_REQUEST) {
			let value = self.call(
				"getMultipleAccounts",
				&json!([
					chunk,
					{ "encoding": "base64", "dataSlice": { "offset": 0, "length": 0 } }
				]),
			)?;
			let (context_slot, accounts) = rpc::parse_accounts(&value)
				.map_err(|error| fork_error("getMultipleAccounts", &error))?;
			slot = slot.max(context_slot);

			for (address, account) in chunk.iter().zip(accounts) {
				if account.is_some() || *address == target.program_id {
					continue;
				}

				self.call(
					"surfnet_offlineAccount",
					&json!([address, { "includeOwnedAccounts": false }]),
				)?;
			}
		}

		Ok(slot)
	}

	fn deployed_program(
		&self,
		target: &Target,
		cluster: &str,
	) -> Result<DeployedProgram, RehearseError> {
		let not_upgradeable = |reason| {
			RehearseError::ProgramNotUpgradeable {
				program_id: target.program_id.clone(),
				reason,
			}
		};
		let account = self.account(&target.program_id)?.ok_or_else(|| {
			RehearseError::ProgramNotFound {
				program_id: target.program_id.clone(),
				cluster: cluster.to_owned(),
			}
		})?;

		if account.owner != UPGRADEABLE_LOADER
			|| !account.executable
			|| account.data.len() < PROGRAM_ACCOUNT_BYTES
			|| account.data[..4] != [2, 0, 0, 0]
		{
			return Err(not_upgradeable(
				"its account is not an upgradeable-loader program",
			));
		}

		let programdata = bs58::encode(&account.data[4..PROGRAM_ACCOUNT_BYTES]).into_string();
		let data = self
			.account(&programdata)?
			.map(|programdata| programdata.data)
			.filter(|data| data.len() >= PROGRAMDATA_HEADER_BYTES && data[..4] == [3, 0, 0, 0])
			.ok_or_else(|| not_upgradeable("its program data is missing or closed"))?;
		let authority = (data[12] == 1)
			.then(|| bs58::encode(&data[13..PROGRAMDATA_HEADER_BYTES]).into_string());

		Ok(DeployedProgram {
			account,
			programdata,
			authority,
			executable_len: data.len() - PROGRAMDATA_HEADER_BYTES,
			sha256: executable_sha256(&data[PROGRAMDATA_HEADER_BYTES..]),
		})
	}

	/// Prove the runtime loaded the program now in program data.
	///
	/// The program account is rewritten with one extra lamport. The runtime
	/// stores a program account only after loading its ELF, so the extra
	/// lamport is visible exactly when the load succeeded. The original
	/// account is then restored, reloading the ELF that just loaded.
	fn confirm_load(
		&self,
		target: &Target,
		account: &AccountImage,
		binary: &'static str,
	) -> Result<(), RehearseError> {
		let sentinel = AccountImage {
			lamports: account.lamports + 1,
			..account.clone()
		};
		self.set_account(&target.program_id, &sentinel)?;
		let observed = self.account(&target.program_id)?;

		if observed.map(|account| account.lamports) != Some(sentinel.lamports) {
			return Err(RehearseError::ProgramLoad {
				program_id: target.program_id.clone(),
				binary,
			});
		}

		self.set_account(&target.program_id, account)
	}

	fn set_account(&self, address: &str, account: &AccountImage) -> Result<(), RehearseError> {
		self.call(
			"surfnet_setAccount",
			&json!([address, {
				"lamports": account.lamports,
				"data": hex(&account.data),
				"owner": account.owner,
				"executable": account.executable,
				"rentEpoch": account.rent_epoch,
			}]),
		)
		.map(|_| ())
	}

	/// Write the candidate into the program's program data as an upgrade
	/// would: same account, same authority, and the old executable's tail
	/// zeroed (`surfnet_writeProgram` only overlays bytes).
	fn install(&self, target: &Target, deployed: &DeployedProgram) -> Result<(), RehearseError> {
		let candidate = &target.candidate;
		let mut offset = 0;

		for chunk in candidate.chunks(WRITE_CHUNK_BYTES) {
			self.write_program(target, deployed, offset, chunk)?;
			offset += chunk.len();
		}

		let zeros = vec![
			0_u8;
			deployed
				.executable_len
				.saturating_sub(candidate.len())
				.min(WRITE_CHUNK_BYTES)
		];

		while offset < deployed.executable_len {
			let length = (deployed.executable_len - offset).min(WRITE_CHUNK_BYTES);
			self.write_program(target, deployed, offset, &zeros[..length])?;
			offset += length;
		}

		self.confirm_load(target, &deployed.account, "candidate")?;
		let data = self
			.account(&deployed.programdata)?
			.map(|programdata| programdata.data)
			.unwrap_or_default();
		let found = executable_sha256(data.get(PROGRAMDATA_HEADER_BYTES..).unwrap_or_default());

		if found != target.candidate_sha256 {
			return Err(RehearseError::CandidateMismatch {
				expected: target.candidate_sha256.clone(),
				found,
			});
		}

		Ok(())
	}

	fn write_program(
		&self,
		target: &Target,
		deployed: &DeployedProgram,
		offset: usize,
		bytes: &[u8],
	) -> Result<(), RehearseError> {
		self.call(
			"surfnet_writeProgram",
			&json!([target.program_id, hex(bytes), offset, deployed.authority]),
		)
		.map(|_| ())
	}

	/// Profile every decodable transaction. A transaction Surfpool refuses
	/// (a failed signature check, for example) records the refusal; transport
	/// failures abort the rehearsal.
	fn profile_all(
		&self,
		transactions: &[Replay],
	) -> Result<Vec<Option<Result<Execution, String>>>, RehearseError> {
		transactions
			.iter()
			.map(|replay| {
				let Ok(decoded) = &replay.transaction else {
					return Ok(None);
				};
				let response = self.rpc.call(
					"surfnet_profileTransaction",
					&json!([decoded.wire, null, { "encoding": "base64", "depth": "instruction" }]),
				);

				match response {
					Err(RpcError::Rpc { message, .. }) => Ok(Some(Err(message))),
					response => {
						response
							.and_then(|value| rpc::parse_execution(&value))
							.map(|execution| Some(Ok(execution)))
							.map_err(|error| fork_error("surfnet_profileTransaction", &error))
					}
				}
			})
			.collect()
	}
}

fn fork_error(method: &'static str, error: &RpcError) -> RehearseError {
	RehearseError::Rpc {
		endpoint: "the Surfpool fork".to_owned(),
		method,
		reason: error.to_string(),
	}
}

/// SHA-256 of an executable with trailing zero padding removed, matching
/// `solana-verify`, so a deployed program's zero-padded program data and its
/// local `.so` hash the same.
fn executable_sha256(bytes: &[u8]) -> String {
	let end = bytes
		.iter()
		.rposition(|byte| *byte != 0)
		.map_or(0, |last| last + 1);

	hex(&Sha256::digest(&bytes[..end]))
}

fn hex(bytes: &[u8]) -> String {
	use std::fmt::Write as _;

	bytes.iter().fold(
		String::with_capacity(bytes.len() * 2),
		|mut output, byte| {
			let _ = write!(output, "{byte:02x}");
			output
		},
	)
}

#[cfg(test)]
#[path = "../../tests/support/rehearse.rs"]
mod fakes;

#[cfg(test)]
mod tests;
