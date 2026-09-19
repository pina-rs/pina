//! Canonical checked-in ABI history for Pina migrations.
//!
//! The CLI and procedural macros share these types and canonicalization
//! routines. This prevents migration checks from depending on two subtly
//! different interpretations of the same Rust schema.
//!
//! # Versioning
//!
//! The ABI document carries an `abiVersion` that belongs to Pina itself and is
//! independent of every user contract's on-chain migration version. The value is
//! the `major.minor` committed in `ABI_VERSION`, pinned to this crate's own
//! release line: it advances with a breaking release of this crate and with
//! nothing else, so the document version moves if and only if the document
//! contract moved. See `docs/src/migrations/abi-versioning.md`.

#![allow(missing_docs)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;

use quote::ToTokens as _;
use semver::Version;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeSeq as _;
use sha2::Digest as _;
use sha2::Sha256;

mod consts;

pub use consts::SchemaConsts;

/// Relative path of the checked-in migration database.
pub const MANIFEST_PATH: &str = "migrations/manifest.json";

/// Relative path of the append-only publication receipts.
pub const PUBLICATIONS_PATH: &str = "migrations/publications.json";

/// The ABI document version this build writes and understands.
///
/// The value is a committed one-line file rather than a compile-time derivation
/// of `CARGO_PKG_VERSION`, so it can lead the crate during the window between a
/// shape-changing merge and the release bump that catches the crate up.
pub const ABI_VERSION: &str = include_str!("../ABI_VERSION").trim_ascii_end();

/// The JSON key carrying [`ABI_VERSION`] in both documents.
pub const ABI_VERSION_KEY: &str = "abiVersion";

/// The oldest ABI document version this build can still read.
///
/// Versions at or above this one are normalized through [`ABI_STEPS`]; nothing
/// below it has ever shipped.
pub const ABI_OLDEST_SUPPORTED: &str = "0.20";

/// Parse a document version into a comparable semver value.
///
/// A `major.minor` document version is padded to `major.minor.0` so the
/// committed spelling stays two components while comparisons use the full
/// semver ordering.
pub fn parse_abi_version(value: &str) -> Result<Version, String> {
	let value = value.trim();
	// `semver` requires all three components, so pad the document's
	// `major.minor` spelling rather than rejecting the value the tool writes.
	let padded = match value.split('.').count() {
		2 => format!("{value}.0"),
		_ => value.to_owned(),
	};
	Version::parse(&padded).map_err(|error| format!("invalid ABI version `{value}`: {error}"))
}

/// Parse a document version, rejecting a patch component.
///
/// Document versions are `major.minor` only; a patch component would imply a
/// precision the release rule does not have.
pub fn parse_document_version(value: &str) -> Result<Version, String> {
	let version = parse_abi_version(value)?;
	if version.patch != 0 {
		return Err(format!(
			"ABI version `{value}` must be `major.minor`; a patch component is not meaningful"
		));
	}
	Ok(version)
}

/// The current document version, parsed.
pub fn current_abi_version() -> Version {
	parse_document_version(ABI_VERSION)
		.unwrap_or_else(|error| panic!("committed ABI_VERSION is invalid: {error}"))
}

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
#[derive(
	Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema,
)]
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
#[derive(
	Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, schemars::JsonSchema,
)]
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
	/// Every kind in stable configuration order.
	pub const ALL: [Self; 3] = [Self::Account, Self::Instruction, Self::Event];

	/// Stable `[migrations].auto` spelling for this kind.
	///
	/// The configuration spelling is plural because one entry selects a whole
	/// contract kind, while the manifest keeps the singular [`Self::as_str`]
	/// identity spelling.
	#[must_use]
	pub const fn config_name(self) -> &'static str {
		match self {
			Self::Account => "accounts",
			Self::Instruction => "instructions",
			Self::Event => "events",
		}
	}

	/// Resolve one `[migrations].auto` entry.
	#[must_use]
	pub fn from_config_name(name: &str) -> Option<Self> {
		match name {
			"accounts" => Some(Self::Account),
			"instructions" => Some(Self::Instruction),
			"events" => Some(Self::Event),
			_ => None,
		}
	}

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

/// Valid `[migrations].auto` kind names, rendered for error messages.
pub const AUTO_KIND_NAMES: &str = "`accounts`, `events`, or `instructions`";

/// Program-wide opt-in policy that envelopes whole contract kinds.
///
/// The policy is recorded in the migration manifest, which is the only source
/// procedural macros consult. It serializes as a sorted array of the same
/// plural kind names accepted by `[migrations].auto` in `pina.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationAuto {
	kinds: BTreeSet<ContractKind>,
}

impl MigrationAuto {
	/// An empty policy that leaves opting in to each declaration.
	#[must_use]
	pub fn none() -> Self {
		Self::default()
	}

	/// A policy that envelopes accounts, instructions, and events.
	#[must_use]
	pub fn all() -> Self {
		Self {
			kinds: ContractKind::ALL.into_iter().collect(),
		}
	}

	/// Whether the policy envelopes no kind.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.kinds.is_empty()
	}

	/// Whether the policy envelopes `kind`.
	#[must_use]
	pub fn contains(&self, kind: ContractKind) -> bool {
		self.kinds.contains(&kind)
	}

	/// Add `kind` to the policy, returning whether it was newly inserted.
	pub fn insert(&mut self, kind: ContractKind) -> bool {
		self.kinds.insert(kind)
	}

	/// Add `kind` to the policy without reporting duplicates.
	pub fn add(&mut self, kind: ContractKind) {
		self.kinds.insert(kind);
	}

	/// Every kind in the policy, in stable order.
	pub fn iter(&self) -> impl Iterator<Item = ContractKind> + '_ {
		self.kinds.iter().copied()
	}

	/// Kinds that this policy drops relative to `previous`.
	pub fn removed_since(&self, previous: &Self) -> Vec<ContractKind> {
		previous.kinds.difference(&self.kinds).copied().collect()
	}
}

impl std::fmt::Display for MigrationAuto {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut names = self.kinds.iter().map(|kind| kind.config_name());
		match names.next() {
			Some(first) => formatter.write_str(first)?,
			None => return formatter.write_str("none"),
		}
		for name in names {
			write!(formatter, ", {name}")?;
		}
		Ok(())
	}
}

impl Serialize for MigrationAuto {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut sequence = serializer.serialize_seq(Some(self.kinds.len()))?;
		for kind in &self.kinds {
			sequence.serialize_element(kind.config_name())?;
		}
		sequence.end()
	}
}

impl<'de> Deserialize<'de> for MigrationAuto {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct AutoVisitor;

		impl<'de> serde::de::Visitor<'de> for AutoVisitor {
			type Value = MigrationAuto;

			fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				formatter.write_str("an array of migration kind names")
			}

			fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
			where
				A: serde::de::SeqAccess<'de>,
			{
				let mut auto = MigrationAuto::none();
				while let Some(name) = sequence.next_element::<String>()? {
					let kind = ContractKind::from_config_name(&name).ok_or_else(|| {
						serde::de::Error::custom(format!(
							"unknown migration kind `{name}` in auto policy; expected \
							 {AUTO_KIND_NAMES}"
						))
					})?;
					if !auto.insert(kind) {
						return Err(serde::de::Error::custom(format!(
							"duplicate migration kind `{name}` in auto policy"
						)));
					}
				}
				Ok(auto)
			}
		}

		deserializer.deserialize_seq(AutoVisitor)
	}
}

impl std::fmt::Display for ContractKind {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(self.as_str())
	}
}

// `MigrationAuto` has hand-written serde impls, so its schema is hand-written
// too: the document records a sorted array of the `[migrations].auto` kind
// names, and the schema must describe exactly that.
impl schemars::JsonSchema for MigrationAuto {
	fn schema_name() -> std::borrow::Cow<'static, str> {
		"MigrationAuto".into()
	}

	fn schema_id() -> std::borrow::Cow<'static, str> {
		"pina_abi::MigrationAuto".into()
	}

	fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
		let items = <String as schemars::JsonSchema>::json_schema(generator);
		schemars::json_schema!({
			"type": "array",
			"description": "Contract kinds enveloped without a per-item token, in stable order.",
			"items": items,
			"uniqueItems": true,
		})
	}
}

/// Physical layout family used by a contract.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LayoutKind {
	/// Fixed-size `PinaPod` layout.
	Fixed,
	/// Compact `PinaPod` account with a variable-length tail.
	Compact,
}

/// Frozen physical representation derived from Pina's closed schema grammar.
///
/// Every size and offset excludes the discriminator and migration-version
/// envelope. Generated program code adds that program-specific header before
/// asserting `PinaPod`'s compiled representation.
///
/// This descriptor is **derived, not stored**: a document records the field
/// schema, and [`DataSchema::physical`] recomputes the descriptor on load.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(
	tag = "kind",
	rename_all = "camelCase",
	rename_all_fields = "camelCase"
)]
pub enum PhysicalLayout {
	/// One exact payload with every field stored inline.
	Fixed {
		size: u64,
		fields: Vec<FixedFieldLayout>,
	},
	/// A fixed payload header followed by active bounded tails.
	Compact {
		header_size: u64,
		maximum_size: u64,
		tail_alignment: u64,
		fields: Vec<CompactFieldLayout>,
	},
}

/// Payload-relative location of one fixed field.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct FixedFieldLayout {
	pub name: String,
	pub offset: u64,
	pub size: u64,
}

/// Payload-relative header location and optional tail metadata for one compact
/// field.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct CompactFieldLayout {
	pub name: String,
	pub header_offset: u64,
	pub header_size: u64,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tail: Option<CompactTailLayout>,
}

/// Dynamic compact-tail representation in declaration order.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct CompactTailLayout {
	pub index: u32,
	pub kind: CompactTailKind,
	pub optional: bool,
	pub prefix_bytes: u8,
	pub capacity: u64,
	pub element_size: u64,
	pub maximum_bytes: u64,
}

/// Logical payload encoded by one compact tail.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CompactTailKind {
	String,
	Vector,
}

/// Versioned byte codec whose invariants define a schema snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DataCodec {
	/// `PinaPod` 0.2 fixed and compact wire semantics.
	PinaPodV2,
}

/// Identity of a wire contract, independent of its Rust name.
#[derive(
	Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, schemars::JsonSchema,
)]
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

	/// Validate a decoded identity against its own declared width.
	///
	/// The identity is interpolated into history keys and transition paths, so
	/// a decoded document must prove that `discriminator_hex` is canonical
	/// lowercase hexadecimal of exactly `discriminator_bytes` bytes from the
	/// supported width set. Without this check a tampered manifest can alias
	/// one on-chain discriminator under several keys or escape the migrations
	/// directory through the derived transition path.
	pub fn validate(&self) -> Result<(), String> {
		if !matches!(self.discriminator_bytes, 1 | 2 | 4 | 8) {
			return Err(format!(
				"identity `{}` declares unsupported discriminator width {}; expected 1, 2, 4, or 8",
				self.key(),
				self.discriminator_bytes
			));
		}

		let bytes = unhex(&self.discriminator_hex)?;
		if bytes.len() != usize::from(self.discriminator_bytes) {
			return Err(format!(
				"identity `{}` contains {} discriminator bytes, expected {}",
				self.key(),
				bytes.len(),
				self.discriminator_bytes
			));
		}

		if self.discriminator_hex != hex(&bytes) {
			return Err(format!(
				"identity `{}` must encode its discriminator as canonical lowercase hexadecimal",
				self.key()
			));
		}

		Ok(())
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
///
/// A version stores the facts a reader cannot recompute — the layout family, the
/// fields in physical order, and the codec that defines their wire semantics.
/// The byte-level descriptor is derived on load rather than stored, so a
/// document cannot disagree with its own derivation and a layout change is a
/// deliberate, visible version event instead of a silently reinterpreted cache.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct DataSchema {
	/// Fixed or compact physical encoding.
	pub layout: LayoutKind,
	/// User fields in physical declaration order. Framework envelope fields are omitted.
	pub fields: Vec<FieldSchema>,
	/// Versioned codec used to validate and reconstruct these bytes.
	pub codec: DataCodec,
}

