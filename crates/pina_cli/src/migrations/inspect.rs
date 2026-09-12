//! On-chain migration inspection for a single account.
//!
//! Fetches the account over RPC, decodes its discriminator and version
//! envelope, maps both against the checked-in migration manifest, and reports
//! stored versus current version, the pending adjacent transitions with their
//! byte sizes, and the approximate rent deficit per hop. Pairs with ADR 0008
//! (agent flows): the command exits non-zero when the account is stale or
//! from the future, and `--json` emits a machine-readable record.

use std::path::Path;
use std::str::FromStr;

use base64::Engine as _;
use pina_abi::ContractHistory;
use pina_abi::MANIFEST_PATH;
use pina_abi::MigrationManifest;
use serde::Serialize;
use solana_address::Address;

use super::transition::RENT_EXEMPT_LAMPORTS_PER_BYTE;
use crate::error::IdlError;
use crate::project::Project;

/// Approximate planning rent figure, mirrored from the runtime docs.
pub(crate) const RENT_PER_BYTE: u64 = RENT_EXEMPT_LAMPORTS_PER_BYTE;

/// How the inspected account's migration envelope relates to the local
/// manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum InspectState {
	/// The account's version matches the manifest's current version.
	Current,
	/// The account predates the manifest: the listed adjacent transitions
	/// would bring it current.
	Stale,
	/// The account was written by a newer program than this checkout knows.
	Future,
	/// The account data does not start with any manifest discriminator, or is
	/// too short to hold one.
	UnknownContract,
	/// The account exists on chain but carries no data.
	Empty,
}

/// One pending adjacent transition with its cost estimate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InspectHop {
	/// Version the hop starts from.
	pub from: u32,
	/// Version the hop lands on.
	pub to: u32,
	/// Total account byte size at `from` (header plus payload).
	pub byte_size_from: usize,
	/// Total account byte size at `to`.
	pub byte_size_to: usize,
	/// Approximate additional rent-exemption lamports the hop needs.
	pub rent_delta_lamports: u64,
}

/// The full inspection record for one account.
#[derive(Clone, Debug, Serialize)]
pub struct InspectReport {
	/// The requested address, base58 as provided.
	pub address: String,
	/// RPC URL the account was read from.
	pub rpc_url: String,
	/// Whether the account exists on chain.
	pub exists: bool,
	/// On-chain account data length in bytes, when it exists.
	pub data_len: Option<usize>,
	/// Contract key from the manifest, when the discriminator matched.
	pub contract: Option<String>,
	/// Current Rust source name from the manifest.
	pub rust_name: Option<String>,
	/// Version stored on chain, when decodable.
	pub stored_version: Option<u32>,
	/// Current version according to the manifest.
	pub current_version: Option<u32>,
	/// How the stored envelope relates to the local manifest.
	pub state: InspectState,
	/// Pending adjacent transitions, for stale accounts.
	pub hops: Vec<InspectHop>,
}

/// Terminal outcome of the inspect command.
#[derive(Debug, PartialEq, Eq)]
pub struct InspectExit {
	/// Process exit code: 0 for current or unknown contracts, 1 for stale or
	/// future accounts, 2 for operational failures.
	pub code: i32,
}

/// Errors surfaced while inspecting an account.
#[derive(Debug, thiserror::Error)]
pub enum InspectError {
	/// The address argument is not a valid Solana address.
	#[error("invalid account address {address:?}: {reason}")]
	InvalidAddress { address: String, reason: String },
	/// The RPC endpoint could not be reached or returned transport-level
	/// garbage.
	#[error("could not fetch account {address} from {rpc_url}: {reason}")]
	Fetch {
		address: String,
		rpc_url: String,
		reason: String,
	},
	/// The RPC endpoint answered with a JSON-RPC error object.
	#[error("RPC error for account {address}: {message}")]
	Rpc { address: String, message: String },
	/// The manifest could not be loaded.
	#[error(transparent)]
	Manifest(#[from] super::MigrationError),
	/// Project discovery failed.
	#[error(transparent)]
	Project(#[from] crate::project::ProjectError),
}

/// Fetch the raw account data for `address` over JSON-RPC.
///
/// Split from the pure decode logic so tests can exercise the transport
/// against a local HTTP endpoint.
pub fn fetch_account_data(rpc_url: &str, address: &Address) -> Result<Option<Vec<u8>>, String> {
	let body = serde_json::json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "getAccountInfo",
		"params": [
			address.to_string(),
			{ "encoding": "base64", "dataSlice": { "offset": 0, "length": 0 } }
		]
	});
	let mut response = ureq::post(rpc_url)
		.header("content-type", "application/json")
		.send_json(&body)
		.map_err(|error| error.to_string())?;
	let text = response
		.body_mut()
		.read_to_string()
		.map_err(|error| error.to_string())?;
	parse_rpc_account_data(&text, address)
}

