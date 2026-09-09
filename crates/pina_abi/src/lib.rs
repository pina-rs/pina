//! Canonical checked-in ABI history for Pina migrations.
//!
//! The CLI and procedural macros share these types and canonicalization
//! routines. This prevents migration checks from depending on two subtly
//! different interpretations of the same Rust schema.

use std::collections::BTreeMap;
use std::path::PathBuf;

use quote::ToTokens as _;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest as _;
use sha2::Sha256;

/// Relative path of the checked-in migration database.
pub const MANIFEST_PATH: &str = "migrations/manifest.json";

/// Relative path of the append-only publication receipts.
pub const PUBLICATIONS_PATH: &str = "migrations/publications.json";

/// Current serialization format for migration manifests.
///
/// This version belongs to Pina's checked-in ABI document. It is independent
/// from every user contract's on-chain migration version.
pub const MANIFEST_FORMAT_VERSION: u32 = 2;

/// Current serialization format for publication receipts.
pub const PUBLICATION_FORMAT_VERSION: u32 = 2;

/// Stable relative path for one adjacent Rust transition.
#[must_use]
pub fn transition_path(identity: &ContractIdentity, from: u32, to: u32) -> PathBuf {
	let contract = identity.key().replace(':', "_");
	PathBuf::from("migrations")
		.join("transitions")
		.join(contract)
		.join(format!("v{from}_to_v{to}.rs"))
}

/// SHA-256 encoded as lowercase hexadecimal.
#[must_use]
pub fn sha256_bytes(bytes: impl AsRef<[u8]>) -> String {
	hex(&Sha256::digest(bytes.as_ref()))
}

/// Program-wide integer encoding for every migration version envelope.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MigrationVersionType {
	/// One byte, supporting versions 0 through 255.
	#[default]
	U8,
	/// Two bytes, supporting versions 0 through 65,535.
	U16,
	/// Four bytes, supporting every `u32` version.
	U32,
}

impl MigrationVersionType {
	/// Stable configuration spelling.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::U8 => "u8",
			Self::U16 => "u16",
			Self::U32 => "u32",
		}
	}

	/// Number of bytes in the on-chain envelope.
	#[must_use]
	pub const fn bytes(self) -> usize {
		match self {
			Self::U8 => 1,
			Self::U16 => 2,
			Self::U32 => 4,
		}
	}

	/// Largest version representable by this encoding.
	#[must_use]
	pub const fn max_version(self) -> u32 {
		match self {
			Self::U8 => u8::MAX as u32,
			Self::U16 => u16::MAX as u32,
			Self::U32 => u32::MAX,
		}
	}
}

impl std::fmt::Display for MigrationVersionType {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(self.as_str())
	}
}

/// Kind of versioned data contract.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum ContractKind {
	/// Program-owned persisted account data.
	Account,
	/// Data bytes sent to one stable instruction process.
	Instruction,
	/// Immutable data emitted into transaction logs.
	Event,
}

impl ContractKind {
	/// Stable manifest spelling.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Account => "account",
			Self::Instruction => "instruction",
			Self::Event => "event",
		}
	}
}

impl std::fmt::Display for ContractKind {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(self.as_str())
	}
}

/// Physical layout family used by a contract.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LayoutKind {
	/// Fixed-size PinaPod layout.
	Fixed,
	/// Compact PinaPod account with a variable-length tail.
	Compact,
}

/// Identity of a wire contract, independent of its Rust name.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ContractIdentity {
	/// Contract namespace.
	pub kind: ContractKind,
	/// Width of the existing discriminator.
	pub discriminator_bytes: u8,
	/// Discriminator encoded little-endian using exactly `discriminator_bytes`.
	pub discriminator_hex: String,
}

impl ContractIdentity {
	/// Construct and validate one identity.
	pub fn try_new(
		kind: ContractKind,
		discriminator_bytes: usize,
		discriminator: u64,
	) -> Result<Self, String> {
		if !matches!(discriminator_bytes, 1 | 2 | 4 | 8) {
			return Err(format!(
				"discriminator width must be 1, 2, 4, or 8 bytes, got {discriminator_bytes}"
			));
		}

		let encoded = discriminator.to_le_bytes();
		let bytes = &encoded[..discriminator_bytes];
		if discriminator_bytes < encoded.len()
			&& encoded[discriminator_bytes..].iter().any(|byte| *byte != 0)
		{
			return Err(format!(
				"discriminator {discriminator} does not fit in {discriminator_bytes} bytes"
			));
		}

		Ok(Self {
			kind,
			discriminator_bytes: discriminator_bytes as u8,
			discriminator_hex: hex(bytes),
		})
	}

	/// Stable map key for checked-in history.
	#[must_use]
	pub fn key(&self) -> String {
		format!(
			"{}:{}:{}",
			self.kind, self.discriminator_bytes, self.discriminator_hex
		)
	}

	/// Decode the exact little-endian discriminator value.
	pub fn discriminator_value(&self) -> Result<u64, String> {
		let bytes = unhex(&self.discriminator_hex)?;
		if bytes.len() != usize::from(self.discriminator_bytes) {
			return Err(format!(
				"identity `{}` contains {} discriminator bytes, expected {}",
				self.key(),
				bytes.len(),
				self.discriminator_bytes
			));
		}
		let mut encoded = [0_u8; size_of::<u64>()];
		encoded[..bytes.len()].copy_from_slice(&bytes);
		Ok(u64::from_le_bytes(encoded))
	}
}