impl DataSchema {
	/// Construct a schema, deriving its physical descriptor.
	pub fn try_new(layout: LayoutKind, fields: Vec<FieldSchema>) -> Result<Self, String> {
		let schema = Self {
			layout,
			fields,
			codec: DataCodec::PinaPodV2,
		};
		// Prove the grammar accepts the schema at construction time, so an
		// unsupported field type fails where it was built rather than on the
		// first reader that asks for a descriptor.
		schema.physical()?;
		Ok(schema)
	}

	/// Verify that the schema is well formed under Pina's current closed grammar.
	pub fn validate(&self) -> Result<(), String> {
		if self.codec != DataCodec::PinaPodV2 {
			return Err("unsupported data codec".to_owned());
		}
		self.physical().map(|_| ())
	}

	/// Derive the complete payload-relative physical descriptor.
	///
	/// This is the single source of truth for every offset, size, and capacity
	/// the generated program code and the ABI layout test read.
	pub fn physical(&self) -> Result<PhysicalLayout, String> {
		physical_layout(self.layout, &self.fields)
	}

	/// SHA-256 of the canonical JSON representation.
	#[must_use]
	pub fn sha256(&self) -> String {
		hash_json(self)
	}

	/// Worst-case payload size, excluding discriminator and version.
	///
	/// Fixed schemas return their exact size; compact schemas return the
	/// `maximum_size` the capacity grammar derives from the declared field
	/// capacities, so a warning can quote an exact worst-case figure.
	#[must_use]
	pub fn maximum_payload_size(&self) -> Option<usize> {
		match self.physical().ok()? {
			PhysicalLayout::Fixed { size, .. } => usize::try_from(size).ok(),
			PhysicalLayout::Compact { maximum_size, .. } => usize::try_from(maximum_size).ok(),
		}
	}

	/// Exact payload size for a fixed schema, excluding discriminator and version.
	///
	/// Compact schemas return `None` because their active tail length is dynamic.
	#[must_use]
	pub fn fixed_payload_size(&self) -> Option<usize> {
		match self.physical().ok()? {
			PhysicalLayout::Fixed { size, .. } => usize::try_from(size).ok(),
			PhysicalLayout::Compact { .. } => None,
		}
	}

	/// Return fixed payload offsets in declaration order.
	#[must_use]
	pub fn fixed_field_offsets(&self) -> Option<BTreeMap<String, (usize, usize)>> {
		let PhysicalLayout::Fixed { fields, .. } = self.physical().ok()? else {
			return None;
		};
		fields
			.iter()
			.map(|field| {
				Some((
					field.name.clone(),
					(
						usize::try_from(field.offset).ok()?,
						usize::try_from(field.size).ok()?,
					),
				))
			})
			.collect()
	}
}

/// One named ABI field.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct FieldSchema {
	/// Wire-significant field name used by automatic transition matching.
	pub name: String,
	/// Canonical closed-grammar type spelling.
	pub rust_type: String,
}

/// Stable instruction account and authorization contract.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
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
///
/// Only wire facts are recorded: the fields here decide whether old bytes and
/// old account lists still parse. Declarative validation rules are program
/// semantics, not wire format, and live in the IDL that clients generate from;
/// recording them here made a validation-only change look like a wire break.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
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
}

/// Account-list compatibility proved for an adjacent instruction version.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProcessTransitionKind {
	/// The process account ABI is byte-for-byte unchanged.
	Unchanged,
	/// The destination appends only optional slots to the source prefix.
	AppendOptional,
}

/// Proof describing how two adjacent instruction account ABIs relate.
///
/// This is re-derived by [`classify_process_transition`] on load rather than
/// stored, so a document cannot claim a compatibility relationship its own
/// account lists do not prove.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ProcessTransition {
	pub kind: ProcessTransitionKind,
	pub source_accounts: u32,
	pub destination_accounts: u32,
}

/// How a checked-in adjacent transition is implemented.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TransitionMode {
	/// Pina proved and generated the complete conversion.
	Automatic,
	/// The developer owns the generated typed transition function body.
	Manual,
}

/// One field rename carried by an adjacent transition.
#[derive(
	Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RenameMapping {
	/// Field name in the source schema whose bytes move to `to`.
	pub from: String,
	/// Field name in the destination schema receiving those bytes.
	pub to: String,
}

/// Description of an adjacent schema conversion.
///
/// A transition sits on version `index` and describes the step from `index - 1`.
/// Adjacency, the neighbouring schema and process hashes, and the account-list
/// proof are all derived on load rather than stored as a second copy of the
/// same invariant.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Transition {
	pub mode: TransitionMode,
	/// Disambiguated field renames answered through `pina migrations make`.
	/// Renames record source intent so repeated runs stay stable and the
	/// generated transition moves bytes instead of dropping them.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub renames: Vec<RenameMapping>,
	/// Hash of generated or manual Rust once the version is published.
	pub implementation_sha256: Option<String>,
}

/// One immutable schema version in a contract history.
///
/// The version number is this entry's position in the history, so it is not
/// stored. Schema and process hashes are computed from the decoded content, so
/// they are not stored either; publication receipts compute their pins from the
/// same functions when a deployment is recorded.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SchemaVersion {
	pub schema: DataSchema,
	/// Instruction account ABI for this version. Absent for accounts and events.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub process: Option<ProcessContract>,
	/// Absent only for version zero.
	pub transition: Option<Transition>,
}

impl SchemaVersion {
	/// SHA-256 of this version's canonical schema representation.
	#[must_use]
	pub fn schema_sha256(&self) -> String {
		self.schema.sha256()
	}

	/// SHA-256 of this version's instruction process, when it has one.
	#[must_use]
	pub fn process_sha256(&self) -> Option<String> {
		self.process.as_ref().map(ProcessContract::sha256)
	}
}

/// Complete history for one discriminator identity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ContractHistory {
	pub identity: ContractIdentity,
	/// Current Rust source name. It is not part of stable identity.
	pub rust_name: String,
	pub versions: Vec<SchemaVersion>,
}

impl ContractHistory {
	/// Current schema version number: the last entry's index.
	#[must_use]
	pub fn current_version(&self) -> Option<u32> {
		self.versions
			.len()
			.checked_sub(1)
			.and_then(|index| u32::try_from(index).ok())
	}

	/// Current schema version.
	#[must_use]
	pub fn current(&self) -> Option<&SchemaVersion> {
		self.versions.last()
	}

	/// Resolve a version number to the entry at that position.
	#[must_use]
	pub fn version(&self, number: u32) -> Option<&SchemaVersion> {
		usize::try_from(number)
			.ok()
			.and_then(|index| self.versions.get(index))
	}

	/// The transition entering version `number`.
	#[must_use]
	pub fn transition_into(&self, number: u32) -> Option<&Transition> {
		self.version(number)?.transition.as_ref()
	}

	/// Re-derive the process proof for the transition entering `number`.
	pub fn process_transition_into(
		&self,
		number: u32,
	) -> Result<Option<ProcessTransition>, String> {
		if self.transition_into(number).is_none() || number == 0 {
			return Ok(None);
		}
		let previous = self
			.version(number - 1)
			.ok_or_else(|| format!("version {number} has no predecessor"))?;
		let current = self
			.version(number)
			.ok_or_else(|| format!("version {number} does not exist"))?;
		match (previous.process.as_ref(), current.process.as_ref()) {
			(Some(source), Some(destination)) => {
				classify_process_transition(source, destination).map(Some)
			}
			_ => Ok(None),
		}
	}

	/// Validate ordering, hashes, and adjacent process compatibility proofs.
	pub fn validate(&self, version_type: MigrationVersionType) -> Result<(), String> {
		if self.versions.is_empty() {
			return Err(format!(
				"contract `{}` has no versions",
				self.identity.key()
			));
		}
		self.identity.validate().map_err(|reason| {
			format!(
				"contract `{}` has an invalid identity: {reason}",
				self.identity.key()
			)
		})?;
		for (index, version) in self.versions.iter().enumerate() {
			let number = u32::try_from(index).map_err(|_| {
				format!(
					"contract `{}` has more versions than u32 can index",
					self.identity.key()
				)
			})?;
			if number > version_type.max_version() {
				return Err(format!(
					"contract `{}` version {number} exceeds configured {version_type}",
					self.identity.key()
				));
			}
			version.schema.validate().map_err(|reason| {
				format!(
					"contract `{}` version {number} has an invalid data schema: {reason}",
					self.identity.key()
				)
			})?;
			match self.identity.kind {
				ContractKind::Instruction => {
					if version.schema.layout != LayoutKind::Fixed {
						return Err(format!(
							"instruction contract `{}` version {number} must use a fixed layout",
							self.identity.key()
						));
					}
					if version.process.is_none() {
						return Err(format!(
							"instruction contract `{}` version {number} is missing its process \
							 contract",
							self.identity.key()
						));
					}
				}
				ContractKind::Account | ContractKind::Event => {
					if self.identity.kind == ContractKind::Event
						&& version.schema.layout != LayoutKind::Fixed
					{
						return Err(format!(
							"event contract `{}` version {number} must use a fixed layout",
							self.identity.key()
						));
					}
					if version.process.is_some() {
						return Err(format!(
							"{} contract `{}` version {number} cannot contain an instruction \
							 process",
							self.identity.kind,
							self.identity.key()
						));
					}
				}
			}
			match (number, &version.transition) {
				(0, None) => {}
				(0, Some(_)) => {
					return Err(format!(
						"contract `{}` version zero cannot have a transition",
						self.identity.key()
					));
				}
				(_, None) => {
					return Err(format!(
						"contract `{}` version {number} has no adjacent transition",
						self.identity.key()
					));
				}
				(_, Some(transition)) => {
					let previous = &self.versions[index - 1];
					let previous_fields = previous
						.schema
						.fields
						.iter()
						.map(|field| field.name.as_str())
						.collect::<BTreeSet<_>>();
					let destination_fields = version
						.schema
						.fields
						.iter()
						.map(|field| field.name.as_str())
						.collect::<BTreeSet<_>>();
					for rename in &transition.renames {
						if !previous_fields.contains(rename.from.as_str()) {
							return Err(format!(
								"contract `{}` version {number} renames unknown source field `{}`",
								self.identity.key(),
								rename.from
							));
						}
						if !destination_fields.contains(rename.to.as_str()) {
							return Err(format!(
								"contract `{}` version {number} renames into unknown destination \
								 field `{}`",
								self.identity.key(),
								rename.to
							));
						}
					}

					if self.identity.kind == ContractKind::Instruction {
						let source = previous.process.as_ref().unwrap_or_else(|| {
							panic!("validated above: instruction source process")
						});
						let destination = version.process.as_ref().unwrap_or_else(|| {
							panic!("validated above: instruction destination process")
						});
						classify_process_transition(source, destination).map_err(|reason| {
							format!(
								"instruction contract `{}` version {number} process is breaking: \
								 {reason}",
								self.identity.key()
							)
						})?;
					}
				}
			}
		}

		Ok(())
	}
}

/// Checked-in source of truth for every migration-aware data contract.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct MigrationManifest {
	/// The ABI document version that wrote this manifest.
	pub abi_version: String,
	pub program_id: String,
	pub version_type: MigrationVersionType,
	/// Kinds whose declarations are enveloped without a per-item token.
	#[serde(default, skip_serializing_if = "MigrationAuto::is_empty")]
	pub auto: MigrationAuto,
	pub contracts: BTreeMap<String, ContractHistory>,
}

impl MigrationManifest {
	/// Construct an empty history for one program identity.
	#[must_use]
	pub fn new(program_id: String, version_type: MigrationVersionType) -> Self {
		Self {
			abi_version: ABI_VERSION.to_owned(),
			program_id,
			version_type,
			auto: MigrationAuto::none(),
			contracts: BTreeMap::new(),
		}
	}

	/// Validate all content-addressed invariants.
	pub fn validate(&self) -> Result<(), String> {
		validate_document_version("migration manifest", &self.abi_version)?;
		self.validate_contracts()
	}