/// Parse a `getAccountInfo` JSON-RPC response into raw account data.
///
/// `Ok(None)` means the RPC answered that the account does not exist.
fn parse_rpc_account_data(text: &str, address: &Address) -> Result<Option<Vec<u8>>, String> {
	let value: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
	if let Some(error) = value.get("error") {
		return Err(error
			.get("message")
			.and_then(|message| message.as_str())
			.unwrap_or("unknown JSON-RPC error")
			.to_owned());
	}
	let account = value
		.pointer("/result/value")
		.ok_or_else(|| "response is missing result.value".to_owned())?;
	if account.is_null() {
		return Ok(None);
	}
	let encoding = account
		.get("data")
		.and_then(|data| data.get(1))
		.and_then(|encoding| encoding.as_str())
		.unwrap_or_default();
	if encoding != "base64" {
		return Err(format!("unexpected account encoding {encoding:?}"));
	}
	let encoded = account
		.get("data")
		.and_then(|data| data.get(0))
		.and_then(|bytes| bytes.as_str())
		.ok_or_else(|| "account data is not a base64 pair".to_owned())?;
	let decoded = base64::engine::general_purpose::STANDARD
		.decode(encoded)
		.map_err(|error| error.to_string())?;
	let _ = address;
	Ok(Some(decoded))
}

/// Decode the discriminator and version envelope from raw account data.
///
/// Returns the matching manifest contract, the stored version, and the state
/// classification. Accounts without a matching discriminator or with data too
/// short for the envelope classify as [`InspectState::UnknownContract`].
pub fn decode_envelope<'manifest>(
	manifest: &'manifest MigrationManifest,
	data: &[u8],
) -> Option<(&'manifest ContractHistory, u32, InspectState)> {
	for history in manifest.contracts.values() {
		let identity = &history.identity;
		let disc_bytes = usize::from(identity.discriminator_bytes);
		if data.len() < disc_bytes {
			continue;
		}
		let mut expected = [0_u8; 8];
		let hex = identity.discriminator_hex.trim_start_matches("0x");
		if hex.len() != disc_bytes * 2 {
			continue;
		}
		if base16_decode(hex, &mut expected[..disc_bytes]).is_err() {
			continue;
		}
		if data[..disc_bytes] != expected[..disc_bytes] {
			continue;
		}

		let version_bytes = manifest.version_type.bytes();
		if data.len() < disc_bytes + version_bytes {
			return Some((history, 0, InspectState::UnknownContract));
		}
		let mut stored = [0_u8; 8];
		stored[..version_bytes].copy_from_slice(&data[disc_bytes..disc_bytes + version_bytes]);
		let stored_version = u64::from_le_bytes(stored);
		let Some(current) = history.current() else {
			return Some((history, 0, InspectState::UnknownContract));
		};
		if stored_version > u64::from(current.version) {
			return Some((
				history,
				u32::try_from(stored_version).unwrap_or(u32::MAX),
				InspectState::Future,
			));
		}

		let state = if stored_version == u64::from(current.version) {
			InspectState::Current
		} else {
			InspectState::Stale
		};
		let stored_version = u32::try_from(stored_version).unwrap_or(u32::MAX);
		return Some((history, stored_version, state));
	}
	None
}

/// Decode a hex string into exactly `out.len()` bytes.
fn base16_decode(hex: &str, out: &mut [u8]) -> Result<(), String> {
	if hex.len() != out.len() * 2 {
		return Err("length mismatch".to_owned());
	}
	for (index, byte) in out.iter_mut().enumerate() {
		*byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
			.map_err(|error| error.to_string())?;
	}
	Ok(())
}