/// Canonical source schema for one version.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct DataSchema {
	/// Fixed or compact physical encoding.
	pub layout: LayoutKind,
	/// User fields in physical declaration order. Framework envelope fields are omitted.
	pub fields: Vec<FieldSchema>,
}

impl DataSchema {
	/// SHA-256 of the canonical JSON representation.
	#[must_use]
	pub fn sha256(&self) -> String {
		hash_json(self)
	}

	/// Exact payload size for a fixed schema, excluding discriminator and version.
	///
	/// Compact schemas return `None` because their active tail length is dynamic.
	#[must_use]
	pub fn fixed_payload_size(&self) -> Option<usize> {
		if self.layout == LayoutKind::Compact {
			return None;
		}
		self.fields.iter().try_fold(0_usize, |total, field| {
			fixed_type_size(&field.rust_type).and_then(|size| total.checked_add(size))
		})
	}

	/// Return fixed payload offsets in declaration order.
	#[must_use]
	pub fn fixed_field_offsets(&self) -> Option<BTreeMap<String, (usize, usize)>> {
		if self.layout == LayoutKind::Compact {
			return None;
		}
		let mut offset = 0_usize;
		let mut fields = BTreeMap::new();
		for field in &self.fields {
			let size = fixed_type_size(&field.rust_type)?;
			fields.insert(field.name.clone(), (offset, size));
			offset = offset.checked_add(size)?;
		}
		Some(fields)
	}
}

/// One named ABI field.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct FieldSchema {
	/// Wire-significant field name used by automatic transition matching.
	pub name: String,
	/// Canonical closed-grammar type spelling.
	pub rust_type: String,
}

/// Stable instruction account and authorization contract.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ProcessContract {
	/// Positional account slots in transaction order.
	pub accounts: Vec<ProcessAccount>,
}

impl ProcessContract {
	/// SHA-256 of the canonical JSON representation.
	#[must_use]
	pub fn sha256(&self) -> String {
		hash_json(self)
	}
}

/// Prove the account-list relationship supported by Pina's default process
/// compatibility policy.
///
/// Existing slots must remain identical and positional. A destination may
/// append optional slots because an old request can represent each appended
/// value as absent. Every other change fails closed.
pub fn classify_process_transition(
	source: &ProcessContract,
	destination: &ProcessContract,
) -> Result<ProcessTransition, String> {
	let source_accounts = u32::try_from(source.accounts.len())
		.map_err(|_| "source process has too many account slots".to_owned())?;
	let destination_accounts = u32::try_from(destination.accounts.len())
		.map_err(|_| "destination process has too many account slots".to_owned())?;

	if source == destination {
		return Ok(ProcessTransition {
			kind: ProcessTransitionKind::Unchanged,
			source_accounts,
			destination_accounts,
		});
	}
	if destination.accounts.len() >= source.accounts.len()
		&& destination.accounts[..source.accounts.len()] == source.accounts
		&& destination.accounts[source.accounts.len()..]
			.iter()
			.all(|account| account.optional)
	{
		return Ok(ProcessTransition {
			kind: ProcessTransitionKind::AppendOptional,
			source_accounts,
			destination_accounts,
		});
	}

	Err(
		"existing instruction account slots changed, or a newly appended slot is required"
			.to_owned(),
	)
}

/// One positional account in an instruction process.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ProcessAccount {
	/// Stable semantic slot name. Renaming a slot is currently breaking.
	pub name: String,
	/// Privilege expected by the current process.
	pub writable: bool,
	/// Signature expected by the current process.
	pub signer: bool,
	/// Whether an absent value is representable by this process.
	pub optional: bool,
	/// Known address emitted by generated clients, when one exists.
	pub default_value: Option<String>,
	/// Stable Pina PDA identity, when the slot is a generated PDA.
	pub pda: Option<String>,
	/// Canonical declarative validation rules not represented by the fields
	/// above, sorted to make source annotation order irrelevant.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub constraints: Vec<String>,
}

/// Account-list compatibility proved for an adjacent instruction version.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProcessTransitionKind {
	/// The process account ABI is byte-for-byte unchanged.
	Unchanged,
	/// The destination appends only optional slots to the source prefix.
	AppendOptional,
}

/// Frozen proof describing how two adjacent instruction account ABIs relate.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ProcessTransition {
	pub kind: ProcessTransitionKind,
	pub source_accounts: u32,
	pub destination_accounts: u32,
}

/// How a checked-in adjacent transition is implemented.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransitionMode {
	/// Pina proved and generated the complete conversion.
	Automatic,
	/// The developer owns the generated typed transition function body.
	Manual,
}

/// Frozen description of an adjacent schema conversion.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Transition {
	pub from: u32,
	pub to: u32,
	pub mode: TransitionMode,
	pub source_schema_sha256: String,
	pub destination_schema_sha256: String,
	/// Instruction-only source process hash.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub source_process_sha256: Option<String>,
	/// Instruction-only destination process hash.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub destination_process_sha256: Option<String>,
	/// Instruction-only account-list compatibility proof.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub process: Option<ProcessTransition>,
	/// Hash of generated or manual Rust once the version is published.
	pub implementation_sha256: Option<String>,
}