	/// Validate every contract independently of the document version.
	pub fn validate_contracts(&self) -> Result<(), String> {
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
				"{kind} `{rust_name}` is opted into migrations but has no snapshot; run `pina \
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

/// Reject a document version the running build cannot read.
pub fn validate_document_version(kind: &str, version: &str) -> Result<(), String> {
	let found = parse_document_version(version)?;
	let supported = current_abi_version();
	if found > supported {
		return Err(format!(
			"{kind} records ABI version {found}, but this Pina build supports {supported}; \
			 upgrade Pina to read it"
		));
	}
	Ok(())
}

/// A document converter between two adjacent ABI versions.
pub type AbiConverter = fn(serde_json::Value) -> Result<serde_json::Value, String>;

/// One adjacent converter: normalize a document from `from` into `to`.
///
/// The table is ordered oldest-first and is walked forward one step at a time.
/// Edges are never deleted once shipped; fixing a defective converter means a
/// new version, never editing published history.
pub struct AbiStep {
	/// The version this step converts away from.
	pub from: &'static str,
	/// The version this step produces.
	pub to: &'static str,
	/// Convert a document body between the two versions.
	pub convert: AbiConverter,
}

/// Every adjacent ABI document step, oldest first.
///
/// A step exists only between two *distinct* delivered shapes: the reset
/// baseline is [`ABI_OLDEST_SUPPORTED`], so the table is empty until a later
/// release changes the document shape. The `abi_version_guards` tests require
/// each step to advance the version and to continue from the previous one, so
/// the walk can never reach a version it cannot traverse.
pub const ABI_STEPS: &[AbiStep] = &[];

/// A step that validates the document and returns it unchanged.
///
/// Used when the ABI version advanced for a reason that did not change the
/// document shape. The step still exists so the walk never has to special-case
/// a version it cannot traverse.
pub fn identity_step(value: serde_json::Value) -> Result<serde_json::Value, String> {
	Ok(value)
}

/// Normalize a decoded document body to the current ABI version.
///
/// The caller supplies the document's own version and the value as decoded; the
/// returned value is ready for the current typed model.
pub fn walk_document(
	kind: &str,
	from: &str,
	value: serde_json::Value,
) -> Result<serde_json::Value, String> {
	walk_document_to(
		kind,
		from,
		value,
		ABI_STEPS,
		ABI_OLDEST_SUPPORTED,
		ABI_VERSION,
	)
}

/// Normalize a document body toward an explicit target version and step table.
///
/// Split out with the versions injectable so the guards can prove how the walk
/// behaves when the table has a hole — a state the shipped table must never
/// reach, and one that cannot be constructed against a single shipped version.
fn walk_document_to(
	kind: &str,
	from: &str,
	mut value: serde_json::Value,
	steps: &[AbiStep],
	oldest_version: &str,
	target_version: &str,
) -> Result<serde_json::Value, String> {
	let start = parse_document_version(from)?;
	let oldest = parse_document_version(oldest_version)?;
	let current = parse_document_version(target_version)?;
	if start > current {
		return Err(format!(
			"{kind} records ABI version {start}, but this Pina build supports {current}; upgrade \
			 Pina to read it"
		));
	}
	if start < oldest {
		return Err(format!(
			"{kind} records ABI version {start}, which predates the oldest supported version \
			 {oldest}; regenerate it with `pina migrations make`"
		));
	}
	if start == current {
		return Ok(value);
	}

	let mut position = start;
	for step in steps {
		let step_from = parse_document_version(step.from)?;
		let step_to = parse_document_version(step.to)?;
		if position == step_from {
			value = (step.convert)(value)?;
			position = step_to;
			if position >= current {
				return Ok(value);
			}
		}
	}

	if position != current {
		return Err(format!(
			"{kind} cannot be read: no ABI converter reaches version {position}"
		));
	}
	Ok(value)
}

/// Read the `abiVersion` key from a decoded document body.
fn document_abi_version(kind: &str, value: &serde_json::Value) -> Result<String, String> {
	value
		.as_object()
		.and_then(|object| object.get(ABI_VERSION_KEY))
		.and_then(serde_json::Value::as_str)
		.map(str::to_owned)
		.ok_or_else(|| format!("{kind} is missing a string `{ABI_VERSION_KEY}` field"))
}

/// Decode any supported historical Pina ABI document into the current model.
///
/// Pina's document format evolves with the crate's own release line. Readers
/// normalize the document through every adjacent converter before validating
/// invariants or generating program code.
pub fn decode_manifest(source: &[u8]) -> Result<MigrationManifest, String> {
	let value: serde_json::Value = serde_json::from_slice(source)
		.map_err(|error| format!("invalid migration manifest JSON: {error}"))?;
	let version = document_abi_version("migration manifest", &value)?;
	let normalized = walk_document("migration manifest", &version, value)?;
	let manifest: MigrationManifest = serde_json::from_value(normalized)
		.map_err(|error| format!("invalid migration manifest {version}: {error}"))?;
	manifest.validate()?;
	Ok(manifest)
}

/// Decode and validate the publication ledger.
pub fn decode_publication_ledger(source: &[u8]) -> Result<PublicationLedger, String> {
	let value: serde_json::Value = serde_json::from_slice(source)
		.map_err(|error| format!("invalid publication ledger JSON: {error}"))?;
	let version = document_abi_version("publication ledger", &value)?;
	let normalized = walk_document("publication ledger", &version, value)?;
	let ledger: PublicationLedger = serde_json::from_value(normalized)
		.map_err(|error| format!("invalid publication ledger {version}: {error}"))?;
	ledger.validate()?;
	Ok(ledger)
}

/// Encode one validated current manifest as canonical JSON.
pub fn encode_manifest(manifest: &MigrationManifest) -> Result<Vec<u8>, String> {
	manifest.validate()?;
	serde_json::to_vec_pretty(manifest)
		.map_err(|error| format!("could not encode migration manifest: {error}"))
}

/// Encode one validated current ledger as canonical JSON.
pub fn encode_publication_ledger(ledger: &PublicationLedger) -> Result<Vec<u8>, String> {
	ledger.validate()?;
	serde_json::to_vec_pretty(ledger)
		.map_err(|error| format!("could not encode publication ledger: {error}"))
}

/// One published schema pinned by a receipt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PublishedSchema {
	/// Content hash of the published schema.
	pub schema_sha256: String,
	/// Hash of the adjacent transition implementation entering this schema.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub transition_sha256: Option<String>,
}

/// One contract's published state as frozen by a receipt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PublishedContract {
	/// Highest version made live for the contract.
	///
	/// Redundant with `history.len() - 1` for a pinned receipt, but retained so
	/// an unpinned receipt can still name the version it made live.
	pub version: u32,
	/// Schema and transition pins for every version up to and including
	/// `version`. Empty for receipts whose history cannot be reconstructed.
	pub history: Vec<PublishedSchema>,
}

impl PublishedContract {
	/// Construct an unpinned entry.
	#[must_use]
	pub fn legacy(version: u32) -> Self {
		Self {
			version,
			history: Vec::new(),
		}
	}

	/// Return the pinned highest published version.
	#[must_use]
	pub const fn version(&self) -> u32 {
		self.version
	}

	/// Return the pinned history, empty for unpinned receipts.
	#[must_use]
	pub fn history(&self) -> &[PublishedSchema] {
		&self.history
	}
}

/// One successful persistent deployment receipt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PublicationReceipt {
	pub sequence: u64,
	/// Credential-free RPC endpoint used by the deployment plan.
	pub rpc_url: String,
	pub program_id: String,
	pub executable_sha256: String,
	pub manifest_sha256: String,
	/// Highest version made live for every contract identity, with the schema
	/// history those deployments froze.
	pub versions: BTreeMap<String, PublishedContract>,
	pub previous_receipt_sha256: Option<String>,
	/// Marks a receipt created by reconciling an ambiguous deployment as
	/// abandoned. The pinned versions stay frozen because the deployment may
	/// still have gone live.
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub abandoned: bool,
}

impl PublicationReceipt {
	/// Hash chained into the next append-only receipt.
	#[must_use]
	pub fn sha256(&self) -> String {
		hash_json(self)
	}
}

/// Recoverable record written before a persistent deployment starts.
///
/// A pending record means the named versions may already be live. Pina freezes
/// them until the exact deployment is completed into a receipt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PendingPublication {
	/// Cluster label used to reconcile the in-flight deployment.
	///
	/// Not part of the immutable receipt: it exists only on this transient local
	/// record so `pina deploy` can report and match the pending target.
	pub cluster: String,
	/// Credential-free RPC endpoint used by the deployment plan.
	pub rpc_url: String,
	pub program_id: String,
	pub executable_sha256: String,
	pub manifest_sha256: String,
	/// Highest version that the in-flight deployment may make live, with the
	/// schema history the deployment freezes.
	pub versions: BTreeMap<String, PublishedContract>,
	pub previous_receipt_sha256: Option<String>,
}

/// Local, hash-chained record of versions that have been made persistent.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct PublicationLedger {
	/// The ABI document version that wrote this ledger.
	pub abi_version: String,
	pub receipts: Vec<PublicationReceipt>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub pending: Option<PendingPublication>,
}

impl Default for PublicationLedger {
	fn default() -> Self {
		Self {
			abi_version: ABI_VERSION.to_owned(),
			receipts: Vec::new(),
			pending: None,
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
				.is_some_and(|published| published.version >= version)
		})
	}

	/// Return whether a published or possibly-live deployment freezes a version.
	#[must_use]
	pub fn version_is_frozen(&self, contract: &str, version: u32) -> bool {
		self.ever_published(contract, version)
			|| self.pending.as_ref().is_some_and(|pending| {
				pending
					.versions
					.get(contract)
					.is_some_and(|candidate| candidate.version >= version)
			})
	}

	/// Validate receipt sequence and hash-chain integrity.
	pub fn validate(&self) -> Result<(), String> {
		validate_document_version("publication ledger", &self.abi_version)?;
		let mut previous = None;
		let mut published_versions = BTreeMap::<String, u32>::new();
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
			validate_publication_identity(
				receipt.sequence,
				&receipt.program_id,
				&receipt.executable_sha256,
				&receipt.manifest_sha256,
				&receipt.versions,
			)?;
			if receipt.rpc_url.is_empty() || receipt.rpc_url.chars().any(char::is_control) {
				return Err(format!(
					"publication receipt {} contains an invalid RPC URL",
					receipt.sequence
				));
			}
			validate_publication_versions(
				receipt.sequence,
				&receipt.versions,
				&mut published_versions,
			)?;
			previous = Some(receipt.sha256());
		}
		if let Some(pending) = &self.pending {
			let sequence = self.receipts.len() as u64;
			if pending.previous_receipt_sha256 != previous {
				return Err(
					"pending publication does not extend the previous receipt hash".to_owned(),
				);
			}
			validate_publication_identity(
				sequence,
				&pending.program_id,
				&pending.executable_sha256,
				&pending.manifest_sha256,
				&pending.versions,
			)?;
			if pending.rpc_url.is_empty() || pending.rpc_url.chars().any(char::is_control) {
				return Err("pending publication contains an invalid RPC URL".to_owned());
			}
			validate_publication_versions(sequence, &pending.versions, &mut published_versions)?;
		}
		Ok(())
	}
}

fn validate_publication_identity(
	sequence: u64,
	program_id: &str,
	executable_sha256: &str,
	manifest_sha256: &str,
	versions: &BTreeMap<String, PublishedContract>,
) -> Result<(), String> {
	if program_id.is_empty()
		|| !is_sha256(executable_sha256)
		|| !is_sha256(manifest_sha256)
		|| versions.is_empty()
	{
		return Err(format!(
			"publication receipt {sequence} contains invalid identity or hash fields"
		));
	}
	Ok(())
}

fn validate_publication_versions(
	sequence: u64,
	versions: &BTreeMap<String, PublishedContract>,
	published_versions: &mut BTreeMap<String, u32>,
) -> Result<(), String> {
	for (contract, published) in versions {
		if contract.is_empty() || contract.chars().any(char::is_control) {
			return Err(format!(
				"publication receipt {sequence} contains an invalid contract identity"
			));
		}
		validate_published_history(sequence, contract, published)?;
		if published_versions
			.get(contract)
			.is_some_and(|previous| published.version < *previous)
		{
			return Err(format!(
				"publication receipt {sequence} regresses `{contract}` to version {}",
				published.version
			));
		}
		published_versions.insert(contract.clone(), published.version);
	}
	Ok(())
}

