//! Off-chain diagnosis of failed Pina transactions.
//!
//! A failed Pina check returns a bare error code, and several checks share
//! `InvalidAccountData`. A unique code per check would cost every program
//! size and compute units, so `pina explain` reconstructs the failing field and
//! rule off-chain instead: from the transaction's account privileges, its error,
//! and its logs, matched against the program's source with `path:line` locations.

mod decode;
mod diagnose;
mod render;
mod rpc;
mod source;
#[cfg(test)]
mod tests;
mod transaction;

use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use clap::ValueEnum;
use serde::Serialize;
use serde_json::Value;

pub use self::decode::CpiFailure;
pub use self::decode::DecodedError;
pub use self::decode::ErrorKind;
use self::decode::PINA_PROGRAM_ERRORS;
use self::decode::RESERVED_ERROR_CODE_START;
pub use self::diagnose::AccountRow;
pub use self::diagnose::Candidate;
pub use self::diagnose::CandidateScope;
pub use self::diagnose::Confidence;
use self::diagnose::CurrentState;
use self::diagnose::Failure;
pub use self::diagnose::InstructionSummary;
pub use self::rpc::RpcEndpoint;
pub use self::source::ErrorSite;
use self::source::SourceIndex;
use self::transaction::InstructionError;
use self::transaction::Outcome;
use self::transaction::Transaction;
use crate::error::IdlError;
use crate::ir::ProgramIr;
use crate::parse::error_enum::DeclaredError;
use crate::parse::parse_program_with_sources;
use crate::project::Project;
use crate::project::ProjectError;

/// Versioned JSON schema emitted by `pina explain --json`.
pub const EXPLAIN_SCHEMA_VERSION: u8 = 1;

/// How many trailing log lines of the failing instruction a report keeps.
const LOG_LINES: usize = 12;

/// A named Solana cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Network {
	/// A local validator or Surfnet at `http://127.0.0.1:8899`.
	Localnet,
	/// The public devnet endpoint.
	Devnet,
	/// The public testnet endpoint.
	Testnet,
	/// The public mainnet-beta endpoint. Never selected by default.
	Mainnet,
}

/// Where the transaction to explain comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionInput {
	/// Fetch the transaction by signature, then read current account state from
	/// the same endpoint.
	Signature {
		signature: String,
		endpoint: RpcEndpoint,
	},
	/// Read a saved `getTransaction` result or JSON-RPC response. Current
	/// account state is read only when an endpoint is given.
	File {
		path: PathBuf,
		endpoint: Option<RpcEndpoint>,
	},
}

/// Inputs for [`explain`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplainOptions {
	/// Directory inside the program project.
	pub project: PathBuf,
	/// The transaction to explain.
	pub input: TransactionInput,
}

/// Errors that prevent an explanation.
#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error("could not read the program source: {0}")]
	Source(IdlError),

	#[error("could not index the program source: {0}")]
	SourceIndex(syn::Error),

	#[error("invalid transaction signature `{signature}`: expected 64 base58-encoded bytes")]
	InvalidSignature { signature: String },

	#[error("unsafe RPC URL: {reason}")]
	InvalidRpcUrl { reason: String },

	#[error("could not read {}: {source}", path.display())]
	ReadFile {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("invalid transaction JSON: {reason}")]
	InvalidTransaction { reason: String },

	#[error("{method} on the {endpoint} failed: {reason}")]
	Rpc {
		method: String,
		endpoint: String,
		reason: String,
	},

	#[error(
		"transaction {signature} was not found on the {endpoint}; it may not be confirmed yet, \
		 preflight may have rejected it before it landed, or the node no longer stores it"
	)]
	NotFound { signature: String, endpoint: String },
}

/// Whether the explained transaction succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
	Succeeded,
	Failed,
}

/// The program the project describes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramSummary {
	pub name: String,
	pub program_id: String,
}

/// What failed and with which error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureReport {
	/// The failing top-level instruction; `None` for a transaction-level error.
	pub instruction_index: Option<usize>,
	/// Program the failing instruction targets.
	pub program_id: Option<String>,
	/// Whether that program is the one the project describes.
	pub targets_program: bool,
	/// The instruction, when the project identifies it.
	pub instruction: Option<InstructionSummary>,
	pub error: DecodedError,
	/// The program that failed first when the error came from a CPI.
	pub cpi: Option<CpiFailure>,
}

/// The explanation of one transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainReport {
	/// Report schema version for agent compatibility.
	pub schema_version: u8,
	pub signature: String,
	pub slot: u64,
	/// `file`, or the label of the endpoint the transaction was fetched from.
	pub source: String,
	pub program: ProgramSummary,
	pub status: TransactionStatus,
	pub failure: Option<FailureReport>,
	/// Checks that may have produced the error, most likely first.
	pub candidates: Vec<Candidate>,
	/// Source sites that construct the error, sites of the failing instruction
	/// first.
	pub error_sites: Vec<ErrorSite>,
	/// The failing instruction's accounts.
	pub accounts: Vec<AccountRow>,
	/// Trailing log lines of the failing instruction.
	pub logs: Vec<String>,
	pub notes: Vec<String>,
}

