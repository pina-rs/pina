//! JSON-RPC reads for `pina explain`.
//!
//! Each command issues at most one `getTransaction` and one
//! `getMultipleAccounts` request. Neither is retried: a failure is reported
//! with the endpoint's label, never its URL, because a custom URL's path can
//! carry a provider token.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use serde_json::json;
use url::Host;
use url::Url;

use super::ExplainError;
use super::Network;

/// How long one RPC request may take, end to end.
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Owner the runtime reports for an account that does not exist.
pub(crate) const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/// A JSON-RPC endpoint and the label reports show for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RpcEndpoint {
	url: String,
	label: String,
}

impl RpcEndpoint {
	/// The public endpoint of a named cluster.
	#[must_use]
	pub fn network(network: Network) -> Self {
		let (url, label) = match network {
			Network::Localnet => ("http://127.0.0.1:8899", "localnet"),
			Network::Devnet => ("https://api.devnet.solana.com", "devnet"),
			Network::Testnet => ("https://api.testnet.solana.com", "testnet"),
			Network::Mainnet => ("https://api.mainnet-beta.solana.com", "mainnet"),
		};

		Self {
			url: url.to_owned(),
			label: label.to_owned(),
		}
	}

	/// A custom HTTP(S) endpoint.
	///
	/// # Errors
	///
	/// Rejects URLs with control characters, credentials, a query, or a
	/// fragment, URLs without a host, schemes other than HTTP(S), and plaintext
	/// HTTP to anything but a loopback host.
	pub fn custom(url: &str) -> Result<Self, ExplainError> {
		if url.chars().any(char::is_control) {
			return Err(invalid_url("control characters are not accepted"));
		}

		let parsed = Url::parse(url).map_err(|_| invalid_url("it is not a valid URL"))?;
		let is_loopback = match parsed.host() {
			Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
			Some(Host::Ipv4(address)) => address.is_loopback(),
			Some(Host::Ipv6(address)) => address.is_loopback(),
			None => return Err(invalid_url("an endpoint host is required")),
		};

		if !matches!(parsed.scheme(), "http" | "https") {
			return Err(invalid_url("only http and https endpoints are accepted"));
		}
		if !parsed.username().is_empty() || parsed.password().is_some() {
			return Err(invalid_url("embedded credentials are not accepted"));
		}
		if parsed.query().is_some() || parsed.fragment().is_some() {
			return Err(invalid_url(
				"query parameters and fragments are not accepted",
			));
		}
		if parsed.scheme() == "http" && !is_loopback {
			return Err(invalid_url(
				"plaintext http is accepted only for a loopback host",
			));
		}

		Ok(Self {
			url: parsed.to_string(),
			label: "custom RPC endpoint".to_owned(),
		})
	}

	/// The label reports show instead of the URL.
	#[must_use]
	pub fn label(&self) -> &str {
		&self.label
	}
}

fn invalid_url(reason: &str) -> ExplainError {
	ExplainError::InvalidRpcUrl {
		reason: reason.to_owned(),
	}
}

/// Current on-chain state of one account, as far as a diagnosis reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AccountState {
	pub(crate) owner: String,
	pub(crate) executable: bool,
	/// `None` when the RPC did not report the account's size.
	pub(crate) data_len: Option<u64>,
}

/// Fetch a transaction by signature at `confirmed` commitment.
///
/// # Errors
///
/// Returns an error when the request fails, or [`ExplainError::NotFound`] when
/// the endpoint does not know the signature.
pub(crate) fn get_transaction(
	endpoint: &RpcEndpoint,
	signature: &str,
) -> Result<Value, ExplainError> {
	let result = call(
		endpoint,
		"getTransaction",
		&json!([
			signature,
			{ "encoding": "json", "maxSupportedTransactionVersion": 0, "commitment": "confirmed" }
		]),
	)?;

	if result.is_null() {
		return Err(ExplainError::NotFound {
			signature: signature.to_owned(),
			endpoint: endpoint.label.clone(),
		});
	}

	Ok(result)
}

/// Fetch the current owner, executable flag, and size of `addresses`.
///
/// The request asks for an empty data slice and reads the size from `space`, so
/// a large account costs no transfer.
///
/// # Errors
///
/// Returns an error when the request fails or the response does not list one
/// entry per address.
pub(crate) fn get_account_states(
	endpoint: &RpcEndpoint,
	addresses: &[String],
) -> Result<HashMap<String, AccountState>, ExplainError> {
	let result = call(
		endpoint,
		"getMultipleAccounts",
		&json!([
			addresses,
			{ "encoding": "base64", "dataSlice": { "offset": 0, "length": 0 }, "commitment": "confirmed" }
		]),
	)?;
	let values = result
		.get("value")
		.and_then(Value::as_array)
		.filter(|values| values.len() == addresses.len())
		.ok_or_else(|| {
			rpc_error(
				endpoint,
				"getMultipleAccounts",
				"the response does not list one account per requested address",
			)
		})?;

	Ok(addresses
		.iter()
		.zip(values)
		.map(|(address, value)| (address.clone(), account_state(value)))
		.collect())
}

fn account_state(value: &Value) -> AccountState {
	if value.is_null() {
		return AccountState {
			owner: SYSTEM_PROGRAM_ID.to_owned(),
			executable: false,
			data_len: Some(0),
		};
	}

	AccountState {
		owner: value
			.get("owner")
			.and_then(Value::as_str)
			.unwrap_or_default()
			.to_owned(),
		executable: value
			.get("executable")
			.and_then(Value::as_bool)
			.unwrap_or_default(),
		data_len: value.get("space").and_then(Value::as_u64),
	}
}

/// Send one JSON-RPC request and return its `result`.
///
/// Redirects are not followed: a redirect would send the request, and any
/// token in the URL path, to a host the user did not name.
fn call(endpoint: &RpcEndpoint, method: &str, params: &Value) -> Result<Value, ExplainError> {
	let agent = ureq::Agent::config_builder()
		.timeout_global(Some(RPC_TIMEOUT))
		.max_redirects(0)
		.build()
		.new_agent();
	let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
	let mut response = agent
		.post(&endpoint.url)
		.header("content-type", "application/json")
		.send_json(&body)
		.map_err(|error| rpc_error(endpoint, method, &error.to_string()))?;
	let status = response.status();

	if !status.is_success() {
		return Err(rpc_error(
			endpoint,
			method,
			&format!("the endpoint answered HTTP {status}; redirects are not followed"),
		));
	}

	let text = response
		.body_mut()
		.read_to_string()
		.map_err(|error| rpc_error(endpoint, method, &error.to_string()))?;
	let value: Value = serde_json::from_str(&text)
		.map_err(|error| rpc_error(endpoint, method, &format!("invalid JSON: {error}")))?;

	envelope_result(value).map_err(|reason| rpc_error(endpoint, method, &reason))
}

/// Unwrap a JSON-RPC response envelope into its `result`, which may be null.
pub(crate) fn envelope_result(mut value: Value) -> Result<Value, String> {
	if let Some(error) = value.get("error") {
		return Err(error
			.get("message")
			.and_then(Value::as_str)
			.map_or_else(|| error.to_string(), ToOwned::to_owned));
	}

	value
		.get_mut("result")
		.map(Value::take)
		.ok_or_else(|| "the response has neither `result` nor `error`".to_owned())
}

fn rpc_error(endpoint: &RpcEndpoint, method: &str, reason: &str) -> ExplainError {
	ExplainError::Rpc {
		method: method.to_owned(),
		endpoint: endpoint.label.clone(),
		reason: reason.to_owned(),
	}
}
