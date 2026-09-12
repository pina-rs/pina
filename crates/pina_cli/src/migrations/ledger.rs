//! Manifest and publication ledger loading, recording, and validation.

use std::path::Path;

use pina_abi::MANIFEST_PATH;
use pina_abi::MigrationManifest;
use pina_abi::PUBLICATIONS_PATH;
use pina_abi::PendingPublication;
use pina_abi::PublicationLedger;
use pina_abi::PublicationReceipt;
use pina_abi::PublishedContract;
use pina_abi::PublishedSchema;
use serde::Serialize;

use super::MigrationError;
use super::check_project_migrations;
use super::storage::acquire_migration_lock;
use super::storage::hash_regular_file;
use super::storage::hex_digest;
use super::storage::read_bytes;
use super::storage::write_json_atomic;
use crate::project::Project;

/// Persist the exact ABI candidate before a persistent deployment starts.
///
/// Repeating the same attempt is idempotent. A different attempt is rejected
/// until the pending deployment is reconciled, because it may already be live.
pub fn begin_publication(
	start: &Path,
	cluster: &str,
	rpc_url: &str,
	program_id: &str,
	artifact: &Path,
	expected_artifact_digest: [u8; 32],
) -> Result<Option<PendingPublication>, MigrationError> {
	let project = Project::discover(start)?;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let mut ledger = load_publication_ledger(&publication_path)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?;

	if let Some(existing) = &ledger.pending {
		let manifest = manifest.as_ref().ok_or_else(|| {
			MigrationError::InvalidHistory(
				"migration manifest disappeared during publication".to_owned(),
			)
		})?;
		validate_ledger_for_manifest(&ledger, manifest)?;
		let artifact_digest = hash_regular_file(artifact)?;
		if artifact_digest != expected_artifact_digest {
			return Err(MigrationError::PublicationArtifactChanged {
				path: artifact.to_path_buf(),
			});
		}
		if existing.cluster == cluster
			&& existing.rpc_url == rpc_url
			&& existing.program_id == program_id
			&& existing.executable_sha256 == hex_digest(artifact_digest)
		{
			return Ok(Some(existing.clone()));
		}
		return Err(MigrationError::PublicationPending {
			program_id: existing.program_id.clone(),
			cluster: existing.cluster.clone(),
		});
	}

	let statuses = check_project_migrations(&project)?;
	if statuses.is_empty() {
		return Ok(None);
	}
	let manifest = manifest
		.expect("a successful migration check with contracts requires the already-loaded manifest");
	if manifest.program_id != program_id {
		return Err(MigrationError::PublicationProgramMismatch {
			deployed: program_id.to_owned(),
			manifest: manifest.program_id,
		});
	}
	validate_ledger_for_manifest(&ledger, &manifest)?;
	let artifact_digest = hash_regular_file(artifact)?;
	if artifact_digest != expected_artifact_digest {
		return Err(MigrationError::PublicationArtifactChanged {
			path: artifact.to_path_buf(),
		});
	}
	let versions = statuses
		.into_iter()
		.map(|status| {
			let history = manifest
				.contracts
				.get(&status.identity)
				.unwrap_or_else(|| panic!("migration status must name a manifest contract"));
			let pinned = history
				.versions
				.iter()
				.map(|version| {
					PublishedSchema {
						schema_sha256: version.schema_sha256.clone(),
						transition_sha256: version
							.transition
							.as_ref()
							.and_then(|transition| transition.implementation_sha256.clone()),
					}
				})
				.collect();
			(
				status.identity,
				PublishedContract {
					version: status.current_version,
					history: pinned,
				},
			)
		})
		.collect();
	let pending = PendingPublication {
		cluster: cluster.to_owned(),
		rpc_url: rpc_url.to_owned(),
		program_id: program_id.to_owned(),
		executable_sha256: hex_digest(artifact_digest),
		manifest_sha256: manifest.sha256(),
		versions,
		previous_receipt_sha256: ledger.receipts.last().map(PublicationReceipt::sha256),
	};
	ledger.pending = Some(pending.clone());
	validate_ledger_for_manifest(&ledger, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(Some(pending))
}

/// Convert the matching pending deployment into an immutable receipt.
///
/// The pending record remains intact on every error so a remotely successful
/// deployment cannot become a mutable local draft.
pub fn record_publication(
	start: &Path,
	cluster: &str,
	rpc_url: &str,
	deployed_program_id: &str,
	artifact: &Path,
	expected_artifact_digest: [u8; 32],
) -> Result<Option<PublicationReceipt>, MigrationError> {
	let project = Project::discover(start)?;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?.ok_or_else(|| {
		MigrationError::InvalidHistory(
			"migration manifest disappeared during publication".to_owned(),
		)
	})?;
	if manifest.program_id != deployed_program_id {
		return Err(MigrationError::PublicationProgramMismatch {
			deployed: deployed_program_id.to_owned(),
			manifest: manifest.program_id,
		});
	}
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let mut ledger = load_publication_ledger(&publication_path)?;
	validate_ledger_for_manifest(&ledger, &manifest)?;
	let artifact_digest = hash_regular_file(artifact)?;
	if artifact_digest != expected_artifact_digest {
		return Err(MigrationError::PublicationArtifactChanged {
			path: artifact.to_path_buf(),
		});
	}
	let pending = ledger
		.pending
		.clone()
		.ok_or(MigrationError::MissingPendingPublication)?;
	if pending.cluster != cluster
		|| pending.rpc_url != rpc_url
		|| pending.program_id != deployed_program_id
		|| pending.executable_sha256 != hex_digest(artifact_digest)
	{
		return Err(MigrationError::MissingPendingPublication);
	}
	let sequence = u64::try_from(ledger.receipts.len())
		.map_err(|_| MigrationError::PublicationSequenceExhausted)?;
	let receipt = PublicationReceipt {
		sequence,
		cluster: pending.cluster,
		rpc_url: pending.rpc_url,
		program_id: pending.program_id,
		executable_sha256: pending.executable_sha256,
		manifest_sha256: pending.manifest_sha256,
		versions: pending.versions,
		previous_receipt_sha256: pending.previous_receipt_sha256,
		abandoned: false,
	};
	ledger.receipts.push(receipt.clone());
	ledger.pending = None;
	validate_ledger_for_manifest(&ledger, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(Some(receipt))
}

/// Outcome of reconciling a pending deployment.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ReconcileOutput {
	/// No deployment was pending, so there was nothing to reconcile.
	pub no_pending: bool,
	/// The pending deployment was converted into an abandoned receipt.
	pub abandoned: bool,
	/// Cluster of the pending deployment.
	pub cluster: Option<String>,
	/// RPC URL of the pending deployment.
	pub rpc_url: Option<String>,
	/// Program identity of the pending deployment.
	pub program_id: Option<String>,
	/// Executable digest the pending deployment planned to ship.
	pub executable_sha256: Option<String>,
}

/// Inspect or abandon an ambiguous pending deployment.
///
/// Without `abandon`, this reports the exact deployment that must be resumed:
/// rerunning the same cluster, RPC URL, program, and artifact reconciles the
/// pending record into a receipt. With `abandon`, the operator asserts the
/// deployment never went live or accepts the risk; the pending record becomes
/// an abandoned receipt that still freezes its pinned versions because the
/// remote outcome cannot be proven from the local ledger.
pub fn reconcile_publication(
	start: &Path,
	abandon: bool,
) -> Result<ReconcileOutput, MigrationError> {
	let project = Project::discover(start)?;
	let publication_path = project.program_dir.join(PUBLICATIONS_PATH);
	let manifest_path = project.program_dir.join(MANIFEST_PATH);
	let manifest = load_manifest(&manifest_path)?.ok_or_else(|| {
		MigrationError::InvalidHistory(
			"migration manifest disappeared during reconciliation".to_owned(),
		)
	})?;
	let ledger = load_publication_ledger_for_manifest(&publication_path, &manifest)?;
	let Some(pending) = ledger.pending.clone() else {
		return Ok(ReconcileOutput {
			no_pending: true,
			abandoned: false,
			cluster: None,
			rpc_url: None,
			program_id: None,
			executable_sha256: None,
		});
	};

	if !abandon {
		return Ok(ReconcileOutput {
			no_pending: false,
			abandoned: false,
			cluster: Some(pending.cluster),
			rpc_url: Some(pending.rpc_url),
			program_id: Some(pending.program_id),
			executable_sha256: Some(pending.executable_sha256),
		});
	}

	let mut ledger = ledger;
	let _lock = acquire_migration_lock(&project.program_dir)?;
	let sequence = u64::try_from(ledger.receipts.len())
		.map_err(|_| MigrationError::PublicationSequenceExhausted)?;
	let receipt = PublicationReceipt {
		sequence,
		cluster: pending.cluster,
		rpc_url: pending.rpc_url,
		program_id: pending.program_id,
		executable_sha256: pending.executable_sha256,
		manifest_sha256: pending.manifest_sha256,
		versions: pending.versions,
		previous_receipt_sha256: pending.previous_receipt_sha256,
		abandoned: true,
	};
	ledger.receipts.push(receipt);
	ledger.pending = None;
	validate_ledger_for_manifest(&ledger, &manifest)?;
	write_json_atomic(&publication_path, &ledger)?;
	Ok(ReconcileOutput {
		no_pending: false,
		abandoned: true,
		cluster: None,
		rpc_url: None,
		program_id: None,
		executable_sha256: None,
	})
}

pub(super) fn validate_ledger_for_manifest(
	ledger: &PublicationLedger,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	ledger.validate().map_err(MigrationError::InvalidHistory)?;
	for receipt in &ledger.receipts {
		if receipt.program_id != manifest.program_id {
			return Err(MigrationError::InvalidHistory(format!(
				"publication receipt {} belongs to program {}, expected {}",
				receipt.sequence, receipt.program_id, manifest.program_id
			)));
		}
		for (key, published) in &receipt.versions {
			validate_published_contract(
				&format!("publication receipt {}", receipt.sequence),
				key,
				published,
				manifest,
			)?;
		}
	}
	if let Some(pending) = &ledger.pending {
		if pending.program_id != manifest.program_id {
			return Err(MigrationError::InvalidHistory(format!(
				"pending publication belongs to program {}, expected {}",
				pending.program_id, manifest.program_id
			)));
		}
		for (key, candidate) in &pending.versions {
			validate_published_contract("pending publication", key, candidate, manifest)?;
		}
	}
	Ok(())
}

/// Verify one receipt or pending record against the checked-in manifest.
///
/// Every pinned schema and transition must still be present with exactly the
/// recorded hash. A published schema rewritten with consistently recomputed
/// internal hashes therefore still fails the check instead of silently
/// changing how historical account bytes are interpreted.
pub(super) fn validate_published_contract(
	source: &str,
	key: &str,
	published: &PublishedContract,
	manifest: &MigrationManifest,
) -> Result<(), MigrationError> {
	let history = manifest.contracts.get(key).ok_or_else(|| {
		MigrationError::InvalidHistory(format!("{source} names unknown contract `{key}`"))
	})?;
	let current = history.current().ok_or_else(|| {
		MigrationError::InvalidHistory(format!("contract `{key}` has no versions"))
	})?;
	if published.version > current.version {
		return Err(MigrationError::InvalidHistory(format!(
			"{source} claims future version {} for `{key}`",
			published.version
		)));
	}
	if published.history.is_empty() {
		return Ok(());
	}
	for (index, pin) in published.history.iter().enumerate() {
		let version = &history.versions[index];
		if pin.schema_sha256 != version.schema_sha256 {
			return Err(MigrationError::InvalidHistory(format!(
				"{source} pinned schema {} for `{key}` version {index}, but the manifest now 				 \
				 records {}",
				pin.schema_sha256, version.schema_sha256
			)));
		}
		let implementation = version
			.transition
			.as_ref()
			.and_then(|transition| transition.implementation_sha256.as_deref());
		if pin.transition_sha256.as_deref() != implementation {
			return Err(MigrationError::InvalidHistory(format!(
				"{source} pinned a different transition implementation for `{key}` version 				 \
				 {index}"
			)));
		}
	}
	Ok(())
}

pub(crate) fn load_manifest(path: &Path) -> Result<Option<MigrationManifest>, MigrationError> {
	if !path.exists() {
		return Ok(None);
	}
	let source = read_bytes(path)?;
	pina_abi::decode_manifest(&source)
		.map(Some)
		.map_err(|reason| {
			MigrationError::InvalidDocument {
				path: path.to_path_buf(),
				reason,
			}
		})
}

pub(super) fn load_publication_ledger(path: &Path) -> Result<PublicationLedger, MigrationError> {
	if !path.exists() {
		return Ok(PublicationLedger::default());
	}
	let source = read_bytes(path)?;
	pina_abi::decode_publication_ledger(&source).map_err(|reason| {
		MigrationError::InvalidDocument {
			path: path.to_path_buf(),
			reason,
		}
	})
}

/// Load the ledger and fail closed when advanced versions lost their pins.
///
/// Versions beyond zero only exist after a publication, so a manifest with
/// advanced versions and no ledger file means the publication evidence was
/// deleted or never committed. Treating that state as drafts would let
/// `pina migrations make` rewrite published history in place.
pub(super) fn load_publication_ledger_for_manifest(
	path: &Path,
	manifest: &MigrationManifest,
) -> Result<PublicationLedger, MigrationError> {
	let ledger = load_publication_ledger(path)?;
	if !path.exists()
		&& manifest
			.contracts
			.values()
			.any(|history| history.versions.len() > 1)
	{
		return Err(MigrationError::InvalidHistory(
			"the manifest records advanced versions but the publication ledger is missing; 			 \
			 restore migrations/publications.json from version control because published 			 \
			 history must stay pinned"
				.to_owned(),
		));
	}
	Ok(ledger)
}