/// Explain a transaction against the program in `options.project`.
///
/// The project is parsed before any network request, so a source problem never
/// costs an RPC call. A signature input issues one `getTransaction`; a failed
/// instruction of the project's program then issues one `getMultipleAccounts`
/// when an endpoint is available. Nothing is retried.
///
/// # Errors
///
/// Returns an error when the project cannot be discovered or parsed, the
/// signature or file is invalid, or the transaction cannot be fetched. A failed
/// state lookup is reported as a note instead.
pub fn explain(options: &ExplainOptions) -> Result<ExplainReport, ExplainError> {
	if let TransactionInput::Signature { signature, .. } = &options.input {
		validate_signature(signature)?;
	}

	let project = Project::discover(&options.project)?;
	let (program, files) =
		parse_program_with_sources(&project.program_dir, None, &pina_abi::MigrationAuto::none())
			.map_err(ExplainError::Source)?;
	let source = source::index(files, &project.root).map_err(ExplainError::SourceIndex)?;

	let (value, endpoint, origin) = match &options.input {
		TransactionInput::Signature {
			signature,
			endpoint,
		} => {
			let value = rpc::get_transaction(endpoint, signature)?;

			(value, Some(endpoint), endpoint.label().to_owned())
		}
		TransactionInput::File { path, endpoint } => {
			(
				read_transaction_file(path)?,
				endpoint.as_ref(),
				"file".to_owned(),
			)
		}
	};
	let transaction = transaction::parse_transaction(value)?;

	Ok(report(&program, &source, &transaction, endpoint, origin))
}

fn validate_signature(signature: &str) -> Result<(), ExplainError> {
	match bs58::decode(signature).into_vec() {
		Ok(bytes) if bytes.len() == 64 => Ok(()),
		_ => {
			Err(ExplainError::InvalidSignature {
				signature: signature.to_owned(),
			})
		}
	}
}