/// One immutable schema version in a contract history.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SchemaVersion {
	pub version: u32,
	pub schema_sha256: String,
	pub schema: DataSchema,
	/// Instruction account ABI for this version. Absent for accounts and events.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub process: Option<ProcessContract>,
	/// Content hash of `process` for instruction versions.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub process_sha256: Option<String>,
	/// Absent only for version zero.
	pub transition: Option<Transition>,
}

/// Complete history for one discriminator identity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ContractHistory {
	pub identity: ContractIdentity,
	/// Current Rust source name. It is not part of stable identity.
	pub rust_name: String,
	pub versions: Vec<SchemaVersion>,
}

impl ContractHistory {
	/// Current schema version.
	#[must_use]
	pub fn current(&self) -> Option<&SchemaVersion> {
		self.versions.last()
	}

	/// Validate ordering, hashes, and adjacent process compatibility proofs.
	pub fn validate(&self, version_type: MigrationVersionType) -> Result<(), String> {
		if self.versions.is_empty() {
			return Err(format!(
				"contract `{}` has no versions",
				self.identity.key()
			));
		}
		for (index, version) in self.versions.iter().enumerate() {
			let expected = index as u32;
			if version.version != expected {
				return Err(format!(
					"contract `{}` expected version {expected}, found {}",
					self.identity.key(),
					version.version
				));
			}
			if version.version > version_type.max_version() {
				return Err(format!(
					"contract `{}` version {} exceeds configured {version_type}",
					self.identity.key(),
					version.version
				));
			}
			if version.schema_sha256 != version.schema.sha256() {
				return Err(format!(
					"contract `{}` version {} schema hash does not match its contents",
					self.identity.key(),
					version.version
				));
			}
			match self.identity.kind {
				ContractKind::Instruction => {
					let process = version.process.as_ref().ok_or_else(|| {
						format!(
							"instruction contract `{}` version {} is missing its process contract",
							self.identity.key(),
							version.version
						)
					})?;
					if version.process_sha256.as_deref() != Some(process.sha256().as_str()) {
						return Err(format!(
							"instruction contract `{}` version {} process hash does not match its \
							 contents",
							self.identity.key(),
							version.version
						));
					}
				}
				ContractKind::Account | ContractKind::Event => {
					if version.process.is_some() || version.process_sha256.is_some() {
						return Err(format!(
							"{} contract `{}` version {} cannot contain an instruction process",
							self.identity.kind,
							self.identity.key(),
							version.version
						));
					}
				}
			}
			match (version.version, &version.transition) {
				(0, None) => {}
				(0, Some(_)) => {
					return Err(format!(
						"contract `{}` version zero cannot have a transition",
						self.identity.key()
					));
				}
				(_, Some(transition)) => {
					let previous = &self.versions[index - 1];
					if transition.from + 1 != transition.to
						|| transition.to != version.version
						|| transition.source_schema_sha256 != previous.schema_sha256
						|| transition.destination_schema_sha256 != version.schema_sha256
					{
						return Err(format!(
							"contract `{}` version {} has an invalid adjacent transition",
							self.identity.key(),
							version.version
						));
					}

					match self.identity.kind {
						ContractKind::Instruction => {
							let source = previous.process.as_ref().expect("validated above");
							let destination = version.process.as_ref().expect("validated above");
							let proof = classify_process_transition(source, destination).map_err(
								|reason| {
									format!(
										"instruction contract `{}` version {} process is \
										 breaking: {reason}",
										self.identity.key(),
										version.version
									)
								},
							)?;
							if transition.source_process_sha256 != previous.process_sha256
								|| transition.destination_process_sha256 != version.process_sha256
								|| transition.process.as_ref() != Some(&proof)
							{
								return Err(format!(
									"instruction contract `{}` version {} has an invalid process \
									 proof",
									self.identity.key(),
									version.version
								));
							}
						}
						ContractKind::Account | ContractKind::Event => {
							if transition.source_process_sha256.is_some()
								|| transition.destination_process_sha256.is_some()
								|| transition.process.is_some()
							{
								return Err(format!(
									"{} contract `{}` version {} cannot contain a process proof",
									self.identity.kind,
									self.identity.key(),
									version.version
								));
							}
						}
					}
				}
				_ => {
					return Err(format!(
						"contract `{}` version {} has an invalid adjacent transition",
						self.identity.key(),
						version.version
					));
				}
			}
		}

		Ok(())
	}
}

/// Checked-in source of truth for every migration-aware data contract.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct MigrationManifest {
	pub format_version: u32,
	pub program_id: String,
	pub version_type: MigrationVersionType,
	pub contracts: BTreeMap<String, ContractHistory>,
}

impl MigrationManifest {
	/// Construct an empty history for one program identity.
	#[must_use]
	pub fn new(program_id: String, version_type: MigrationVersionType) -> Self {
		Self {
			format_version: MANIFEST_FORMAT_VERSION,
			program_id,
			version_type,
			contracts: BTreeMap::new(),
		}
	}

	/// Validate all content-addressed invariants.
	pub fn validate(&self) -> Result<(), String> {
		if self.format_version != MANIFEST_FORMAT_VERSION {
			return Err(format!(
				"unsupported migration manifest format {}; expected {MANIFEST_FORMAT_VERSION}",
				self.format_version
			));
		}
		for (key, history) in &self.contracts {
			if *key != history.identity.key() {
				return Err(format!(
					"migration manifest key `{key}` does not match identity `{}`",
					history.identity.key()
				));
			}
			history.validate(self.version_type)?;
		}
		Ok(())
	}