/// Validate one receipt's pinned schema history.
///
/// A pinned history covers every version up to and including the published
/// version. The first entry never has an entering transition. Empty histories
/// are unpinned receipts and stay unpinned.
fn validate_published_history(
	sequence: u64,
	contract: &str,
	published: &PublishedContract,
) -> Result<(), String> {
	let expected = usize::try_from(published.version)
		.ok()
		.and_then(|version| version.checked_add(1));
	if !published.history.is_empty() && Some(published.history.len()) != expected {
		return Err(format!(
			"publication receipt {sequence} pins {} history entries for `{contract}` at version {}",
			published.history.len(),
			published.version
		));
	}
	for (index, pin) in published.history.iter().enumerate() {
		if !is_sha256(&pin.schema_sha256) {
			return Err(format!(
				"publication receipt {sequence} pins an invalid schema hash for `{contract}` \
				 version {index}"
			));
		}
		if let Some(transition) = &pin.transition_sha256
			&& !is_sha256(transition)
		{
			return Err(format!(
				"publication receipt {sequence} pins an invalid transition hash for `{contract}` \
				 version {index}"
			));
		}
	}
	if published
		.history
		.first()
		.is_some_and(|pin| pin.transition_sha256.is_some())
	{
		return Err(format!(
			"publication receipt {sequence} pins a transition entering version zero of \
			 `{contract}`"
		));
	}
	Ok(())
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

/// Maximum nested generic depth accepted by the fixed-layout type grammar.
///
/// Real schemas stay within a handful of levels; the bound keeps hostile type
/// strings in a decoded manifest from driving unbounded recursion.
const MAX_TYPE_NESTING_DEPTH: usize = 32;

/// Exact storage size for Pina's closed fixed-layout type grammar.
#[must_use]
pub fn fixed_type_size(ty: &str) -> Option<usize> {
	fixed_type_size_at_depth(ty, 0)
}

fn fixed_type_size_at_depth(ty: &str, depth: usize) -> Option<usize> {
	if depth > MAX_TYPE_NESTING_DEPTH {
		return None;
	}
	match ty.trim() {
		"u8" | "i8" | "bool" | "PodBool" => Some(1),
		"u16" | "i16" | "PodU16" | "PodI16" => Some(2),
		"u32" | "i32" | "PodU32" | "PodI32" | "f32" => Some(4),
		"u64" | "i64" | "PodU64" | "PodI64" | "f64" => Some(8),
		"u128" | "i128" | "PodU128" | "PodI128" => Some(16),
		"Address" => Some(32),
		other => {
			if let Some((element, length)) = parse_array(other) {
				// Element size composes: `[u8; N]` stays `N` because `u8`
				// measures one byte, and typed arrays multiply their element
				// size exactly like the compiler lays them out.
				return fixed_type_size_at_depth(element, depth + 1)?.checked_mul(length);
			}
			let (name, arguments) = parse_generic(other)?;
			match name {
				"Option" => {
					let [inner] = arguments.as_slice() else {
						return None;
					};
					fixed_type_size_at_depth(inner, depth + 1)?.checked_add(1)
				}
				"String" => {
					let [capacity] = arguments.as_slice() else {
						return None;
					};
					let capacity = capacity.parse::<usize>().ok()?;
					validate_compact_prefix(capacity, 1).ok()?;
					capacity.checked_add(1)
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
					let capacity = capacity.parse::<usize>().ok()?;
					validate_compact_prefix(capacity, prefix).ok()?;
					capacity.checked_add(prefix)
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
					let capacity = capacity.parse::<usize>().ok()?;
					validate_compact_prefix(capacity, prefix).ok()?;
					fixed_type_size_at_depth(element, depth + 1)?
						.checked_mul(capacity)?
						.checked_add(prefix)
				}
				name => {
					match fixed_point_storage_size(name) {
						// The single generic argument is the fractional-bits
						// parameter; storage depends only on the backing width.
						Some(size) if arguments.len() == 1 => Some(size),
						_ => None,
					}
				}
			}
		}
	}
}

/// Storage size of a `fixed` crate schema type by its backing integer width.
fn fixed_point_storage_size(name: &str) -> Option<usize> {
	match name {
		"FixedI8" | "FixedU8" => Some(1),
		"FixedI16" | "FixedU16" => Some(2),
		"FixedI32" | "FixedU32" => Some(4),
		"FixedI64" | "FixedU64" => Some(8),
		"FixedI128" | "FixedU128" => Some(16),
		_ => None,
	}
}

fn physical_layout(layout: LayoutKind, fields: &[FieldSchema]) -> Result<PhysicalLayout, String> {
	match layout {
		LayoutKind::Fixed => {
			let mut offset = 0_usize;
			let mut physical_fields = Vec::with_capacity(fields.len());
			for field in fields {
				let size = fixed_type_size(&field.rust_type).ok_or_else(|| {
					format!(
						"field `{}` uses unsupported fixed ABI type `{}`",
						field.name, field.rust_type
					)
				})?;
				physical_fields.push(FixedFieldLayout {
					name: field.name.clone(),
					offset: u64::try_from(offset)
						.map_err(|_| "fixed field offset exceeds u64".to_owned())?,
					size: u64::try_from(size)
						.map_err(|_| "fixed field size exceeds u64".to_owned())?,
				});
				offset = offset
					.checked_add(size)
					.ok_or_else(|| "fixed payload size overflowed".to_owned())?;
			}
			Ok(PhysicalLayout::Fixed {
				size: u64::try_from(offset)
					.map_err(|_| "fixed payload size exceeds u64".to_owned())?,
				fields: physical_fields,
			})
		}
		LayoutKind::Compact => compact_physical_layout(fields),
	}
}

#[derive(Clone, Copy)]
struct ParsedCompactTail {
	kind: CompactTailKind,
	optional: bool,
	prefix_bytes: usize,
	capacity: usize,
	element_size: usize,
}

fn compact_physical_layout(fields: &[FieldSchema]) -> Result<PhysicalLayout, String> {
	let mut header_size = 0_usize;
	let mut maximum_tail_size = 0_usize;
	let mut tail_alignment = 0_usize;
	let mut tail_index = 0_u32;
	let mut seen_tail = false;
	let mut physical_fields = Vec::with_capacity(fields.len());

	for field in fields {
		let header_offset = header_size;
		let Some(tail) = parse_compact_tail(&field.rust_type)? else {
			if seen_tail {
				return Err(format!(
					"compact inline field `{}` cannot follow a dynamic tail",
					field.name
				));
			}
			let size = fixed_type_size(&field.rust_type).ok_or_else(|| {
				format!(
					"field `{}` uses unsupported compact inline ABI type `{}`",
					field.name, field.rust_type
				)
			})?;
			header_size = header_size
				.checked_add(size)
				.ok_or_else(|| "compact header size overflowed".to_owned())?;
			physical_fields.push(CompactFieldLayout {
				name: field.name.clone(),
				header_offset: u64::try_from(header_offset)
					.map_err(|_| "compact header offset exceeds u64".to_owned())?,
				header_size: u64::try_from(size)
					.map_err(|_| "compact header field size exceeds u64".to_owned())?,
				tail: None,
			});
			continue;
		};

		seen_tail = true;
		validate_compact_prefix(tail.capacity, tail.prefix_bytes)?;
		let header_field_size = if tail.optional { 1 } else { tail.prefix_bytes };
		header_size = header_size
			.checked_add(header_field_size)
			.ok_or_else(|| "compact header size overflowed".to_owned())?;
		let payload_size = tail
			.capacity
			.checked_mul(tail.element_size)
			.ok_or_else(|| "compact tail capacity overflowed".to_owned())?;
		let maximum_bytes = if tail.optional {
			tail.prefix_bytes
				.checked_add(payload_size)
				.ok_or_else(|| "optional compact tail size overflowed".to_owned())?
		} else {
			payload_size
		};
		maximum_tail_size = maximum_tail_size
			.checked_add(maximum_bytes)
			.ok_or_else(|| "compact maximum size overflowed".to_owned())?;
		let field_alignment = if tail.optional {
			greatest_common_divisor(tail.prefix_bytes, tail.element_size)
		} else {
			tail.element_size
		};
		tail_alignment = greatest_common_divisor(tail_alignment, field_alignment);
		physical_fields.push(CompactFieldLayout {
			name: field.name.clone(),
			header_offset: u64::try_from(header_offset)
				.map_err(|_| "compact header offset exceeds u64".to_owned())?,
			header_size: u64::try_from(header_field_size)
				.map_err(|_| "compact header field size exceeds u64".to_owned())?,
			tail: Some(CompactTailLayout {
				index: tail_index,
				kind: tail.kind,
				optional: tail.optional,
				prefix_bytes: u8::try_from(tail.prefix_bytes)
					.map_err(|_| "compact prefix width exceeds u8".to_owned())?,
				capacity: u64::try_from(tail.capacity)
					.map_err(|_| "compact capacity exceeds u64".to_owned())?,
				element_size: u64::try_from(tail.element_size)
					.map_err(|_| "compact element size exceeds u64".to_owned())?,
				maximum_bytes: u64::try_from(maximum_bytes)
					.map_err(|_| "compact tail size exceeds u64".to_owned())?,
			}),
		});
		tail_index = tail_index
			.checked_add(1)
			.ok_or_else(|| "compact tail count exceeds u32".to_owned())?;
	}

	if !seen_tail {
		return Err("compact schemas require at least one dynamic tail".to_owned());
	}
	let maximum_size = header_size
		.checked_add(maximum_tail_size)
		.ok_or_else(|| "compact maximum size overflowed".to_owned())?;
	Ok(PhysicalLayout::Compact {
		header_size: u64::try_from(header_size)
			.map_err(|_| "compact header size exceeds u64".to_owned())?,
		maximum_size: u64::try_from(maximum_size)
			.map_err(|_| "compact maximum size exceeds u64".to_owned())?,
		tail_alignment: u64::try_from(tail_alignment)
			.map_err(|_| "compact tail alignment exceeds u64".to_owned())?,
		fields: physical_fields,
	})
}

fn parse_compact_tail(ty: &str) -> Result<Option<ParsedCompactTail>, String> {
	let mut optional = false;
	let mut tail_type = ty.trim();
	if let Some(("Option", arguments)) = parse_generic(tail_type) {
		let [inner] = arguments.as_slice() else {
			return Err(format!("invalid compact option type `{ty}`"));
		};
		optional = true;
		tail_type = inner;
	}

	let Some((name, arguments)) = parse_generic(tail_type) else {
		return Ok(None);
	};
	match name {
		"String" | "PodString" => {
			let (capacity, prefix_bytes) = parse_compact_capacity(name, &arguments, 1)?;
			Ok(Some(ParsedCompactTail {
				kind: CompactTailKind::String,
				optional,
				prefix_bytes,
				capacity,
				element_size: 1,
			}))
		}
		"Vec" | "PodVec" => {
			let [element, capacity, rest @ ..] = arguments.as_slice() else {
				return Err(format!("invalid compact vector type `{ty}`"));
			};
			let prefix_bytes = match (name, rest) {
				("Vec" | "PodVec", []) => 2,
				("PodVec", [prefix]) => parse_compact_prefix(prefix)?,
				_ => return Err(format!("invalid compact vector type `{ty}`")),
			};
			let capacity = capacity
				.parse::<usize>()
				.map_err(|_| format!("invalid compact vector capacity in `{ty}`"))?;
			if optional && is_compact_string_name(element) {
				return Err(format!(
					"optional compact vectors cannot contain string elements in `{ty}`"
				));
			}
			if !is_compact_string_name(element) && contains_dynamic_compact_name(element) {
				return Err(format!(
					"compact vector element `{element}` contains a nested dynamic type"
				));
			}
			let element_size = fixed_type_size(element)
				.ok_or_else(|| format!("unsupported compact vector element `{element}`"))?;
			Ok(Some(ParsedCompactTail {
				kind: CompactTailKind::Vector,
				optional,
				prefix_bytes,
				capacity,
				element_size,
			}))
		}
		_ if optional && contains_dynamic_compact_name(tail_type) => {
			Err(format!("nested dynamic compact type `{ty}` is unsupported"))
		}
		_ if optional => Ok(None),
		_ => Ok(None),
	}
}

fn is_compact_string_name(ty: &str) -> bool {
	parse_generic(ty.trim()).is_some_and(|(name, _)| matches!(name, "String" | "PodString"))
}

fn contains_dynamic_compact_name(ty: &str) -> bool {
	contains_dynamic_compact_name_at_depth(ty, 0)
}

fn contains_dynamic_compact_name_at_depth(ty: &str, depth: usize) -> bool {
	if depth > MAX_TYPE_NESTING_DEPTH {
		return true;
	}
	let Some((name, arguments)) = parse_generic(ty.trim()) else {
		return false;
	};
	if matches!(name, "String" | "PodString" | "Vec" | "PodVec") {
		return true;
	}

	name == "Option"
		&& arguments
			.first()
			.is_some_and(|inner| contains_dynamic_compact_name_at_depth(inner, depth + 1))
}

fn parse_compact_capacity(
	name: &str,
	arguments: &[&str],
	default_prefix: usize,
) -> Result<(usize, usize), String> {
	let [capacity, rest @ ..] = arguments else {
		return Err(format!("invalid compact {name} type"));
	};
	let capacity = capacity
		.parse::<usize>()
		.map_err(|_| format!("invalid compact {name} capacity"))?;
	let prefix = match (name, rest) {
		("String" | "PodString", []) => default_prefix,
		("PodString", [prefix]) => parse_compact_prefix(prefix)?,
		_ => return Err(format!("invalid compact {name} type")),
	};
	Ok((capacity, prefix))
}

fn parse_compact_prefix(value: &str) -> Result<usize, String> {
	let prefix = value
		.parse::<usize>()
		.map_err(|_| format!("invalid compact prefix width `{value}`"))?;
	if !matches!(prefix, 1 | 2 | 4 | 8) {
		return Err(format!(
			"compact prefix width must be 1, 2, 4, or 8 bytes, got {prefix}"
		));
	}
	Ok(prefix)
}

fn validate_compact_prefix(capacity: usize, prefix_bytes: usize) -> Result<(), String> {
	let capacity =
		u64::try_from(capacity).map_err(|_| "compact capacity exceeds u64".to_owned())?;
	let maximum = match prefix_bytes {
		1 => u64::from(u8::MAX),
		2 => u64::from(u16::MAX),
		4 => u64::from(u32::MAX),
		8 => u64::MAX,
		_ => return Err(format!("invalid compact prefix width {prefix_bytes}")),
	};
	if capacity > maximum {
		return Err(format!(
			"compact capacity {capacity} exceeds a {prefix_bytes}-byte prefix"
		));
	}
	Ok(())
}

const fn greatest_common_divisor(mut left: usize, mut right: usize) -> usize {
	while right != 0 {
		let remainder = left % right;
		left = right;
		right = remainder;
	}
	left
}

/// Build the data-only schema seen by an attribute macro before envelope fields are injected.
///
/// Field types must already be normalized: a capacity is recorded as the number
/// it evaluates to, so callers resolve named constants through
/// [`SchemaConsts::normalize_item`] first.
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

	DataSchema::try_new(layout, fields)
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
	// The separator is the last `;` at any nesting depth, so `[[u8; 4]; 2]`
	// splits into the element `[u8; 4]` and the length `2`.
	let (element, length) = inner.rsplit_once(';')?;
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
	let encoded = serde_json::to_vec(value)
		.unwrap_or_else(|error| panic!("serializing Pina ABI model failed: {error}"));
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
		.as_chunks::<2>()
		.0
		.iter()
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

/// Which ABI document a schema or fixture describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbiDocument {
	/// `migrations/manifest.json`.
	Manifest,
	/// `migrations/publications.json`.
	Publications,
}

