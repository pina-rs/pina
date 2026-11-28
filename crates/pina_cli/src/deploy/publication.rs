//! Authenticate publication against the selected RPC, independently of the child.

use std::path::Path;
use std::str::FromStr as _;
use std::time::Duration;

use serde_json::Value;
use serde_json::json;
use sha2::Digest as _;
use sha2::Sha256;
use solana_address::Address;

use super::Cluster;
use super::DeployError;
use super::DeployError::PublicationUnverified;
use super::DeploymentPlan;
use super::identity;
use crate::rehearse::rpc::JsonRpc;
use crate::rehearse::rpc::parse_accounts;

const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const PROGRAMDATA_HEADER_BYTES: usize = 45;

/// Read both loader accounts from one finalized bank. The loader permits
/// zero-filled allocation after the artifact, but no other extra bytes.
pub(super) fn verify(plan: &DeploymentPlan) -> Result<(), DeployError> {
	let program = Address::from_str(plan.program_id())
		.map_err(|_| PublicationUnverified("invalid program address"))?;
	let loader = Address::from_str_const(LOADER);
	let (programdata, _) = Address::find_program_address(&[program.as_ref()], &loader);
	let (authority, authority_digest) =
		super::read_keypair_with_digest(Path::new(plan.upgrade_authority()), "upgrade authority")?;
	let approved_artifact = std::fs::read(plan.program())
		.map_err(|_| PublicationUnverified("could not read the approved artifact"))?;
	let artifact_len = approved_artifact.len();

	if authority_digest != plan.input_fingerprint.upgrade_authority
		|| <[u8; 32]>::from(Sha256::digest(&approved_artifact)) != plan.program_digest()
	{
		return Err(PublicationUnverified("approved deployment inputs changed"));
	}

	let client = JsonRpc::new(
		plan.rpc_url(),
		Duration::from_secs(30),
		plan.target.loopback,
	);
	let genesis = genesis(&client)?;
	let pinned = plan.target.identity.as_ref();
	verify_genesis(
		plan.target.named,
		pinned.map(|identity| identity.genesis_hash.as_str()),
		&genesis,
	)?;
	verify_surfpool(&client, pinned)?;
	let result = client
		.call(
			"getMultipleAccounts",
			&json!([
				[plan.program_id(), programdata.to_string()],
				{ "encoding": "base64", "commitment": "finalized" }
			]),
		)
		.map_err(|_| PublicationUnverified("could not read finalized program accounts"))?;
	let (slot, accounts) = parse_accounts(&result)
		.map_err(|_| PublicationUnverified("malformed finalized account response"))?;
	let [Some(program_account), Some(data_account)] = accounts.as_slice() else {
		return Err(PublicationUnverified(
			"program or ProgramData account is missing",
		));
	};

	if program_account.owner != LOADER
		|| !program_account.executable
		|| program_account.data.len() != 36
		|| program_account.data[..4] != 2_u32.to_le_bytes()
		|| program_account.data[4..] != programdata.to_bytes()
	{
		return Err(PublicationUnverified(
			"program identity or loader state does not match",
		));
	}

	let data = &data_account.data;

	if data_account.owner != LOADER
		|| data_account.executable
		|| data.len() < PROGRAMDATA_HEADER_BYTES
		|| data[..4] != 3_u32.to_le_bytes()
		|| data[12] != 1
		|| data[13..PROGRAMDATA_HEADER_BYTES] != authority[32..]
	{
		return Err(PublicationUnverified(
			"ProgramData loader state or upgrade authority does not match",
		));
	}

	let deployed_slot = u64::from_le_bytes([
		data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
	]);

	if deployed_slot > slot {
		return Err(PublicationUnverified(
			"deployment slot is newer than the finalized bank",
		));
	}

	let artifact = &data[PROGRAMDATA_HEADER_BYTES..];

	if artifact.len() < artifact_len
		|| artifact[artifact_len..].iter().any(|byte| *byte != 0)
		|| <[u8; 32]>::from(Sha256::digest(&artifact[..artifact_len])) != plan.program_digest()
	{
		return Err(PublicationUnverified(
			"deployed artifact does not match the approved digest",
		));
	}

	if genesis != self::genesis(&client)? {
		return Err(PublicationUnverified(
			"RPC cluster changed during verification",
		));
	}
	verify_surfpool(&client, pinned)?;

	Ok(())
}

/// A Surfpool fork reports the genesis hash of the cluster it forks, so
/// only `getVersion` tells it apart from a port now forwarding to that
/// cluster.
fn verify_surfpool(
	client: &JsonRpc,
	pinned: Option<&identity::Identity>,
) -> Result<(), DeployError> {
	let Some(Ok(surfpool)) = pinned.map(|identity| &identity.surfpool) else {
		return Ok(());
	};
	let version = client
		.call("getVersion", &json!([]))
		.map_err(|_| PublicationUnverified("could not read the RPC version"))?;
	if identity::is_surfpool(&version) != *surfpool {
		return Err(PublicationUnverified(
			"RPC endpoint changed between Surfpool and another validator since planning",
		));
	}

	Ok(())
}

fn genesis(client: &JsonRpc) -> Result<Value, DeployError> {
	let value = client
		.call("getGenesisHash", &json!([]))
		.map_err(|_| PublicationUnverified("could not read the RPC genesis hash"))?;
	if identity::genesis_hash(&value).is_none() {
		return Err(PublicationUnverified("invalid RPC genesis hash"));
	}

	Ok(value)
}

/// A loopback endpoint must still report the genesis hash it reported when
/// the plan was prepared, and a named public cluster its own.
fn verify_genesis(
	cluster: Option<Cluster>,
	pinned: Option<&str>,
	actual: &Value,
) -> Result<(), DeployError> {
	let expected = match (pinned, cluster) {
		(Some(pinned), _) => pinned,
		(None, Some(Cluster::Devnet)) => identity::DEVNET_GENESIS,
		(None, Some(Cluster::Testnet)) => identity::TESTNET_GENESIS,
		(None, Some(Cluster::MainnetBeta)) => identity::MAINNET_GENESIS,
		(None, Some(Cluster::Localnet) | None) => return Ok(()),
	};

	if actual != expected {
		return Err(PublicationUnverified(
			"RPC genesis hash does not match the selected cluster",
		));
	}

	Ok(())
}

#[cfg(test)]
mod tests;
