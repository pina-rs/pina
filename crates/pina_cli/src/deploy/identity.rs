//! Identify the cluster behind a loopback RPC endpoint.
//!
//! A loopback address proves only where the socket is, not which cluster
//! answers it: an SSH tunnel or proxy on `127.0.0.1:8899` can forward to a live
//! cluster (SEC-13). Before planning a loopback deployment, Pina asks the
//! endpoint for its genesis hash. A hash that names a public cluster gets that
//! cluster's safeguards. Any other hash is a private validator, which stays
//! local.
//!
//! Surfpool is the exception: a fork reports the genesis hash of the cluster it
//! forks, because clients read that hash as the cluster's identity, yet every
//! transaction it accepts runs in its local SVM. Its `getVersion` answer
//! carries a `surfnet-version`, which no Agave RPC reports, so a Surfpool
//! endpoint stays local whatever genesis hash it reports.
//!
//! The probe guards against an operator's mistake, not against the endpoint:
//! whatever listens on the loopback port can answer anything.

use std::time::Duration;

use serde_json::Value;
use serde_json::json;

use super::Cluster;
use crate::rehearse::rpc::JsonRpc;

pub(super) const DEVNET_GENESIS: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
pub(super) const TESTNET_GENESIS: &str = "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY";
pub(super) const MAINNET_GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";

/// A local endpoint answers at once, and a stalled port must not hang planning.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// What a loopback endpoint reported about itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
	/// The endpoint's `getGenesisHash` answer.
	pub(crate) genesis_hash: String,
	/// Whether `getVersion` carried Surfpool's `surfnet-version`.
	pub(crate) surfpool: bool,
}

impl Identity {
	/// The public cluster this endpoint serves, or `None` for a private
	/// validator or any Surfpool network.
	pub(super) fn public_cluster(&self) -> Option<Cluster> {
		if self.surfpool {
			return None;
		}

		match self.genesis_hash.as_str() {
			DEVNET_GENESIS => Some(Cluster::Devnet),
			TESTNET_GENESIS => Some(Cluster::Testnet),
			MAINNET_GENESIS => Some(Cluster::MainnetBeta),
			_ => None,
		}
	}
}

/// Ask a loopback endpoint which cluster it serves.
///
/// The error is a short reason for the plan's warning: a probe that fails
/// leaves the endpoint unidentified, and the caller treats it as unknown.
pub(super) fn probe(rpc_url: &str) -> Result<Identity, String> {
	let client = JsonRpc::new(rpc_url, PROBE_TIMEOUT, true);
	let genesis = client
		.call("getGenesisHash", &json!([]))
		.map_err(|error| format!("getGenesisHash {error}"))?;
	let genesis_hash = genesis_hash(&genesis)
		.ok_or("getGenesisHash returned an invalid hash")?
		.to_owned();
	let version = client
		.call("getVersion", &json!([]))
		.map_err(|error| format!("getVersion {error}"))?;

	Ok(Identity {
		genesis_hash,
		surfpool: version.get("surfnet-version").is_some_and(Value::is_string),
	})
}

/// A `getGenesisHash` result that is a base58 32-byte hash.
pub(super) fn genesis_hash(value: &Value) -> Option<&str> {
	value.as_str().filter(|hash| {
		bs58::decode(hash)
			.into_vec()
			.is_ok_and(|bytes| bytes.len() == 32)
	})
}

#[cfg(test)]
mod tests;