impl AbiDocument {
	/// Every document kind.
	pub const ALL: [Self; 2] = [Self::Manifest, Self::Publications];

	/// Stable file name for this document's schema artifact.
	#[must_use]
	pub const fn schema_file_name(self) -> &'static str {
		match self {
			Self::Manifest => "manifest.schema.json",
			Self::Publications => "publications.schema.json",
		}
	}

	/// Stable `--document` spelling.
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Manifest => "manifest",
			Self::Publications => "publications",
		}
	}

	/// Parse the `--document` spelling.
	#[must_use]
	pub fn parse(value: &str) -> Option<Self> {
		match value {
			"manifest" => Some(Self::Manifest),
			"publications" => Some(Self::Publications),
			_ => None,
		}
	}
}

/// Generate the JSON Schema for one ABI document.
pub fn document_schema(kind: AbiDocument) -> serde_json::Value {
	match kind {
		AbiDocument::Manifest => serde_json::to_value(schemars::schema_for!(MigrationManifest)),
		AbiDocument::Publications => serde_json::to_value(schemars::schema_for!(PublicationLedger)),
	}
	.unwrap_or_else(|error| panic!("serializing Pina ABI schema failed: {error}"))
}

/// Permanent versioned URL for one document's schema.
#[must_use]
pub fn schema_url(kind: AbiDocument, version: &str) -> String {
	format!(
		"https://pina-rs.github.io/pina/abi/schemas/{version}/{}",
		kind.schema_file_name()
	)
}