	/// Content hash embedded into build and publication records.
	#[must_use]
	pub fn sha256(&self) -> String {
		hash_json(self)
	}

	/// Locate the latest source binding used by a procedural macro.
	pub fn contract_for_source(
		&self,
		kind: ContractKind,
		rust_name: &str,
	) -> Result<&ContractHistory, String> {
		let mut matches = self
			.contracts
			.values()
			.filter(|history| history.identity.kind == kind && history.rust_name == rust_name);
		let found = matches.next().ok_or_else(|| {
			format!(
				"{kind} `{rust_name}` is marked `migrations` but has no snapshot; run `pina \
				 migrations make`"
			)
		})?;
		if matches.next().is_some() {
			return Err(format!(
				"migration history contains multiple {kind} contracts bound to `{rust_name}`"
			));
		}
		Ok(found)
	}
}

/// Decode any supported historical Pina ABI document into the current model.
///
/// Pina's document format evolves independently from user contract versions.
/// Readers always normalize the document through every adjacent internal
/// migration before validating hashes or generating program code.
pub fn decode_manifest(source: &[u8]) -> Result<MigrationManifest, String> {
	let mut value: serde_json::Value = serde_json::from_slice(source)
		.map_err(|error| format!("invalid migration manifest JSON: {error}"))?;
	let mut version = document_format_version(&value, "migration manifest")?;
	if version > MANIFEST_FORMAT_VERSION {
		return Err(format!(
			"migration manifest format {version} is newer than supported format \
			 {MANIFEST_FORMAT_VERSION}"
		));
	}
	while version < MANIFEST_FORMAT_VERSION {
		value = migrate_manifest_document(version, value)?;
		version += 1;
	}

	let manifest: MigrationManifest = serde_json::from_value(value)
		.map_err(|error| format!("invalid migration manifest format {version}: {error}"))?;
	manifest.validate()?;
	Ok(manifest)
}

/// Decode and validate the publication ledger format.
pub fn decode_publication_ledger(source: &[u8]) -> Result<PublicationLedger, String> {
	let mut value: serde_json::Value = serde_json::from_slice(source)
		.map_err(|error| format!("invalid publication ledger JSON: {error}"))?;
	let mut version = document_format_version(&value, "publication ledger")?;
	if version > PUBLICATION_FORMAT_VERSION {
		return Err(format!(
			"publication ledger format {version} is newer than supported format \
			 {PUBLICATION_FORMAT_VERSION}"
		));
	}
	while version < PUBLICATION_FORMAT_VERSION {
		value = migrate_publication_document(version, value)?;
		version += 1;
	}
	let ledger: PublicationLedger = serde_json::from_value(value)
		.map_err(|error| format!("invalid publication ledger format {version}: {error}"))?;
	ledger.validate()?;
	Ok(ledger)
}

fn migrate_publication_document(
	version: u32,
	value: serde_json::Value,
) -> Result<serde_json::Value, String> {
	match version {
		1 => migrate_publication_v1_to_v2(value),
		_ => {
			Err(format!(
				"no Pina ABI migration is available from publication format {version}"
			))
		}
	}
}

fn migrate_publication_v1_to_v2(mut value: serde_json::Value) -> Result<serde_json::Value, String> {
	let root = value
		.as_object_mut()
		.ok_or_else(|| "publication ledger must be a JSON object".to_owned())?;
	let receipts = root
		.get_mut("receipts")
		.and_then(serde_json::Value::as_array_mut)
		.ok_or_else(|| "publication ledger is missing its receipts array".to_owned())?;
	for receipt_value in receipts {
		let receipt = receipt_value
			.as_object_mut()
			.ok_or_else(|| "publication ledger contains a non-object receipt".to_owned())?;
		let legacy_cluster = receipt
			.get("cluster")
			.and_then(serde_json::Value::as_str)
			.ok_or_else(|| "publication receipt is missing its cluster".to_owned())?
			.to_owned();
		receipt.insert(
			"rpcUrl".to_owned(),
			serde_json::Value::String(legacy_cluster),
		);
	}
	root.insert(
		"formatVersion".to_owned(),
		serde_json::Value::from(PUBLICATION_FORMAT_VERSION),
	);
	let mut ledger: PublicationLedger = serde_json::from_value(value)
		.map_err(|error| format!("invalid publication ledger format 1: {error}"))?;
	let mut previous = None;
	for receipt in &mut ledger.receipts {
		receipt.previous_receipt_sha256 = previous;
		previous = Some(receipt.sha256());
	}
	serde_json::to_value(ledger)
		.map_err(|error| format!("could not encode publication ledger format 2: {error}"))
}

fn document_format_version(value: &serde_json::Value, kind: &str) -> Result<u32, String> {
	let version = value
		.as_object()
		.and_then(|object| object.get("formatVersion"))
		.and_then(serde_json::Value::as_u64)
		.ok_or_else(|| format!("{kind} is missing an integer `formatVersion`"))?;
	u32::try_from(version).map_err(|_| format!("{kind} format version exceeds u32"))
}

fn migrate_manifest_document(
	version: u32,
	value: serde_json::Value,
) -> Result<serde_json::Value, String> {
	match version {
		1 => migrate_manifest_v1_to_v2(value),
		_ => {
			Err(format!(
				"no Pina ABI migration is available from manifest format {version}"
			))
		}
	}
}

