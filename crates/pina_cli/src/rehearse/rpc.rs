//! JSON-RPC transport for the remote cluster and the local Surfpool fork.
//!
//! Every request has an end-to-end timeout and is sent exactly once: a failure
//! is reported, never retried, so a rate-limited endpoint is not hammered.
//! Redirects are not followed, because a redirect could move a request (and
//! anything in its path) to a host the user never named.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine as _;
use serde_json::Value;
use serde_json::json;

use super::report::AccountImage;
use super::report::Execution;

/// Largest response body read. Profiles carry full account images for every
/// instruction, so they can be far larger than ordinary RPC responses.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

/// A JSON-RPC endpoint.
pub(crate) struct JsonRpc {
	agent: ureq::Agent,
	url: String,
	next_id: Cell<u64>,
}

/// Why one JSON-RPC request failed.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum RpcError {
	#[error("request failed: {0}")]
	Transport(String),
	#[error("the endpoint redirected with HTTP {0}; redirects are not followed")]
	Redirect(u16),
	#[error("the endpoint answered HTTP {0}")]
	Http(u16),
	#[error("the response could not be read: {0}")]
	Body(String),
	#[error("JSON-RPC error {code}: {message}")]
	Rpc { code: i64, message: String },
	#[error("unexpected response: {0}")]
	Shape(String),
}

impl JsonRpc {
	/// Create a client. Loopback clients ignore proxy environment variables so
	/// a configured proxy never intercepts the local fork.
	pub(crate) fn new(url: &str, timeout: Duration, loopback: bool) -> Self {
		let mut config = ureq::Agent::config_builder()
			.timeout_global(Some(timeout))
			.max_redirects(0)
			.http_status_as_error(false);

		if loopback {
			config = config.proxy(None);
		}

		Self {
			agent: config.build().new_agent(),
			url: url.to_owned(),
			next_id: Cell::new(1),
		}
	}

	/// Send one request and return its `result`.
	pub(crate) fn call(&self, method: &str, params: &Value) -> Result<Value, RpcError> {
		let id = self.next_id.get();
		self.next_id.set(id + 1);
		let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
		let mut response = self
			.agent
			.post(&self.url)
			.header("content-type", "application/json")
			.send_json(&body)
			.map_err(|error| RpcError::Transport(error.to_string()))?;
		let status = response.status();

		if status.is_redirection() {
			return Err(RpcError::Redirect(status.as_u16()));
		}

		if !status.is_success() {
			return Err(RpcError::Http(status.as_u16()));
		}

		let text = response
			.body_mut()
			.with_config()
			.limit(MAX_RESPONSE_BYTES)
			.read_to_string()
			.map_err(|error| RpcError::Body(error.to_string()))?;
		let mut value: Value =
			serde_json::from_str(&text).map_err(|error| RpcError::Body(error.to_string()))?;

		if let Some(error) = value.get("error") {
			return Err(RpcError::Rpc {
				code: error
					.get("code")
					.and_then(Value::as_i64)
					.unwrap_or_default(),
				message: error
					.get("message")
					.and_then(Value::as_str)
					.unwrap_or("no message")
					.to_owned(),
			});
		}

		value
			.get_mut("result")
			.map(Value::take)
			.ok_or_else(|| RpcError::Shape("the response has no result".to_owned()))
	}
}

/// A transaction fetched from the remote cluster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FetchedTransaction {
	/// The signed transaction exactly as it landed.
	pub(crate) wire: Vec<u8>,
	/// Accounts its address lookup tables resolved to.
	pub(crate) loaded_addresses: Vec<[u8; 32]>,
}

/// Parse `getSignaturesForAddress`.
pub(crate) fn parse_signatures(value: &Value) -> Result<Vec<String>, RpcError> {
	value
		.as_array()
		.ok_or_else(|| shape("signatures are not an array"))?
		.iter()
		.map(|entry| {
			entry
				.get("signature")
				.and_then(Value::as_str)
				.map(str::to_owned)
				.ok_or_else(|| shape("a signature entry has no signature"))
		})
		.collect()
}

/// Parse `getTransaction` with base64 encoding; `None` means the RPC no longer
/// has the transaction.
pub(crate) fn parse_transaction(value: &Value) -> Result<Option<FetchedTransaction>, RpcError> {
	if value.is_null() {
		return Ok(None);
	}

	let wire = base64_pair(&value["transaction"])
		.ok_or_else(|| shape("the transaction is not a base64 pair"))?;
	let loaded = &value["meta"]["loadedAddresses"];
	let mut loaded_addresses = Vec::new();

	for group in ["writable", "readonly"] {
		for address in loaded[group].as_array().into_iter().flatten() {
			let address = address
				.as_str()
				.ok_or_else(|| shape("a loaded address is not a string"))?;
			loaded_addresses.push(decode_address(address)?);
		}
	}

	Ok(Some(FetchedTransaction {
		wire,
		loaded_addresses,
	}))
}