/// Render one document's schema as canonical pretty-printed JSON with `$id`.
pub fn render_document_schema(kind: AbiDocument) -> Result<String, String> {
	let mut schema = document_schema(kind);
	if let Some(object) = schema.as_object_mut() {
		object.insert(
			"$id".to_owned(),
			serde_json::Value::from(schema_url(kind, ABI_VERSION)),
		);
	}
	let mut encoded = serde_json::to_string_pretty(&schema)
		.map_err(|error| format!("could not encode ABI schema: {error}"))?;
	encoded.push('\n');
	Ok(encoded)
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
		}
	}

	fn process(accounts: Vec<ProcessAccount>) -> ProcessContract {
		ProcessContract { accounts }
	}

	fn version(schema: DataSchema, process: Option<ProcessContract>) -> SchemaVersion {
		SchemaVersion {
			schema,
			process,
			transition: None,
		}
	}

	fn fixed_schema(fields: &[(&str, &str)]) -> DataSchema {
		DataSchema::try_new(
			LayoutKind::Fixed,
			fields
				.iter()
				.map(|(name, ty)| {
					FieldSchema {
						name: (*name).to_owned(),
						rust_type: (*ty).to_owned(),
					}
				})
				.collect(),
		)
		.unwrap_or_else(|error| panic!("schema: {error}"))
	}

	fn account_manifest(schema: DataSchema) -> MigrationManifest {
		let identity = ContractIdentity::try_new(ContractKind::Account, 1, 0xAB)
			.unwrap_or_else(|error| panic!("identity: {error}"));
		let key = identity.key();
		let mut manifest = MigrationManifest::new("program".to_owned(), MigrationVersionType::U8);
		manifest.contracts.insert(
			key,
			ContractHistory {
				identity,
				rust_name: "Profile".to_owned(),
				versions: vec![version(schema, None)],
			},
		);
		manifest
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
	fn physical_layout_freezes_compact_headers_tails_and_alignment() {
		let item: syn::ItemStruct = syn::parse_quote! {
			struct Profile {
				authority: Address,
				name: String<8>,
				tags: Option<PodVec<u16, 3, 1>>,
			}
		};
		let schema = data_schema(&item, LayoutKind::Compact)
			.unwrap_or_else(|error| panic!("compact schema: {error}"));

		assert_eq!(
			schema.physical(),
			Ok(PhysicalLayout::Compact {
				header_size: 34,
				maximum_size: 49,
				tail_alignment: 1,
				fields: vec![
					CompactFieldLayout {
						name: "authority".to_owned(),
						header_offset: 0,
						header_size: 32,
						tail: None,
					},
					CompactFieldLayout {
						name: "name".to_owned(),
						header_offset: 32,
						header_size: 1,
						tail: Some(CompactTailLayout {
							index: 0,
							kind: CompactTailKind::String,
							optional: false,
							prefix_bytes: 1,
							capacity: 8,
							element_size: 1,
							maximum_bytes: 8,
						}),
					},
					CompactFieldLayout {
						name: "tags".to_owned(),
						header_offset: 33,
						header_size: 1,
						tail: Some(CompactTailLayout {
							index: 1,
							kind: CompactTailKind::Vector,
							optional: true,
							prefix_bytes: 1,
							capacity: 3,
							element_size: 2,
							maximum_bytes: 7,
						}),
					},
				],
			})
		);
	}

	#[test]
	fn physical_layout_rejects_shapes_the_compact_macro_cannot_generate() {
		let nested_vector: syn::ItemStruct = syn::parse_quote! {
			struct Invalid { values: Vec<Vec<u16, 2>, 3> }
		};
		let optional_strings: syn::ItemStruct = syn::parse_quote! {
			struct Invalid { values: Option<Vec<String<4>, 3>> }
		};
		let nested_option: syn::ItemStruct = syn::parse_quote! {
			struct Invalid { values: Option<Option<String<4>>> }
		};
		let restricted_element_array: syn::ItemStruct = syn::parse_quote! {
			struct Invalid { values: [char; 2], name: String<4> }
		};

		for item in [
			nested_vector,
			optional_strings,
			nested_option,
			restricted_element_array,
		] {
			assert!(data_schema(&item, LayoutKind::Compact).is_err());
		}

		let invalid_prefix: syn::ItemStruct = syn::parse_quote! {
			struct Invalid { value: PodString<4, 3> }
		};
		let overflowing_capacity: syn::ItemStruct = syn::parse_quote! {
			struct Invalid { value: String<256> }
		};
		let unsupported_key_alias: syn::ItemStruct = syn::parse_quote! {
			struct Invalid { value: Pubkey }
		};
		for item in [invalid_prefix, overflowing_capacity, unsupported_key_alias] {
			assert!(data_schema(&item, LayoutKind::Fixed).is_err());
		}
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
	fn auto_policy_serializes_the_config_spelling_and_rejects_unknown_kinds() {
		let mut auto = MigrationAuto::none();
		auto.add(ContractKind::Event);
		auto.add(ContractKind::Account);

		let encoded = serde_json::to_string(&auto).unwrap();
		// BTreeSet iteration keeps the recorded policy in stable kind order.
		assert_eq!(encoded, r#"["accounts","events"]"#);
		assert_eq!(
			serde_json::from_str::<MigrationAuto>(&encoded).unwrap(),
			auto
		);
		assert_eq!(auto.to_string(), "accounts, events");
		assert!(MigrationAuto::all().contains(ContractKind::Instruction));
		assert!(MigrationAuto::none().is_empty());
		// `removed_since` reports the kinds the later policy dropped.
		assert_eq!(
			auto.removed_since(&MigrationAuto::all()),
			vec![ContractKind::Instruction]
		);
		assert!(MigrationAuto::all().removed_since(&auto).is_empty());

		let unknown = serde_json::from_str::<MigrationAuto>(r#"["states"]"#).unwrap_err();
		assert!(
			unknown
				.to_string()
				.contains("unknown migration kind `states`")
		);
		let duplicate =
			serde_json::from_str::<MigrationAuto>(r#"["accounts","accounts"]"#).unwrap_err();
		assert!(duplicate.to_string().contains("duplicate migration kind"));

		for kind in ContractKind::ALL {
			assert_eq!(
				ContractKind::from_config_name(kind.config_name()),
				Some(kind)
			);
			// The manifest keeps the singular identity spelling.
			assert_ne!(kind.config_name(), kind.as_str());
		}
		assert_eq!(ContractKind::from_config_name("account"), None);
	}

	#[test]
	fn manifest_round_trips_and_stores_no_derivations() {
		let manifest = account_manifest(fixed_schema(&[("value", "u64")]));

		let encoded = encode_manifest(&manifest).unwrap_or_else(|error| panic!("encode: {error}"));
		let text = String::from_utf8(encoded.clone()).unwrap();

		// The stored document keeps facts only.
		assert!(text.contains("\"abiVersion\""));
		assert!(!text.contains("physical"));
		assert!(!text.contains("schemaSha256"));
		assert!(!text.contains("processSha256"));
		assert_eq!(
			decode_manifest(&encoded).unwrap_or_else(|error| panic!("decode: {error}")),
			manifest
		);
	}

	#[test]
	fn manifest_version_zero_cannot_carry_a_transition() {
		let mut manifest = account_manifest(fixed_schema(&[("value", "u64")]));
		let key = manifest.contracts.keys().next().cloned().unwrap();
		let history = manifest.contracts.get_mut(&key).unwrap();
		history.versions[0].transition = Some(Transition {
			mode: TransitionMode::Automatic,
			renames: Vec::new(),
			implementation_sha256: None,
		});

		assert!(
			manifest
				.validate()
				.unwrap_err()
				.contains("version zero cannot have a transition")
		);
	}

	#[test]
	fn manifest_requires_a_transition_for_every_later_version() {
		let schema = fixed_schema(&[("value", "u64")]);
		let identity = ContractIdentity::try_new(ContractKind::Account, 1, 0xAB).unwrap();
		let key = identity.key();
		let manifest = MigrationManifest {
			abi_version: ABI_VERSION.to_owned(),
			program_id: "program".to_owned(),
			version_type: MigrationVersionType::U8,
			auto: MigrationAuto::none(),
			contracts: BTreeMap::from([(
				key,
				ContractHistory {
					identity,
					rust_name: "Profile".to_owned(),
					versions: vec![version(schema.clone(), None), version(schema, None)],
				},
			)]),
		};

		assert!(
			manifest
				.validate()
				.unwrap_err()
				.contains("has no adjacent transition")
		);
	}

	#[test]
	fn version_numbers_are_positions_and_hashes_are_derived() {
		let schema = fixed_schema(&[("value", "u64")]);
		let history = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Account, 1, 0xAB).unwrap(),
			rust_name: "Profile".to_owned(),
			versions: vec![version(schema.clone(), None)],
		};

		assert_eq!(history.current_version(), Some(0));
		assert_eq!(
			history.version(0).map(SchemaVersion::schema_sha256),
			Some(schema.sha256())
		);
		assert_eq!(history.version(1), None);
		assert_eq!(history.current(), history.version(0));
	}

	#[test]
	fn instruction_and_event_histories_reject_compact_layouts() {
		let item: syn::ItemStruct = syn::parse_quote! {
			struct Payload { name: String<8> }
		};
		let schema = data_schema(&item, LayoutKind::Compact)
			.unwrap_or_else(|error| panic!("compact schema: {error}"));

		for kind in [ContractKind::Instruction, ContractKind::Event] {
			let identity = ContractIdentity::try_new(kind, 1, 1)
				.unwrap_or_else(|error| panic!("identity: {error}"));
			let mut entry = version(schema.clone(), None);
			if kind == ContractKind::Instruction {
				entry.process = Some(process(vec![process_account("authority", false)]));
			}
			let history = ContractHistory {
				identity,
				rust_name: "Payload".to_owned(),
				versions: vec![entry],
			};

			assert!(
				history.validate(MigrationVersionType::U8).is_err(),
				"{kind} contracts must use fixed layouts"
			);
		}
	}

	#[test]
	fn instruction_versions_require_a_process_contract() {
		let history = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Instruction, 1, 1).unwrap(),
			rust_name: "Transfer".to_owned(),
			versions: vec![version(fixed_schema(&[("amount", "u64")]), None)],
		};

		assert!(
			history
				.validate(MigrationVersionType::U8)
				.unwrap_err()
				.contains("is missing its process contract")
		);
	}

	#[test]
	fn manifest_rejects_path_traversal_in_contract_identities() {
		// Before identity validation, this document decoded and validated,
		// and `transition_path` interpolated the unvalidated hex into
		// `migrations/transitions/account_1_ab/../../../evil/v0_to_v1.rs`,
		// giving a tampered manifest a file-write primitive outside the
		// migrations directory.
		let mut value =
			serde_json::to_value(account_manifest(fixed_schema(&[("value", "u64")]))).unwrap();
		let contracts = value
			.get_mut("contracts")
			.and_then(|contracts| contracts.as_object_mut())
			.unwrap();
		let (_, contract) = contracts.iter_mut().next().unwrap();
		contract["identity"]["discriminatorHex"] =
			serde_json::Value::String("ab/../../../evil".into());
		let (_, history) = contracts.remove_entry("account:1:ab").unwrap();
		contracts.insert("account:1:ab/../../../evil".to_owned(), history);

		assert!(decode_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
	}

	#[test]
	fn manifest_rejects_noncanonical_identity_hex() {
		let mut value =
			serde_json::to_value(account_manifest(fixed_schema(&[("value", "u64")]))).unwrap();
		let contracts = value
			.get_mut("contracts")
			.and_then(|contracts| contracts.as_object_mut())
			.unwrap();
		let (_, contract) = contracts.iter_mut().next().unwrap();
		contract["identity"]["discriminatorHex"] = serde_json::Value::String("AB".to_owned());
		let (_, history) = contracts.remove_entry("account:1:ab").unwrap();
		contracts.insert("account:1:AB".to_owned(), history);

		assert!(decode_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
	}

	#[test]
	fn manifest_rejects_identity_width_mismatches() {
		for (hex, bytes) in [("a", 1_u8), ("ab", 2), ("", 0)] {
			let mut value =
				serde_json::to_value(account_manifest(fixed_schema(&[("value", "u64")]))).unwrap();
			let contracts = value
				.get_mut("contracts")
				.and_then(|contracts| contracts.as_object_mut())
				.unwrap();
			let key = format!("account:{bytes}:{hex}");
			let (_, contract) = contracts.iter_mut().next().unwrap();
			contract["identity"]["discriminatorHex"] = serde_json::Value::String(hex.to_owned());
			contract["identity"]["discriminatorBytes"] = serde_json::Value::from(bytes);
			let (_, history) = contracts.remove_entry("account:1:ab").unwrap();
			contracts.insert(key, history);

			let encoded = serde_json::to_vec(&value).unwrap();
			assert!(
				decode_manifest(&encoded).is_err(),
				"identity width {bytes} with hex `{hex}` must be rejected"
			);
		}
	}

	#[test]
	fn transition_validates_renames_against_neighbouring_fields() {
		let schema = fixed_schema(&[("value", "u64")]);
		let identity = ContractIdentity::try_new(ContractKind::Account, 1, 0xAB).unwrap();
		let key = identity.key();
		let mut second = version(schema.clone(), None);
		second.transition = Some(Transition {
			mode: TransitionMode::Automatic,
			renames: vec![RenameMapping {
				from: "missing".to_owned(),
				to: "value".to_owned(),
			}],
			implementation_sha256: None,
		});
		let manifest = MigrationManifest {
			abi_version: ABI_VERSION.to_owned(),
			program_id: "program".to_owned(),
			version_type: MigrationVersionType::U8,
			auto: MigrationAuto::none(),
			contracts: BTreeMap::from([(
				key,
				ContractHistory {
					identity,
					rust_name: "Profile".to_owned(),
					versions: vec![version(schema, None), second],
				},
			)]),
		};

		assert!(
			manifest
				.validate()
				.unwrap_err()
				.contains("renames unknown source field")
		);
	}

	#[test]
	fn instruction_process_transition_rejects_a_breaking_change() {
		let schema = fixed_schema(&[("amount", "u64")]);
		let identity = ContractIdentity::try_new(ContractKind::Instruction, 1, 4).unwrap();
		let key = identity.key();
		let original = process(vec![process_account("authority", false)]);
		let changed = process(vec![
			process_account("treasury", false),
			process_account("authority", false),
		]);
		let mut second = version(schema.clone(), Some(changed.clone()));
		second.transition = Some(Transition {
			mode: TransitionMode::Automatic,
			renames: Vec::new(),
			implementation_sha256: None,
		});
		let manifest = MigrationManifest {
			abi_version: ABI_VERSION.to_owned(),
			program_id: "program".to_owned(),
			version_type: MigrationVersionType::U8,
			auto: MigrationAuto::none(),
			contracts: BTreeMap::from([(
				key,
				ContractHistory {
					identity,
					rust_name: "Transfer".to_owned(),
					versions: vec![version(schema, Some(original)), second],
				},
			)]),
		};

		assert!(
			manifest
				.validate()
				.unwrap_err()
				.contains("process is breaking")
		);
	}

	#[test]
	fn process_proof_is_derived_for_an_appended_optional_slot() {
		let schema = fixed_schema(&[("amount", "u64")]);
		let original = process(vec![process_account("authority", false)]);
		let appended = process(vec![
			process_account("authority", false),
			process_account("referrer", true),
		]);
		let mut second = version(schema.clone(), Some(appended));
		second.transition = Some(Transition {
			mode: TransitionMode::Automatic,
			renames: Vec::new(),
			implementation_sha256: None,
		});
		let history = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Instruction, 1, 4).unwrap(),
			rust_name: "Transfer".to_owned(),
			versions: vec![version(schema, Some(original)), second],
		};

		assert_eq!(history.validate(MigrationVersionType::U8), Ok(()));
		let proof = history
			.process_transition_into(1)
			.unwrap_or_else(|error| panic!("proof: {error}"))
			.unwrap_or_else(|| panic!("proof must exist"));
		assert_eq!(proof.kind, ProcessTransitionKind::AppendOptional);
		assert_eq!(proof.source_accounts, 1);
		assert_eq!(proof.destination_accounts, 2);
	}

	#[test]
	fn publication_ledger_is_hash_chained() {
		let first = PublicationReceipt {
			sequence: 0,
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "a".repeat(64),
			manifest_sha256: "b".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), PublishedContract::legacy(0))]),
			previous_receipt_sha256: None,
			abandoned: false,
		};
		let second = PublicationReceipt {
			sequence: 1,
			rpc_url: "https://api.mainnet-beta.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "c".repeat(64),
			manifest_sha256: "d".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), PublishedContract::legacy(1))]),
			previous_receipt_sha256: Some(first.sha256()),
			abandoned: false,
		};
		let ledger = PublicationLedger {
			abi_version: ABI_VERSION.to_owned(),
			receipts: vec![first, second],
			pending: None,
		};

		assert_eq!(ledger.validate(), Ok(()));
		assert!(ledger.ever_published("account:1:00", 0));
		assert!(ledger.ever_published("account:1:00", 1));
		assert!(!ledger.ever_published("account:1:00", 2));

		let mut pending = ledger.clone();
		pending.pending = Some(PendingPublication {
			cluster: "devnet".to_owned(),
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "e".repeat(64),
			manifest_sha256: "f".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), PublishedContract::legacy(2))]),
			previous_receipt_sha256: pending.receipts.last().map(PublicationReceipt::sha256),
		});
		assert_eq!(pending.validate(), Ok(()));
		assert!(pending.version_is_frozen("account:1:00", 2));
		assert!(!pending.ever_published("account:1:00", 2));
	}

	#[test]
	fn publication_ledger_rejects_version_regression() {
		let first = PublicationReceipt {
			sequence: 0,
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "a".repeat(64),
			manifest_sha256: "b".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), PublishedContract::legacy(1))]),
			previous_receipt_sha256: None,
			abandoned: false,
		};
		let second = PublicationReceipt {
			sequence: 1,
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "c".repeat(64),
			manifest_sha256: "d".repeat(64),
			versions: BTreeMap::from([("account:1:00".to_owned(), PublishedContract::legacy(0))]),
			previous_receipt_sha256: Some(first.sha256()),
			abandoned: false,
		};
		let ledger = PublicationLedger {
			abi_version: ABI_VERSION.to_owned(),
			receipts: vec![first, second],
			pending: None,
		};

		assert!(ledger.validate().unwrap_err().contains("regresses"));
	}

	#[test]
	fn document_decoder_rejects_missing_and_future_versions() {
		assert!(
			decode_manifest(br#"{"contracts":{}}"#)
				.unwrap_err()
				.contains("abiVersion")
		);
		let future = r#"{"abiVersion":"9.9","programId":"p","versionType":"u8","contracts":{}}"#;
		assert!(
			decode_manifest(future.as_bytes())
				.unwrap_err()
				.contains("supports")
		);
	}

	#[test]
	fn current_manifest_rejects_unknown_fields() {
		let mut value =
			serde_json::to_value(account_manifest(fixed_schema(&[("value", "u64")]))).unwrap();
		value
			.as_object_mut()
			.unwrap()
			.insert("injected".to_owned(), serde_json::Value::Bool(true));

		assert!(decode_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
	}

	#[test]
	fn document_version_accepts_minor_and_rejects_patch() {
		assert!(parse_document_version("0.20").is_ok());
		assert!(parse_document_version("0.20.0").is_ok());
		assert!(parse_document_version("0.20.1").is_err());
		assert!(parse_document_version("not-a-version").is_err());
		assert_eq!(current_abi_version().major, 0);
	}

	#[test]
	fn walk_rejects_a_document_from_the_future() {
		let value =
			serde_json::to_value(account_manifest(fixed_schema(&[("value", "u64")]))).unwrap();

		let error = walk_document("migration manifest", "99.0", value).unwrap_err();
		assert!(error.contains("upgrade Pina"));
	}

	#[test]
	fn walk_is_identity_at_the_current_version() {
		let manifest = account_manifest(fixed_schema(&[("value", "u64")]));
		let value = serde_json::to_value(&manifest).unwrap();

		let walked = walk_document("migration manifest", ABI_VERSION, value.clone())
			.unwrap_or_else(|error| panic!("walk: {error}"));
		assert_eq!(walked, value);
	}

	#[test]
	fn walk_cannot_reach_a_version_below_the_baseline() {
		let value = serde_json::Value::Object(serde_json::Map::new());
		let error = walk_document("migration manifest", "0.1", value).unwrap_err();
		assert!(error.contains("predates the oldest supported version"));
	}

	#[test]
	fn deep_type_strings_are_bounded() {
		let mut deep = String::from("u64");
		for _ in 0..128 {
			deep = format!("Option<{deep}>");
		}

		assert!(
			fixed_type_size(&deep).is_none(),
			"deeply nested type strings must be rejected"
		);
		assert_eq!(fixed_type_size("u64"), Some(8));
		assert_eq!(fixed_type_size("Option<u64>"), Some(9));
	}

	#[test]
	fn float_types_size_by_backing_width() {
		assert_eq!(fixed_type_size("f32"), Some(4));
		assert_eq!(fixed_type_size("f64"), Some(8));
		assert_eq!(fixed_type_size("Vec<f32, 4>"), Some(18));
		assert_eq!(fixed_type_size("Option<f64>"), Some(9));
	}

	#[test]
	fn typed_arrays_size_by_element_composition() {
		assert_eq!(fixed_type_size("[u8; 32]"), Some(32));
		assert_eq!(fixed_type_size("[u64; 8]"), Some(64));
		assert_eq!(fixed_type_size("[PodU64; 4]"), Some(32));
		assert_eq!(fixed_type_size("[[u8; 4]; 2]"), Some(8));
		assert_eq!(fixed_type_size("Option<[u64; 2]>"), Some(17));
		assert_eq!(fixed_type_size("[Address; 2]"), Some(64));
		assert_eq!(fixed_type_size("[char; 4]"), None);
		assert_eq!(fixed_type_size("[u64; N]"), None);
	}

	#[test]
	fn fixed_point_types_size_by_backing_width() {
		assert_eq!(fixed_type_size("FixedI8<U1>"), Some(1));
		assert_eq!(fixed_type_size("FixedU8<U7>"), Some(1));
		assert_eq!(fixed_type_size("FixedI16<U9>"), Some(2));
		assert_eq!(fixed_type_size("FixedU16<U2>"), Some(2));
		assert_eq!(fixed_type_size("FixedI32<U24>"), Some(4));
		assert_eq!(fixed_type_size("FixedU32<U1>"), Some(4));
		assert_eq!(fixed_type_size("FixedI64<U48>"), Some(8));
		assert_eq!(fixed_type_size("FixedU64<U16>"), Some(8));
		assert_eq!(fixed_type_size("FixedI128<U96>"), Some(16));
		assert_eq!(fixed_type_size("FixedU128<U127>"), Some(16));

		// Fixed-point values compose with the bounded collections.
		assert_eq!(fixed_type_size("Option<FixedU64<U16>>"), Some(9));
		assert_eq!(fixed_type_size("Vec<FixedU64<U16>, 4>"), Some(34));
	}

	#[test]
	fn fixed_point_types_reject_invalid_arity() {
		assert_eq!(fixed_type_size("FixedU64"), None);
		assert_eq!(fixed_type_size("FixedU64<U16, U32>"), None);
		assert_eq!(fixed_type_size("FixedU256<U16>"), None);
		assert_eq!(fixed_type_size("Vec<FixedU64<U16>, 4, 2>"), None);
	}

	#[test]
	fn document_schemas_cover_both_documents_and_name_themselves() {
		for kind in AbiDocument::ALL {
			let schema = document_schema(kind);
			assert!(schema.is_object(), "{kind:?} must produce an object schema");
			let rendered =
				render_document_schema(kind).unwrap_or_else(|error| panic!("render: {error}"));
			assert!(rendered.contains(&schema_url(kind, ABI_VERSION)));
			assert!(rendered.ends_with('\n'));
			assert_eq!(AbiDocument::parse(kind.as_str()), Some(kind));
			assert!(rendered.contains(kind.schema_file_name()));
		}
		assert_eq!(AbiDocument::parse("other"), None);
	}

	/// The committed value must not lag the crate's `major.minor`, and a value
	/// ahead of it is only legitimate while a changeset will catch the crate up.
	///
	/// This is the pre-release window: a shape-changing pull request advances
	/// `ABI_VERSION` while `Cargo.toml` still names the released version, and the
	/// `pina_abi` changeset carries the crate to meet it at release time. Any
	/// other ahead state means the value was advanced with nothing to release it.
	#[test]
	fn abi_version_does_not_lag_the_crate_and_ahead_requires_a_changeset() {
		let crate_version = Version::parse(env!("CARGO_PKG_VERSION"))
			.unwrap_or_else(|error| panic!("crate version: {error}"));
		let committed = current_abi_version();
		let crate_minor = (crate_version.major, crate_version.minor);
		let committed_minor = (committed.major, committed.minor);

		assert!(
			committed_minor >= crate_minor,
			"ABI_VERSION {committed} lags the crate's major.minor {crate_version}"
		);

		if committed_minor > crate_minor {
			let changeset_dir =
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.changeset");
			let declares_pina_abi = std::fs::read_dir(&changeset_dir)
				.map(|entries| {
					entries.filter_map(Result::ok).any(|entry| {
						let path = entry.path();
						if path.extension().is_none_or(|extension| extension != "md") {
							return false;
						}
						std::fs::read_to_string(&path).is_ok_and(|body| body.contains("pina_abi:"))
					})
				})
				.unwrap_or(false);
			assert!(
				declares_pina_abi,
				"ABI_VERSION {committed} leads the crate at {crate_version}; an active \
				 `.changeset/*.md` entry for `pina_abi` must carry the crate to meet it"
			);
		}
	}

	/// The converter table must be gapless: every step advances the version,
	/// each continues from the previous one, and the chain reaches the current
	/// version whenever a step exists at all.
	#[test]
	fn converter_chain_is_gapless_and_reaches_current() {
		let current = current_abi_version();
		let oldest =
			parse_document_version(ABI_OLDEST_SUPPORTED).unwrap_or_else(|error| panic!("{error}"));
		assert!(
			oldest <= current,
			"the baseline cannot postdate the current version"
		);

		let mut position = None::<Version>;
		for step in ABI_STEPS {
			let from =
				parse_document_version(step.from).unwrap_or_else(|error| panic!("step: {error}"));
			let to =
				parse_document_version(step.to).unwrap_or_else(|error| panic!("step: {error}"));
			assert!(from < to, "step {from} -> {to} must advance the version");
			match &position {
				None => {
					assert_eq!(
						from, oldest,
						"the first step must start at the oldest supported version"
					)
				}
				Some(expected) => {
					assert_eq!(
						&from, expected,
						"step {from} -> {to} must continue from the previous step"
					)
				}
			}
			position = Some(to);
		}

		match position {
			// No shape change has shipped since the reset baseline, so there is
			// nothing to walk and the reader accepts exactly the current version.
			None => assert_eq!(current, oldest),
			Some(last) => assert_eq!(last, current, "the chain must reach the current version"),
		}
	}

	/// Encoding a validated document must round-trip through the reader, so the
	/// bytes `pina migrations make` writes are exactly what a later build reads.
	#[test]
	fn both_documents_encode_and_decode_through_the_same_model() {
		let manifest = account_manifest(fixed_schema(&[("value", "u64")]));
		let manifest_bytes =
			encode_manifest(&manifest).unwrap_or_else(|error| panic!("encode manifest: {error}"));
		assert_eq!(
			decode_manifest(&manifest_bytes).unwrap_or_else(|error| panic!("decode: {error}")),
			manifest
		);

		let receipt = PublicationReceipt {
			sequence: 0,
			rpc_url: "https://api.devnet.solana.com".to_owned(),
			program_id: "program".to_owned(),
			executable_sha256: "a".repeat(64),
			manifest_sha256: "b".repeat(64),
			versions: BTreeMap::from([(
				"account:1:00".to_owned(),
				PublishedContract {
					version: 0,
					history: vec![PublishedSchema {
						schema_sha256: "c".repeat(64),
						transition_sha256: None,
					}],
				},
			)]),
			previous_receipt_sha256: None,
			abandoned: false,
		};
		let ledger = PublicationLedger {
			abi_version: ABI_VERSION.to_owned(),
			receipts: vec![receipt],
			pending: None,
		};
		let ledger_bytes = encode_publication_ledger(&ledger)
			.unwrap_or_else(|error| panic!("encode ledger: {error}"));
		assert_eq!(
			decode_publication_ledger(&ledger_bytes)
				.unwrap_or_else(|error| panic!("decode: {error}")),
			ledger
		);

		// An unpinned history stays valid and reports the version it made live.
		let unpinned = PublishedContract::legacy(3);
		assert_eq!(unpinned.version(), 3);
		assert!(unpinned.history().is_empty());
	}

	/// An invalid document is refused at encode time, not written and then
	/// rejected by the next reader.
	#[test]
	fn encoding_rejects_an_invalid_document() {
		let mut manifest = account_manifest(fixed_schema(&[("value", "u64")]));
		manifest.abi_version = "9.9".to_owned();
		assert!(encode_manifest(&manifest).unwrap_err().contains("supports"));

		let mut ledger = PublicationLedger::default();
		ledger.abi_version = "9.9".to_owned();
		assert!(
			encode_publication_ledger(&ledger)
				.unwrap_err()
				.contains("supports")
		);
	}

	/// The baseline is enforced by the walk rather than by the version check:
	/// `validate_document_version` rejects only the future, so a reader
	/// normalizes what it can and names the remedy for what it cannot.
	#[test]
	fn a_document_below_the_baseline_names_the_remedy() {
		assert!(
			validate_document_version("migration manifest", ABI_OLDEST_SUPPORTED).is_ok(),
			"the oldest supported version must pass the version check"
		);
		assert!(
			validate_document_version("migration manifest", ABI_VERSION).is_ok(),
			"the current version must pass the version check"
		);

		let value = serde_json::Value::Object(serde_json::Map::new());
		let walked = walk_document("migration manifest", "0.1", value).unwrap_err();
		assert!(walked.contains("regenerate it with `pina migrations make`"));
	}

	/// An identity step returns the document unchanged, so a version that
	/// advanced without a shape change still traverses the walk.
	#[test]
	fn an_identity_step_preserves_the_document() {
		let value = serde_json::json!({"abiVersion": "0.20", "programId": "p"});
		assert_eq!(
			identity_step(value.clone()).unwrap_or_else(|error| panic!("{error}")),
			value
		);
		assert!(ABI_OLDEST_SUPPORTED <= ABI_VERSION);
	}

	#[test]
	fn derived_hashes_and_process_lookups_cover_an_instruction_history() {
		let schema = fixed_schema(&[("amount", "u64")]);
		let original = process(vec![process_account("authority", false)]);
		let appended = process(vec![
			process_account("authority", false),
			process_account("referrer", true),
		]);
		let mut second = version(schema.clone(), Some(appended.clone()));
		second.transition = Some(Transition {
			mode: TransitionMode::Automatic,
			renames: Vec::new(),
			implementation_sha256: None,
		});
		let history = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Instruction, 1, 4).unwrap(),
			rust_name: "Transfer".to_owned(),
			versions: vec![version(schema, Some(original.clone())), second],
		};

		// The derived accessors must agree with the values they replace.
		assert_eq!(
			history
				.version(0)
				.map(SchemaVersion::process_sha256)
				.flatten(),
			Some(original.sha256())
		);
		assert_eq!(
			history
				.version(1)
				.and_then(SchemaVersion::process_sha256)
				.as_deref(),
			Some(appended.sha256().as_str())
		);

		// Version zero has no entering transition, so no proof exists.
		assert_eq!(
			history
				.process_transition_into(0)
				.unwrap_or_else(|error| panic!("{error}")),
			None
		);
		// A version with no recorded transition also has no proof.
		assert_eq!(
			history
				.process_transition_into(9)
				.unwrap_or_else(|error| panic!("{error}")),
			None
		);
		// An account history carries no process, so the proof is absent.
		let account = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap(),
			rust_name: "State".to_owned(),
			versions: vec![version(fixed_schema(&[("value", "u64")]), None)],
		};
		assert_eq!(
			account
				.process_transition_into(0)
				.unwrap_or_else(|error| panic!("{error}")),
			None
		);
	}

	#[test]
	fn validation_reports_every_structural_violation() {
		let schema = fixed_schema(&[("value", "u64")]);

		// A history with no versions cannot describe a contract.
		let empty = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap(),
			rust_name: "State".to_owned(),
			versions: Vec::new(),
		};
		assert!(
			empty
				.validate(MigrationVersionType::U8)
				.unwrap_err()
				.contains("has no versions")
		);

		// An identity whose hex is not canonical is rejected on its own terms.
		// Mutating it inside a manifest cannot reach this path, because the
		// manifest key is derived from the identity and would mismatch first.
		let mut identity = ContractIdentity::try_new(ContractKind::Account, 1, 0xAB).unwrap();
		identity.discriminator_hex = "ZZ".to_owned();
		assert!(
			identity
				.validate()
				.unwrap_err()
				.contains("invalid hex value")
		);
		identity.discriminator_hex = "AB".to_owned();
		assert!(
			identity
				.validate()
				.unwrap_err()
				.contains("canonical lowercase hexadecimal")
		);

		// A version beyond the configured width cannot be encoded.
		let narrow = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Account, 1, 1).unwrap(),
			rust_name: "State".to_owned(),
			versions: vec![version(schema.clone(), None)],
		};
		let mut many = narrow.clone();
		for _ in 0..=u32::from(u8::MAX) {
			let mut next = version(schema.clone(), None);
			next.transition = Some(Transition {
				mode: TransitionMode::Automatic,
				renames: Vec::new(),
				implementation_sha256: None,
			});
			many.versions.push(next);
		}
		assert!(
			many.validate(MigrationVersionType::U8)
				.unwrap_err()
				.contains("exceeds configured")
		);
	}

	#[test]
	fn manifest_contract_keys_must_match_their_identity() {
		let mut manifest = account_manifest(fixed_schema(&[("value", "u64")]));
		let (key, history) = manifest
			.contracts
			.iter()
			.next()
			.map(|(key, history)| (key.clone(), history.clone()))
			.unwrap();
		manifest.contracts.remove(&key);
		manifest
			.contracts
			.insert("account:1:ff".to_owned(), history);

		assert!(
			manifest
				.validate()
				.unwrap_err()
				.contains("does not match identity")
		);
	}

	#[test]
	fn events_reject_a_process_and_accounts_reject_a_compact_instruction() {
		// An event that carries an instruction process is malformed.
		let schema = fixed_schema(&[("value", "u64")]);
		let mut entry = version(
			schema.clone(),
			Some(process(vec![process_account("a", false)])),
		);
		entry.transition = Some(Transition {
			mode: TransitionMode::Automatic,
			renames: Vec::new(),
			implementation_sha256: None,
		});
		let event = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Event, 1, 1).unwrap(),
			rust_name: "Changed".to_owned(),
			versions: vec![entry],
		};
		assert!(
			event
				.validate(MigrationVersionType::U8)
				.unwrap_err()
				.contains("cannot contain an instruction process")
		);

		// An instruction with a compact schema is rejected too.
		let compact = DataSchema::try_new(
			LayoutKind::Compact,
			vec![FieldSchema {
				name: "name".to_owned(),
				rust_type: "String<8>".to_owned(),
			}],
		)
		.unwrap();
		let instruction = ContractHistory {
			identity: ContractIdentity::try_new(ContractKind::Instruction, 1, 1).unwrap(),
			rust_name: "Send".to_owned(),
			versions: vec![version(
				compact,
				Some(process(vec![process_account("a", false)])),
			)],
		};
		assert!(
			instruction
				.validate(MigrationVersionType::U8)
				.unwrap_err()
				.contains("must use a fixed layout")
		);
	}

	#[test]
	fn a_rename_into_an_unknown_destination_field_is_rejected() {
		let schema = fixed_schema(&[("value", "u64")]);
		let mut second = version(schema.clone(), None);
		second.transition = Some(Transition {
			mode: TransitionMode::Automatic,
			renames: vec![RenameMapping {
				from: "value".to_owned(),
				to: "absent".to_owned(),
			}],
			implementation_sha256: None,
		});
		let manifest = MigrationManifest {
			abi_version: ABI_VERSION.to_owned(),
			program_id: "program".to_owned(),
			version_type: MigrationVersionType::U8,
			auto: MigrationAuto::none(),
			contracts: BTreeMap::from([(
				ContractIdentity::try_new(ContractKind::Account, 1, 0xAB)
					.unwrap()
					.key(),
				ContractHistory {
					identity: ContractIdentity::try_new(ContractKind::Account, 1, 0xAB).unwrap(),
					rust_name: "Profile".to_owned(),
					versions: vec![version(schema, None), second],
				},
			)]),
		};

		assert!(
			manifest
				.validate()
				.unwrap_err()
				.contains("renames into unknown destination field")
		);
	}

	/// Every frozen fixture must decode into the current model.
	///
	/// This is the gapless-walk guard: a document an older release wrote is
	/// walked forward through the step table, so a missing or mis-wired
	/// converter is a test failure rather than a user's runtime error.
	#[test]
	fn every_frozen_fixture_decodes_to_the_current_model() {
		let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
		let mut walked = 0;

		for entry in
			std::fs::read_dir(&root).unwrap_or_else(|error| panic!("fixtures directory: {error}"))
		{
			let directory = entry
				.unwrap_or_else(|error| panic!("fixture entry: {error}"))
				.path();
			if !directory.is_dir() {
				continue;
			}

			let manifest_path = directory.join("manifest.json");
			let bytes = std::fs::read(&manifest_path)
				.unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display()));
			let manifest = decode_manifest(&bytes)
				.unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display()));
			// The fixture must land on the current version, not merely decode.
			assert_eq!(
				manifest.abi_version,
				ABI_VERSION,
				"{} decoded to {}",
				manifest_path.display(),
				manifest.abi_version
			);

			let ledger_path = directory.join("publications.json");
			let bytes = std::fs::read(&ledger_path)
				.unwrap_or_else(|error| panic!("{}: {error}", ledger_path.display()));
			decode_publication_ledger(&bytes)
				.unwrap_or_else(|error| panic!("{}: {error}", ledger_path.display()));

			walked += 1;
		}

		assert!(walked > 0, "the fixture matrix must not be empty");
	}

	/// A frozen fixture's schema must match the schema generated for the version
	/// the fixture records, so a historical shape stays printable.
	#[test]
	fn frozen_fixture_schemas_match_their_recorded_version() {
		let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
		for entry in
			std::fs::read_dir(&root).unwrap_or_else(|error| panic!("fixtures directory: {error}"))
		{
			let directory = entry
				.unwrap_or_else(|error| panic!("fixture entry: {error}"))
				.path();
			if !directory.is_dir() {
				continue;
			}
			let manifest_path = directory.join("manifest.json");
			let bytes = std::fs::read(&manifest_path)
				.unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display()));
			let manifest = decode_manifest(&bytes)
				.unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display()));
			// Only the current version's shape is representable by these types;
			// an older fixture keeps its own frozen schema alongside it.
			if manifest.abi_version == ABI_VERSION {
				for kind in AbiDocument::ALL {
					let schema_path = directory.join(kind.schema_file_name());
					let frozen = std::fs::read_to_string(&schema_path)
						.unwrap_or_else(|error| panic!("{}: {error}", schema_path.display()));
					let generated = render_document_schema(kind)
						.unwrap_or_else(|error| panic!("render: {error}"));
					assert_eq!(
						frozen,
						generated,
						"{} is stale; regenerate it with `pina abi schema --document {}`",
						schema_path.display(),
						kind.as_str()
					);
				}
			}
		}
	}

	/// The checked-in schema artifact must equal what this build generates, so
	/// the published schema cannot drift from the types that enforce it.
	#[test]
	fn checked_in_schema_artifacts_match_the_generated_schema() {
		for (kind, checked_in) in [
			(
				AbiDocument::Manifest,
				include_str!("../schemas/manifest.schema.json"),
			),
			(
				AbiDocument::Publications,
				include_str!("../schemas/publications.schema.json"),
			),
		] {
			let generated =
				render_document_schema(kind).unwrap_or_else(|error| panic!("render: {error}"));
			assert_eq!(
				generated,
				checked_in,
				"{} is stale; regenerate it with `pina abi schema --document {}`",
				kind.schema_file_name(),
				kind.as_str()
			);
		}
	}

	/// The schema's own `required` list must name exactly the keys a document of
	/// that shape carries, so a consumer validating against it accepts what the
	/// tool writes and rejects a document missing a field.
	#[test]
	fn schema_required_fields_match_a_written_document() {
		let manifest = account_manifest(fixed_schema(&[("value", "u64")]));
		let document = serde_json::to_value(&manifest).unwrap();
		let schema = document_schema(AbiDocument::Manifest);

		let required = schema["required"]
			.as_array()
			.unwrap_or_else(|| panic!("manifest schema must declare required fields"));
		let mut required = required
			.iter()
			.map(|value| {
				value
					.as_str()
					.unwrap_or_else(|| panic!("required entries must be strings"))
					.to_owned()
			})
			.collect::<Vec<_>>();
		required.sort();

		let mut present = document
			.as_object()
			.unwrap_or_else(|| panic!("a document is an object"))
			.keys()
			.cloned()
			.collect::<Vec<_>>();
		// `auto` is skipped when empty, so it is not required.
		present.retain(|key| key != "auto");
		present.sort();

		assert_eq!(required, present);
		assert_eq!(
			schema["additionalProperties"],
			serde_json::Value::Bool(false),
			"the schema must forbid unknown fields to match `deny_unknown_fields`"
		);
	}

	/// A document stamped with a version the table cannot reach must fail
	/// closed rather than deserialize into the wrong shape.
	#[test]
	fn a_hole_in_the_step_table_leaves_the_walk_unreachable() {
		// Manufacture the state a dropped converter would leave behind: a step
		// table whose oldest entry is newer than the version being read. The
		// walk must report the hole instead of claiming it normalized the
		// document.
		let steps = [AbiStep {
			from: "0.30",
			to: "0.99",
			convert: identity_step,
		}];
		let value = serde_json::Value::Object(serde_json::Map::new());
		let error = walk_document_to("migration manifest", "0.20", value, &steps, "0.20", "0.99")
			.unwrap_err();
		assert!(
			error.contains("no ABI converter reaches"),
			"a holed table must fail closed, got: {error}"
		);
	}
}