/// Format 1 stored one immutable instruction process on the contract history.
/// Format 2 snapshots the process on every contract version and freezes an
/// adjacent compatibility proof, allowing safe account-list evolution.
fn migrate_manifest_v1_to_v2(mut value: serde_json::Value) -> Result<serde_json::Value, String> {
	let root = value
		.as_object_mut()
		.ok_or_else(|| "migration manifest must be a JSON object".to_owned())?;
	let contracts = root
		.get_mut("contracts")
		.and_then(serde_json::Value::as_object_mut)
		.ok_or_else(|| "migration manifest is missing its `contracts` object".to_owned())?;

	for (key, history_value) in contracts {
		let history = history_value
			.as_object_mut()
			.ok_or_else(|| format!("contract `{key}` history must be an object"))?;
		let is_instruction = history
			.get("identity")
			.and_then(|identity| identity.get("kind"))
			.and_then(serde_json::Value::as_str)
			== Some("instruction");
		let process = history.remove("process");
		let process_sha256 = history.remove("processSha256");
		let process_count = process
			.as_ref()
			.and_then(|process| process.get("accounts"))
			.and_then(serde_json::Value::as_array)
			.map(Vec::len)
			.unwrap_or_default();
		let process_count = u32::try_from(process_count)
			.map_err(|_| format!("contract `{key}` has too many process accounts"))?;
		let versions = history
			.get_mut("versions")
			.and_then(serde_json::Value::as_array_mut)
			.ok_or_else(|| format!("contract `{key}` is missing its `versions` array"))?;

		for version_value in versions {
			let schema_version = version_value
				.as_object_mut()
				.ok_or_else(|| format!("contract `{key}` contains a non-object version"))?;
			if is_instruction {
				if let Some(process) = &process {
					schema_version.insert("process".to_owned(), process.clone());
				}
				if let Some(process_sha256) = &process_sha256 {
					schema_version.insert("processSha256".to_owned(), process_sha256.clone());
				}
				if let Some(transition) = schema_version
					.get_mut("transition")
					.and_then(serde_json::Value::as_object_mut)
				{
					if let Some(process_sha256) = &process_sha256 {
						transition.insert("sourceProcessSha256".to_owned(), process_sha256.clone());
						transition.insert(
							"destinationProcessSha256".to_owned(),
							process_sha256.clone(),
						);
					}
					transition.insert(
						"process".to_owned(),
						serde_json::json!({
							"kind": "unchanged",
							"sourceAccounts": process_count,
							"destinationAccounts": process_count,
						}),
					);
				}
			}
		}
	}
	root.insert(
		"formatVersion".to_owned(),
		serde_json::Value::from(MANIFEST_FORMAT_VERSION),
	);
	Ok(value)
}

/// One successful persistent deployment receipt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PublicationReceipt {
	pub sequence: u64,
	pub cluster: String,
	/// Credential-free RPC endpoint used by the deployment plan.
	pub rpc_url: String,
	pub program_id: String,
	pub executable_sha256: String,
	pub manifest_sha256: String,
	/// Highest version made live for every contract identity.
	pub versions: BTreeMap<String, u32>,
	pub previous_receipt_sha256: Option<String>,
}

impl PublicationReceipt {
	/// Hash chained into the next append-only receipt.
	#[must_use]
	pub fn sha256(&self) -> String {
		hash_json(self)
	}
}

/// Local, hash-chained record of versions that have been made persistent.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PublicationLedger {
	pub format_version: u32,
	pub receipts: Vec<PublicationReceipt>,
}

impl Default for PublicationLedger {
	fn default() -> Self {
		Self {
			format_version: PUBLICATION_FORMAT_VERSION,
			receipts: Vec::new(),
		}
	}
}

impl PublicationLedger {
	/// Return whether a contract version has ever been published persistently.
	#[must_use]
	pub fn ever_published(&self, contract: &str, version: u32) -> bool {
		self.receipts.iter().any(|receipt| {
			receipt
				.versions
				.get(contract)
				.is_some_and(|published| *published >= version)
		})
	}

	/// Validate receipt sequence and hash-chain integrity.
	pub fn validate(&self) -> Result<(), String> {
		if self.format_version != PUBLICATION_FORMAT_VERSION {
			return Err(format!(
				"unsupported publication ledger format {}; expected {PUBLICATION_FORMAT_VERSION}",
				self.format_version
			));
		}
		let mut previous = None;
		let mut published_versions = BTreeMap::<&str, u32>::new();
		for (index, receipt) in self.receipts.iter().enumerate() {
			if receipt.sequence != index as u64 {
				return Err(format!(
					"publication receipt sequence expected {index}, found {}",
					receipt.sequence
				));
			}
			if receipt.previous_receipt_sha256 != previous {
				return Err(format!(
					"publication receipt {} does not extend the previous hash",
					receipt.sequence
				));
			}
			if receipt.cluster.is_empty()
				|| receipt.cluster.chars().any(char::is_control)
				|| receipt.rpc_url.is_empty()
				|| receipt.rpc_url.chars().any(char::is_control)
				|| receipt.program_id.is_empty()
				|| !is_sha256(&receipt.executable_sha256)
				|| !is_sha256(&receipt.manifest_sha256)
				|| receipt.versions.is_empty()
			{
				return Err(format!(
					"publication receipt {} contains invalid identity or hash fields",
					receipt.sequence
				));
			}
			for (contract, version) in &receipt.versions {
				if contract.is_empty() || contract.chars().any(char::is_control) {
					return Err(format!(
						"publication receipt {} contains an invalid contract identity",
						receipt.sequence
					));
				}
				if published_versions
					.get(contract.as_str())
					.is_some_and(|published| version < published)
				{
					return Err(format!(
						"publication receipt {} regresses `{contract}` to version {version}",
						receipt.sequence
					));
				}
				published_versions.insert(contract, *version);
			}
			previous = Some(receipt.sha256());
		}
		Ok(())
	}
}