/// Read a `getTransaction` result, unwrapping a JSON-RPC envelope.
fn read_transaction_file(path: &Path) -> Result<Value, ExplainError> {
	let text = std::fs::read_to_string(path).map_err(|source| {
		ExplainError::ReadFile {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let value: Value = serde_json::from_str(&text).map_err(|error| {
		ExplainError::InvalidTransaction {
			reason: error.to_string(),
		}
	})?;

	if value.get("transaction").is_some() {
		return Ok(value);
	}

	let result = rpc::envelope_result(value).map_err(|reason| {
		ExplainError::InvalidTransaction {
			reason: format!("the file is not a getTransaction result: {reason}"),
		}
	})?;

	if result.is_null() {
		return Err(ExplainError::InvalidTransaction {
			reason: "the file holds a null result; the RPC did not find the transaction".to_owned(),
		});
	}

	Ok(result)
}

fn report(
	program: &ProgramIr,
	source: &SourceIndex,
	transaction: &Transaction,
	endpoint: Option<&RpcEndpoint>,
	origin: String,
) -> ExplainReport {
	let mut report = ExplainReport {
		schema_version: EXPLAIN_SCHEMA_VERSION,
		signature: transaction.signature.clone(),
		slot: transaction.slot,
		source: origin,
		program: ProgramSummary {
			name: program.name.clone(),
			program_id: program.public_key.clone(),
		},
		status: TransactionStatus::Failed,
		failure: None,
		candidates: Vec::new(),
		error_sites: Vec::new(),
		accounts: Vec::new(),
		logs: Vec::new(),
		notes: Vec::new(),
	};
	let logs = transaction.logs.as_deref().unwrap_or_default();

	match &transaction.outcome {
		Outcome::Succeeded => {
			report.status = TransactionStatus::Succeeded;
			report
				.notes
				.push("The transaction succeeded; there is nothing to explain.".to_owned());
		}
		Outcome::TransactionFailed(error) => {
			report.failure = Some(FailureReport {
				instruction_index: None,
				program_id: None,
				targets_program: false,
				instruction: None,
				error: transaction_error(error),
				cpi: None,
			});
			report.logs = tail(logs);
			report.notes.push(
				"The runtime rejected the transaction outside instruction execution, so no \
				 account check ran."
					.to_owned(),
			);
		}
		Outcome::InstructionFailed { index, error } => {
			explain_instruction(
				&mut report,
				program,
				source,
				transaction,
				(*index, error),
				endpoint,
			);
		}
	}

	if transaction.logs.is_none() && report.status == TransactionStatus::Failed {
		report
			.notes
			.push("The node returned no logs for this transaction.".to_owned());
	}

	report
}

fn explain_instruction(
	report: &mut ExplainReport,
	program: &ProgramIr,
	source: &SourceIndex,
	transaction: &Transaction,
	(index, error): (usize, &InstructionError),
	endpoint: Option<&RpcEndpoint>,
) {
	let instruction = &transaction.instructions[index];
	let logs = transaction.logs.as_deref().unwrap_or_default();
	let analysis = decode::analyze_logs(logs, &instruction.program_id);
	let targets_program = instruction.program_id == program.public_key;
	let own_error = targets_program && analysis.cpi_failure.is_none();
	let declared: &[DeclaredError] = if own_error { &source.errors } else { &[] };
	let mut decoded = decode::decode_error(error, declared);

	if !targets_program && decoded.kind == ErrorKind::Unknown {
		decoded.description = Some(format!(
			"A custom error of {}, which this project does not describe.",
			instruction.program_id
		));
	}

	report.logs = tail(if analysis.segment.is_empty() {
		logs
	} else {
		&analysis.segment
	});
	if analysis.truncated {
		report.notes.push(
			"The node truncated the logs, so the failing instruction's lines may be incomplete."
				.to_owned(),
		);
	}

	let mut failure = FailureReport {
		instruction_index: Some(index),
		program_id: Some(instruction.program_id.clone()),
		targets_program,
		instruction: None,
		error: decoded,
		cpi: analysis.cpi_failure.clone(),
	};

	if !targets_program {
		report.accounts = instruction
			.accounts
			.iter()
			.enumerate()
			.map(|(position, key)| {
				let account = &transaction.accounts[*key];

				AccountRow {
					index: position,
					field: None,
					address: account.address.clone(),
					signer: account.signer,
					writable: account.writable,
					absent: false,
				}
			})
			.collect();
		report.notes.push(format!(
			"Instruction #{index} targets {}, not `{}` ({}), so its checks are not in this project.",
			instruction.program_id, program.name, program.public_key
		));
		report.failure = Some(failure);
		return;
	}

	let mut states = None;
	let mut unavailable = false;

	// Current state only matters for the program's own checks.
	if let Some(endpoint) = endpoint
		&& own_error
		&& !instruction.accounts.is_empty()
	{
		let addresses: Vec<String> = instruction
			.accounts
			.iter()
			.map(|key| transaction.accounts[*key].address.clone())
			.collect::<BTreeSet<_>>()
			.into_iter()
			.collect();

		match rpc::get_account_states(endpoint, &addresses) {
			Ok(fetched) => states = Some(fetched),
			Err(error) => {
				report
					.notes
					.push(format!("Current account state is unavailable: {error}."));
				unavailable = true;
			}
		}
	}

	let state = match &states {
		Some(states) => CurrentState::Fetched(states),
		None if unavailable => CurrentState::Unavailable,
		None => CurrentState::NotRequested,
	};
	let diagnosis = diagnose::diagnose(
		program,
		source,
		&Failure {
			transaction,
			instruction,
			error,
			messages: &analysis.program_messages,
			state,
		},
	);

	report.accounts = diagnosis.accounts;
	report.notes.extend(diagnosis.notes);
	failure.instruction = diagnosis.instruction;

	if let Some(cpi) = &analysis.cpi_failure {
		report.notes.push(format!(
			"The instruction failed inside a CPI to {} (depth {}): {}. The program's own account \
			 checks ran before the call.",
			cpi.program_id, cpi.depth, cpi.message
		));
	} else {
		report.candidates = diagnosis.candidates;
		report.error_sites =
			source.error_sites(&construction_paths(error, declared), &diagnosis.flow);

		if let Some(endpoint) = endpoint
			&& report
				.candidates
				.iter()
				.any(|candidate| candidate.confidence == Confidence::CheckedAgainstCurrentState)
		{
			report.notes.push(format!(
				"Account state was read from the {} after the transaction ran; it may have \
				 changed since.",
				endpoint.label()
			));
		}

		if report.candidates.is_empty() {
			report.notes.push(
				"No declared Pina check of this instruction returns this error; it came from the \
				 processor body or a helper it calls."
					.to_owned(),
			);
		}
	}

	report.failure = Some(failure);
}

/// The `Enum::Variant` paths a program writes to return `error`.
fn construction_paths(
	error: &InstructionError,
	declared: &[DeclaredError],
) -> Vec<(String, String)> {
	match error {
		InstructionError::Builtin(name) => vec![("ProgramError".to_owned(), name.clone())],
		InstructionError::Custom(code) if *code >= RESERVED_ERROR_CODE_START => {
			PINA_PROGRAM_ERRORS
				.iter()
				.filter(|(_, candidate, _)| candidate == code)
				.map(|(name, ..)| ("PinaProgramError".to_owned(), (*name).to_owned()))
				.collect()
		}
		InstructionError::Custom(code) => {
			declared
				.iter()
				.filter(|declared| declared.error.code == *code)
				.map(|declared| (declared.enum_name.clone(), declared.error.name.clone()))
				.collect()
		}
	}
}

fn transaction_error(error: &Value) -> DecodedError {
	let name = match error {
		Value::String(name) => name.clone(),
		Value::Object(object) if object.len() == 1 => object.keys().cloned().collect(),
		other => other.to_string(),
	};

	DecodedError {
		kind: ErrorKind::Transaction,
		name,
		code: None,
		description: error.is_object().then(|| error.to_string()),
	}
}

fn tail(lines: &[String]) -> Vec<String> {
	lines[lines.len().saturating_sub(LOG_LINES)..].to_vec()
}