/// Build the pending-hop list for a stale account: every adjacent transition
/// from `stored` up to the manifest's current version, with byte sizes and
/// the approximate rent delta per hop.
pub fn pending_hops(
	history: &ContractHistory,
	stored: u32,
	discriminator_bytes: usize,
	version_bytes: usize,
) -> Vec<InspectHop> {
	let header = discriminator_bytes + version_bytes;
	let size_at = |version_index: usize| {
		history
			.versions
			.get(version_index)
			.and_then(|version| version.schema.maximum_payload_size())
			.map_or(header, |payload| header + payload)
	};
	let mut hops = Vec::new();
	let mut version = stored;
	while version < history.current().map_or(0, |current| current.version) {
		let to = version + 1;
		let from_index = usize::try_from(version).unwrap_or(usize::MAX);
		let to_index = usize::try_from(to).unwrap_or(usize::MAX);
		let byte_size_from = size_at(from_index);
		let byte_size_to = size_at(to_index);
		let rent_delta_lamports = RENT_PER_BYTE.saturating_mul(
			u64::try_from(byte_size_to.saturating_sub(byte_size_from)).unwrap_or(u64::MAX),
		);
		hops.push(InspectHop {
			from: version,
			to,
			byte_size_from,
			byte_size_to,
			rent_delta_lamports,
		});
		version = to;
	}
	hops
}

/// Inspect one account against the project's checked-in manifest.
///
/// Returns the report; callers decide the exit code from
/// [`InspectReport::state`].
pub fn build_report(
	address: &str,
	rpc_url: &str,
	data: Option<Vec<u8>>,
	manifest: &MigrationManifest,
) -> InspectReport {
	let mut report = InspectReport {
		address: address.to_owned(),
		rpc_url: rpc_url.to_owned(),
		exists: data.is_some(),
		data_len: data.as_ref().map(Vec::len),
		contract: None,
		rust_name: None,
		stored_version: None,
		current_version: None,
		state: if data.is_some() {
			InspectState::UnknownContract
		} else {
			InspectState::Empty
		},
		hops: Vec::new(),
	};

	let Some(data) = data else {
		return report;
	};
	if let Some((history, stored, state)) = decode_envelope(manifest, &data) {
		report.contract = Some(history.identity.key());
		report.rust_name = Some(history.rust_name.clone());
		report.stored_version = Some(stored);
		report.current_version = history.current().map(|current| current.version);
		report.state = state;
		if state == InspectState::Stale {
			report.hops = pending_hops(
				history,
				stored,
				usize::from(history.identity.discriminator_bytes),
				manifest.version_type.bytes(),
			);
		}
	}
	report
}

/// Load the migration manifest for the project containing `start`.
fn load_project_manifest(start: &Path) -> Result<MigrationManifest, InspectError> {
	let project = Project::discover(start)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	super::load_manifest(&manifest_path)?.ok_or_else(|| {
		InspectError::Manifest(super::MigrationError::InvalidHistory(format!(
			"no migration manifest at {}",
			manifest_path.display()
		)))
	})
}

/// Run the full inspect flow: fetch, decode, report, and classify the exit.
///
/// Returns the report plus the process exit code; printing is the caller's
/// job so both human and JSON rendering stay testable.
pub fn run_inspect(
	project: &Path,
	address: &str,
	rpc_url: &str,
) -> Result<(InspectReport, InspectExit), InspectError> {
	let parsed_address = Address::from_str(address).map_err(|error| {
		InspectError::InvalidAddress {
			address: address.to_owned(),
			reason: error.to_string(),
		}
	})?;
	let manifest = load_project_manifest(project)?;
	let data = fetch_account_data(rpc_url, &parsed_address).map_err(|reason| {
		InspectError::Fetch {
			address: address.to_owned(),
			rpc_url: rpc_url.to_owned(),
			reason,
		}
	})?;
	let report = build_report(address, rpc_url, data, &manifest);
	let code = match report.state {
		InspectState::Stale | InspectState::Future => 1,
		InspectState::Current | InspectState::UnknownContract | InspectState::Empty => 0,
	};
	Ok((report, InspectExit { code }))
}

/// Validate an address argument up front, for fail-fast CLI behavior.
pub fn validate_address(address: &str) -> Result<Address, IdlError> {
	Address::from_str(address)
		.map_err(|error| IdlError::Other(format!("invalid account address {address:?}: {error}")))
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod inspect_tests;