fn is_sha256(value: &str) -> bool {
	value.len() == 64
		&& value
			.bytes()
			.all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Convert a closed-grammar Rust type into a stable source spelling.
#[must_use]
pub fn canonical_type(ty: &syn::Type) -> String {
	match ty {
		syn::Type::Path(path) => {
			path.path.segments.last().map_or_else(
				|| "unknown".to_owned(),
				|segment| {
					let mut rendered = segment.ident.to_string();
					if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
						let inner = arguments
							.args
							.iter()
							.map(canonical_generic_argument)
							.collect::<Vec<_>>()
							.join(", ");
						rendered = format!("{rendered}<{inner}>");
					}
					rendered
				},
			)
		}
		syn::Type::Array(array) => {
			format!(
				"[{}; {}]",
				canonical_type(&array.elem),
				canonical_const(&array.len)
			)
		}
		other => other.to_token_stream().to_string().replace(' ', ""),
	}
}

/// Exact storage size for Pina's closed fixed-layout type grammar.
#[must_use]
pub fn fixed_type_size(ty: &str) -> Option<usize> {
	match ty.trim() {
		"u8" | "i8" | "bool" | "PodBool" => Some(1),
		"u16" | "i16" | "PodU16" | "PodI16" => Some(2),
		"u32" | "i32" | "PodU32" | "PodI32" => Some(4),
		"u64" | "i64" | "PodU64" | "PodI64" => Some(8),
		"u128" | "i128" | "PodU128" | "PodI128" => Some(16),
		"Address" | "Pubkey" => Some(32),
		other => {
			if let Some((element, length)) = parse_array(other) {
				return fixed_type_size(element)?.checked_mul(length);
			}
			let (name, arguments) = parse_generic(other)?;
			match name {
				"Option" => {
					let [inner] = arguments.as_slice() else {
						return None;
					};
					fixed_type_size(inner)?.checked_add(1)
				}
				"String" => {
					let [capacity] = arguments.as_slice() else {
						return None;
					};
					capacity.parse::<usize>().ok()?.checked_add(1)
				}
				"PodString" => {
					let [capacity, rest @ ..] = arguments.as_slice() else {
						return None;
					};
					let prefix = match rest {
						[] => 1,
						[value] => value.parse::<usize>().ok()?,
						_ => return None,
					};
					capacity.parse::<usize>().ok()?.checked_add(prefix)
				}
				"Vec" | "PodVec" => {
					let [element, capacity, rest @ ..] = arguments.as_slice() else {
						return None;
					};
					let prefix = if name == "Vec" {
						if !rest.is_empty() {
							return None;
						}
						2
					} else {
						match rest {
							[] => 2,
							[value] => value.parse::<usize>().ok()?,
							_ => return None,
						}
					};
					fixed_type_size(element)?
						.checked_mul(capacity.parse::<usize>().ok()?)?
						.checked_add(prefix)
				}
				_ => None,
			}
		}
	}
}

/// Build the data-only schema seen by an attribute macro before envelope fields are injected.
pub fn data_schema(item: &syn::ItemStruct, layout: LayoutKind) -> Result<DataSchema, String> {
	let syn::Fields::Named(fields) = &item.fields else {
		return Err(format!("{} must have named fields", item.ident));
	};
	let fields = fields
		.named
		.iter()
		.map(|field| {
			let name = field
				.ident
				.as_ref()
				.ok_or_else(|| format!("{} contains an unnamed field", item.ident))?
				.to_string();
			Ok(FieldSchema {
				name,
				rust_type: canonical_type(&field.ty),
			})
		})
		.collect::<Result<Vec<_>, String>>()?;

	Ok(DataSchema { layout, fields })
}

fn canonical_generic_argument(argument: &syn::GenericArgument) -> String {
	match argument {
		syn::GenericArgument::Type(ty) => canonical_type(ty),
		syn::GenericArgument::Const(expression) => canonical_const(expression),
		syn::GenericArgument::Lifetime(lifetime) => lifetime.ident.to_string(),
		other => other.to_token_stream().to_string().replace(' ', ""),
	}
}

fn canonical_const(expression: &syn::Expr) -> String {
	match expression {
		syn::Expr::Lit(syn::ExprLit {
			lit: syn::Lit::Int(value),
			..
		}) => value.base10_digits().to_owned(),
		other => other.to_token_stream().to_string().replace(' ', ""),
	}
}

fn parse_array(ty: &str) -> Option<(&str, usize)> {
	let inner = ty.strip_prefix('[')?.strip_suffix(']')?;
	let (element, length) = inner.split_once(';')?;
	Some((element.trim(), length.trim().parse().ok()?))
}