/// Parse `getMultipleAccounts` into its context slot and accounts.
pub(crate) fn parse_accounts(value: &Value) -> Result<(u64, Vec<Option<AccountImage>>), RpcError> {
	let slot = value["context"]["slot"]
		.as_u64()
		.ok_or_else(|| shape("the response has no context slot"))?;
	let accounts = value["value"]
		.as_array()
		.ok_or_else(|| shape("the accounts are not an array"))?
		.iter()
		.map(parse_optional_account)
		.collect::<Result<_, _>>()?;

	Ok((slot, accounts))
}

/// Parse `getAccountInfo`.
pub(crate) fn parse_account_info(value: &Value) -> Result<Option<AccountImage>, RpcError> {
	parse_optional_account(&value["value"])
}

fn parse_optional_account(value: &Value) -> Result<Option<AccountImage>, RpcError> {
	if value.is_null() {
		return Ok(None);
	}

	parse_account(value).map(Some)
}

/// Parse a base64-encoded `UiAccount`.
fn parse_account(value: &Value) -> Result<AccountImage, RpcError> {
	Ok(AccountImage {
		lamports: value["lamports"]
			.as_u64()
			.ok_or_else(|| shape("an account has no lamports"))?,
		owner: value["owner"]
			.as_str()
			.ok_or_else(|| shape("an account has no owner"))?
			.to_owned(),
		executable: value["executable"]
			.as_bool()
			.ok_or_else(|| shape("an account has no executable flag"))?,
		rent_epoch: value["rentEpoch"].as_u64().unwrap_or_default(),
		data: base64_pair(&value["data"])
			.ok_or_else(|| shape("account data is not a base64 pair"))?,
	})
}

/// Parse a `surfnet_profileTransaction` result.
pub(crate) fn parse_execution(value: &Value) -> Result<Execution, RpcError> {
	let profile = &value["value"];
	let transaction = profile
		.get("transactionProfile")
		.ok_or_else(|| shape("the profile has no transaction profile"))?;
	let instruction_units = match profile.get("instructionProfiles") {
		None | Some(Value::Null) => Vec::new(),
		Some(profiles) => {
			profiles
				.as_array()
				.ok_or_else(|| shape("instruction profiles are not an array"))?
				.iter()
				.map(|profile| profile["computeUnitsConsumed"].as_u64())
				.collect::<Option<Vec<_>>>()
				.ok_or_else(|| shape("an instruction profile has no compute units"))?
		}
	};

	Ok(Execution {
		error: transaction["errorMessage"].as_str().map(str::to_owned),
		compute_units: transaction["computeUnitsConsumed"]
			.as_u64()
			.ok_or_else(|| shape("the transaction profile has no compute units"))?,
		logs: transaction["logMessages"]
			.as_array()
			.into_iter()
			.flatten()
			.filter_map(Value::as_str)
			.map(str::to_owned)
			.collect(),
		instruction_units,
		accounts: parse_account_states(&transaction["accountStates"])?,
	})
}

/// Final state of each writable account in a profile.
fn parse_account_states(
	states: &Value,
) -> Result<BTreeMap<String, Option<AccountImage>>, RpcError> {
	let mut accounts = BTreeMap::new();

	for (address, state) in states.as_object().into_iter().flatten() {
		if state["type"] == "readonly" {
			continue;
		}

		let change = &state["accountChange"];
		let data = &change["data"];
		let image = match change["type"].as_str() {
			Some("create") => Some(parse_account(data)?),
			Some("update") => Some(parse_account(&data[1])?),
			Some("delete") => None,
			Some("unchanged") => parse_optional_account(data)?,
			_ => return Err(shape("an account change has an unknown type")),
		};
		accounts.insert(address.clone(), image);
	}

	Ok(accounts)
}

/// Decode a base58 address.
pub(crate) fn decode_address(value: &str) -> Result<[u8; 32], RpcError> {
	bs58::decode(value)
		.into_vec()
		.ok()
		.and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
		.ok_or_else(|| shape("an address is not 32 base58 bytes"))
}

fn base64_pair(value: &Value) -> Option<Vec<u8>> {
	if value[1] != "base64" {
		return None;
	}

	base64::engine::general_purpose::STANDARD
		.decode(value[0].as_str()?)
		.ok()
}

fn shape(message: &str) -> RpcError {
	RpcError::Shape(message.to_owned())
}