fn parse_generic(ty: &str) -> Option<(&str, Vec<&str>)> {
	let open = ty.find('<')?;
	let close = ty.rfind('>')?;
	if close + 1 != ty.len() {
		return None;
	}
	let mut arguments = Vec::new();
	let mut depth = 0_usize;
	let mut start = open + 1;
	for (relative, character) in ty[open + 1..close].char_indices() {
		let index = open + 1 + relative;
		match character {
			'<' | '[' | '(' => depth = depth.checked_add(1)?,
			'>' | ']' | ')' => depth = depth.checked_sub(1)?,
			',' if depth == 0 => {
				arguments.push(ty[start..index].trim());
				start = index + 1;
			}
			_ => {}
		}
	}
	arguments.push(ty[start..close].trim());
	if arguments.iter().any(|argument| argument.is_empty()) {
		return None;
	}
	Some((ty[..open].trim(), arguments))
}

fn hash_json(value: &impl Serialize) -> String {
	let encoded = serde_json::to_vec(value).expect("serializing Pina ABI model cannot fail");
	sha256_bytes(encoded)
}

fn hex(bytes: &[u8]) -> String {
	const DIGITS: &[u8; 16] = b"0123456789abcdef";
	let mut output = String::with_capacity(bytes.len() * 2);
	for byte in bytes {
		output.push(char::from(DIGITS[usize::from(byte >> 4)]));
		output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
	}
	output
}

fn unhex(value: &str) -> Result<Vec<u8>, String> {
	if !value.len().is_multiple_of(2) {
		return Err(format!("hex value `{value}` has an odd number of digits"));
	}
	value
		.as_bytes()
		.chunks_exact(2)
		.map(|pair| {
			let high = hex_digit(pair[0]).ok_or_else(|| format!("invalid hex value `{value}`"))?;
			let low = hex_digit(pair[1]).ok_or_else(|| format!("invalid hex value `{value}`"))?;
			Ok((high << 4) | low)
		})
		.collect()
}

const fn hex_digit(byte: u8) -> Option<u8> {
	match byte {
		b'0'..=b'9' => Some(byte - b'0'),
		b'a'..=b'f' => Some(byte - b'a' + 10),
		b'A'..=b'F' => Some(byte - b'A' + 10),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn process_account(name: &str, optional: bool) -> ProcessAccount {
		ProcessAccount {
			name: name.to_owned(),
			writable: false,
			signer: false,
			optional,
			default_value: None,
			pda: None,
			constraints: vec![],
		}
	}

	fn process(accounts: Vec<ProcessAccount>) -> ProcessContract {
		ProcessContract { accounts }
	}

	#[test]
	fn identity_uses_exact_little_endian_discriminator() {
		let identity = ContractIdentity::try_new(ContractKind::Account, 2, 0x1234)
			.unwrap_or_else(|error| panic!("identity: {error}"));

		assert_eq!(identity.discriminator_hex, "3412");
		assert_eq!(identity.key(), "account:2:3412");
	}

	#[test]
	fn source_schema_is_stable_across_type_qualification() {
		let first: syn::ItemStruct = syn::parse_quote! {
			struct State { owner: pina::Address, values: pina::Vec<u64, 16> }
		};
		let second: syn::ItemStruct = syn::parse_quote! {
			struct State { owner: Address, values: Vec<u64, 16> }
		};

		assert_eq!(
			data_schema(&first, LayoutKind::Fixed),
			data_schema(&second, LayoutKind::Fixed)
		);
	}

	#[test]
	fn publication_ledger_is_hash_chained() {
		let first = PublicationReceipt {
			sequence: 0,
			cluster: "devnet".to_owned(),
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "a".repeat(64),
			manifest_sha256: "b".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), 0)]),
			previous_receipt_sha256: None,
		};
		let second = PublicationReceipt {
			sequence: 1,
			cluster: "mainnet".to_owned(),
			rpc_url: "https://api.mainnet-beta.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "c".repeat(64),
			manifest_sha256: "d".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), 1)]),
			previous_receipt_sha256: Some(first.sha256()),
		};
		let ledger = PublicationLedger {
			format_version: PUBLICATION_FORMAT_VERSION,
			receipts: vec![first, second],
		};

		assert_eq!(ledger.validate(), Ok(()));
		assert!(ledger.ever_published("account:1:00", 0));
		assert!(ledger.ever_published("account:1:00", 1));
		assert!(!ledger.ever_published("account:1:00", 2));
	}

	#[test]
	fn process_compatibility_accepts_only_an_unchanged_prefix_and_optional_suffix() {
		let original = process(vec![process_account("authority", false)]);
		let unchanged = classify_process_transition(&original, &original).unwrap();
		assert_eq!(unchanged.kind, ProcessTransitionKind::Unchanged);

		let appended = process(vec![
			process_account("authority", false),
			process_account("referrer", true),
		]);
		let proof = classify_process_transition(&original, &appended).unwrap();
		assert_eq!(proof.kind, ProcessTransitionKind::AppendOptional);
		assert_eq!(proof.source_accounts, 1);
		assert_eq!(proof.destination_accounts, 2);

		let required = process(vec![
			process_account("authority", false),
			process_account("treasury", false),
		]);
		assert!(classify_process_transition(&original, &required).is_err());

		let mut escalated = original.clone();
		escalated.accounts[0].signer = true;
		assert!(classify_process_transition(&original, &escalated).is_err());

		let inserted = process(vec![
			process_account("referrer", true),
			process_account("authority", false),
		]);
		assert!(classify_process_transition(&original, &inserted).is_err());
	}

	#[test]
	fn manifest_format_one_upgrades_to_versioned_process_snapshots() {
		let schema = DataSchema {
			layout: LayoutKind::Fixed,
			fields: vec![FieldSchema {
				name: "amount".to_owned(),
				rust_type: "u64".to_owned(),
			}],
		};
		let process = process(vec![process_account("authority", false)]);
		let schema_sha256 = schema.sha256();
		let process_sha256 = process.sha256();
		let identity = ContractIdentity::try_new(ContractKind::Instruction, 1, 4).unwrap();
		let key = identity.key();
		let legacy = serde_json::json!({
			"formatVersion": 1,
			"programId": "program",
			"versionType": "u8",
			"contracts": {
				(key.clone()): {
					"identity": identity,
					"rustName": "Transfer",
					"process": process,
					"processSha256": process_sha256,
					"versions": [
						{
							"version": 0,
							"schemaSha256": schema_sha256,
							"schema": schema,
							"transition": null
						},
						{
							"version": 1,
							"schemaSha256": schema_sha256,
							"schema": schema,
							"transition": {
								"from": 0,
								"to": 1,
								"mode": "automatic",
								"sourceSchemaSha256": schema_sha256,
								"destinationSchemaSha256": schema_sha256,
								"implementationSha256": "implementation"
							}
						}
					]
				}
			}
		});

		let migrated = decode_manifest(&serde_json::to_vec(&legacy).unwrap()).unwrap();
		assert_eq!(migrated.format_version, MANIFEST_FORMAT_VERSION);
		let history = migrated.contracts.get(&key).unwrap();
		assert_eq!(history.versions[0].process.as_ref(), Some(&process));
		assert_eq!(history.versions[1].process.as_ref(), Some(&process));
		assert_eq!(
			history.versions[1]
				.transition
				.as_ref()
				.and_then(|transition| transition.process.as_ref())
				.map(|proof| proof.kind),
			Some(ProcessTransitionKind::Unchanged)
		);
	}

	#[test]
	fn publication_format_one_upgrades_rpc_identity_and_rebuilds_its_hash_chain() {
		#[derive(Serialize)]
		#[serde(rename_all = "camelCase")]
		struct LegacyReceipt<'a> {
			sequence: u64,
			cluster: &'a str,
			program_id: &'a str,
			executable_sha256: String,
			manifest_sha256: String,
			versions: BTreeMap<&'a str, u32>,
			previous_receipt_sha256: Option<String>,
		}

		let first = LegacyReceipt {
			sequence: 0,
			cluster: "devnet",
			program_id: "program",
			executable_sha256: "a".repeat(64),
			manifest_sha256: "b".repeat(64),
			versions: BTreeMap::from([("account:1:00", 0)]),
			previous_receipt_sha256: None,
		};
		let second = LegacyReceipt {
			sequence: 1,
			cluster: "devnet",
			program_id: "program",
			executable_sha256: "c".repeat(64),
			manifest_sha256: "d".repeat(64),
			versions: BTreeMap::from([("account:1:00", 1)]),
			previous_receipt_sha256: Some(hash_json(&first)),
		};
		let legacy = serde_json::json!({
			"formatVersion": 1,
			"receipts": [first, second]
		});
		let ledger = decode_publication_ledger(&serde_json::to_vec(&legacy).unwrap())
			.unwrap_or_else(|error| panic!("decode publication ledger: {error}"));

		assert_eq!(ledger.format_version, PUBLICATION_FORMAT_VERSION);
		assert_eq!(ledger.receipts[0].rpc_url, "devnet");
		assert_eq!(
			ledger.receipts[1].previous_receipt_sha256,
			Some(ledger.receipts[0].sha256())
		);
		assert_eq!(ledger.validate(), Ok(()));
	}

	#[test]
	fn publication_ledger_rejects_version_regression() {
		let first = PublicationReceipt {
			sequence: 0,
			cluster: "devnet".to_owned(),
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "a".repeat(64),
			manifest_sha256: "b".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), 1)]),
			previous_receipt_sha256: None,
		};
		let second = PublicationReceipt {
			sequence: 1,
			cluster: "devnet".to_owned(),
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "c".repeat(64),
			manifest_sha256: "d".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), 0)]),
			previous_receipt_sha256: Some(first.sha256()),
		};
		let ledger = PublicationLedger {
			format_version: PUBLICATION_FORMAT_VERSION,
			receipts: vec![first, second],
		};

		assert!(ledger.validate().unwrap_err().contains("regresses"));
	}

	#[test]
	fn manifest_decoder_rejects_missing_and_future_format_versions() {
		assert!(
			decode_manifest(br#"{"contracts":{}}"#)
				.unwrap_err()
				.contains("formatVersion")
		);
		let future = format!(
			r#"{{"formatVersion":{},"programId":"program","versionType":"u8","contracts":{{}}}}"#,
			MANIFEST_FORMAT_VERSION + 1
		);
		assert!(
			decode_manifest(future.as_bytes())
				.unwrap_err()
				.contains("newer than supported")
		);
	}

	#[test]
	fn current_manifest_rejects_unknown_fields() {
		let mut value = serde_json::to_value(MigrationManifest::new(
			"program".to_owned(),
			MigrationVersionType::U8,
		))
		.unwrap();
		value
			.as_object_mut()
			.unwrap()
			.insert("injected".to_owned(), serde_json::Value::Bool(true));

		assert!(decode_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
	}
}
