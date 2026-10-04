//! Evidence-based compute unit budgets for generated clients.
//!
//! Most Solana transactions request the runtime's default compute unit
//! allowance, and priority fees are charged per requested unit, so they pay for
//! units they never use. Pina already runs every instruction through a real
//! runtime in the program's Surfpool suite. `pina test --record-compute-units`
//! keeps the most expensive successful simulation of each instruction in
//! `compute-units.json`, beside the program's `Cargo.toml`. IDL generation
//! turns each measurement into a limit with [`compute_unit_limit`] and attaches
//! both numbers to the instruction as the `pinaComputeUnits` plugin, which every
//! client generator copies. The limit is computed here and nowhere else.
//!
//! An instruction without a measurement gets no plugin, and its clients request
//! no limit, so the runtime default applies.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::ErrorKind;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use atomic_write_file::AtomicWriteFile;
use codama_nodes::CamelCaseString;
use codama_nodes::RootNode;
use pina_codama_renderer::compute_units::ComputeUnitBudget;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest as _;
use sha2::Sha256;

use crate::error::IdlError;
use crate::ir::InstructionIr;
use crate::project::ProjectError;
use crate::project::compute_units_config;
use crate::workflow::WorkflowError;

/// File name of the recorded measurements, beside the program's `Cargo.toml`.
pub const COMPUTE_UNITS_FILE_NAME: &str = "compute-units.json";

/// Version of the `compute-units.json` layout this release reads and writes.
pub const COMPUTE_UNITS_SCHEMA_VERSION: u32 = 1;

/// How the recorded figures are measured: the maximum over every successful
/// Surfpool simulation of the instruction, each simulated alone in a
/// transaction without compute budget instructions.
pub const COMPUTE_UNITS_MEASUREMENT: &str = "surfpool-simulation-max";

/// Margin added to measurements when `pina.toml` sets none.
pub const DEFAULT_COMPUTE_UNIT_MARGIN_PERCENT: u32 = 20;

/// Compute units a `SetComputeUnitLimit` instruction consumes.
///
/// Measured, not assumed: `pina_test` searches for the smallest limit a
/// transaction carrying the instruction succeeds with.
pub const SET_COMPUTE_UNIT_LIMIT_COST: u32 = 150;

/// Compute units a `SetComputeUnitPrice` instruction consumes, measured the
/// same way.
pub const SET_COMPUTE_UNIT_PRICE_COST: u32 = 150;

/// Compute units every limit reserves for the compute budget instructions a
/// priority-fee transaction carries.
///
/// A limit has to cover the instructions that set it. Priority fees are the
/// reason to request a tight limit, so the reserve covers both the limit and
/// the price instruction. A transaction without a price instruction, or a v1
/// transaction that carries its budget in the message, keeps the difference as
/// headroom.
pub const COMPUTE_BUDGET_OVERHEAD: u32 = SET_COMPUTE_UNIT_LIMIT_COST + SET_COMPUTE_UNIT_PRICE_COST;

/// The most compute units a transaction may request.
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// Errors produced while recording or applying compute unit measurements.
#[derive(Debug, thiserror::Error)]
pub enum ComputeUnitsError {
	#[error(transparent)]
	Workflow(#[from] WorkflowError),

	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error(transparent)]
	Idl(#[from] IdlError),

	#[error("Could not read {path}: {source}")]
	Read {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not write {path}: {source}")]
	Write {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Could not parse {path}: {source}")]
	Parse {
		path: PathBuf,
		source: serde_json::Error,
	},

	#[error(
		"{path} uses compute unit schema version {found}; this release of pina reads version \
		 {COMPUTE_UNITS_SCHEMA_VERSION}"
	)]
	UnsupportedSchema { path: PathBuf, found: u32 },

	#[error(
		"{path} records `{found}` measurements; pina only derives limits from \
		 `{COMPUTE_UNITS_MEASUREMENT}`"
	)]
	UnsupportedMeasurement { path: PathBuf, found: String },

	#[error("{path} line {line} is not a compute unit sample: {reason}")]
	InvalidSample {
		path: PathBuf,
		line: usize,
		reason: String,
	},

	#[error(
		"{path} line {line} was recorded by a pina_test release that does not record \
		 `discriminatorBytes` and `success`; update the Surfpool test package's `pina_test` \
		 dependency and record again"
	)]
	OutdatedSample { path: PathBuf, line: usize },

	#[error(
		"{path} measures {}, which the program does not declare. Run `pina test \
		 --record-compute-units` to measure the current instructions, or remove the stale entries \
		 (or the file) to generate clients without them",
		backticked(.names)
	)]
	UnknownInstructions { path: PathBuf, names: Vec<String> },

	#[error(
		"`{name}` consumed {measured} compute units, which leaves no room for the \
		 {COMPUTE_BUDGET_OVERHEAD} compute budget units under the {MAX_COMPUTE_UNIT_LIMIT} \
		 transaction limit"
	)]
	LimitUnreachable { name: String, measured: u32 },
}

impl ComputeUnitsError {
	/// Return the child exit code when a delegated command failed.
	#[must_use]
	pub fn exit_code(&self) -> i32 {
		match self {
			Self::Workflow(error) => error.exit_code(),
			_ => 1,
		}
	}
}

/// What `pina test --record-compute-units` wrote and what it could not measure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComputeUnitRecording {
	/// The `compute-units.json` file that was written.
	pub path: PathBuf,
	/// IDL names of the instructions that now carry a measurement.
	pub measured: Vec<String>,
	/// IDL names of the instructions no successful sample exercised. Their
	/// clients keep requesting the runtime default.
	pub unmeasured: Vec<String>,
	/// Hex-encoded data prefixes of successful samples that match no
	/// instruction discriminator.
	pub unmatched: Vec<String>,
	/// Samples whose simulated transaction failed. They never count toward a
	/// measurement.
	pub failed_samples: usize,
}

/// The limit a transaction carrying one instruction should request.
///
/// `limit = round_up_to_100(measured × (100 + margin_percent) / 100) +
/// COMPUTE_BUDGET_OVERHEAD`, rounding the margin up rather than down and
/// capping the result at [`MAX_COMPUTE_UNIT_LIMIT`]. Rounding to a hundred
/// keeps limits stable when a measurement moves by a few units.
///
/// Returns `None` when even the measurement plus the compute budget overhead
/// exceeds the transaction limit, because no limit could cover the instruction.
#[must_use]
pub fn compute_unit_limit(measured: u32, margin_percent: u32) -> Option<u32> {
	let measured = u64::from(measured);
	let overhead = u64::from(COMPUTE_BUDGET_OVERHEAD);
	let maximum = u64::from(MAX_COMPUTE_UNIT_LIMIT);

	if measured + overhead > maximum {
		return None;
	}

	// Both factors fit in 32 bits, so the product cannot overflow 64.
	let with_margin = (measured * (100 + u64::from(margin_percent))).div_ceil(100);
	let rounded = with_margin.div_ceil(100) * 100;

	u32::try_from((rounded + overhead).min(maximum)).ok()
}

/// The recorded measurements, as `compute-units.json` stores them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ComputeUnitsFile {
	schema_version: u32,
	measurement: String,
	/// SHA-256 of the SBF artifact the measurements came from, so client
	/// generation can tell when the program has been rebuilt since.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	artifact_sha256: Option<String>,
	/// Keyed by IDL instruction name, so the file stays sorted.
	instructions: BTreeMap<String, InstructionMeasurement>,
}

/// One instruction's measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct InstructionMeasurement {
	/// The most compute units any successful sample consumed.
	compute_units: u32,
	/// How many successful samples the maximum was taken over.
	samples: u32,
}

/// One line of the record `pina_test` appends while the suite runs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedSample {
	program: String,
	compute_units: u64,
	discriminator_bytes: Option<String>,
	success: Option<bool>,
}

/// A recorded sample for the program under measurement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sample {
	/// Leading instruction-data bytes; long enough for any discriminator.
	prefix: Vec<u8>,
	compute_units: u32,
	success: bool,
}

/// Path of the measurement file for the program in `program_dir`.
#[must_use]
pub(crate) fn compute_units_path(program_dir: &Path) -> PathBuf {
	program_dir.join(COMPUTE_UNITS_FILE_NAME)
}

/// Parse the samples `program` recorded; samples of other programs are
/// skipped.
pub(crate) fn parse_samples(
	text: &str,
	program: &str,
	path: &Path,
) -> Result<Vec<Sample>, ComputeUnitsError> {
	let mut samples = Vec::new();

	for (index, line) in text.lines().enumerate() {
		let invalid = |reason: String| invalid_sample(path, index + 1, reason);

		if line.trim().is_empty() {
			continue;
		}

		let recorded: RecordedSample =
			serde_json::from_str(line).map_err(|error| invalid(error.to_string()))?;

		if recorded.program != program {
			continue;
		}

		let (Some(prefix), Some(success)) = (recorded.discriminator_bytes, recorded.success) else {
			return Err(ComputeUnitsError::OutdatedSample {
				path: path.to_path_buf(),
				line: index + 1,
			});
		};
		let prefix = decode_hex(&prefix)
			.ok_or_else(|| invalid(format!("`discriminatorBytes` is not hex: `{prefix}`")))?;
		let compute_units = u32::try_from(recorded.compute_units)
			.map_err(|_| invalid("`computeUnits` exceeds 32 bits".to_owned()))?;

		samples.push(Sample {
			prefix,
			compute_units,
			success,
		});
	}

	Ok(samples)
}

fn invalid_sample(path: &Path, line: usize, reason: String) -> ComputeUnitsError {
	ComputeUnitsError::InvalidSample {
		path: path.to_path_buf(),
		line,
		reason,
	}
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
	if !text.len().is_multiple_of(2) {
		return None;
	}

	(0..text.len())
		.step_by(2)
		.map(|start| u8::from_str_radix(text.get(start..start + 2)?, 16).ok())
		.collect()
}

fn encode_hex(bytes: &[u8]) -> String {
	bytes
		.iter()
		.fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
			let _ = write!(hex, "{byte:02x}");
			hex
		})
}

/// The IDL name and wire discriminator of every instruction.
///
/// A program declares one discriminator width, so no discriminator can be a
/// prefix of another and the match below is unambiguous.
fn instruction_discriminators(instructions: &[InstructionIr]) -> Vec<(String, Vec<u8>)> {
	instructions
		.iter()
		.map(|instruction| {
			let bytes = instruction.discriminator.value.to_le_bytes();
			let width = instruction.discriminator.repr_size.min(bytes.len());

			(
				CamelCaseString::new(&instruction.name).to_string(),
				bytes[..width].to_vec(),
			)
		})
		.collect()
}

/// Reduce samples to one measurement per instruction.
///
/// Each instruction keeps the maximum over its successful samples. Failed
/// samples never count: a transaction that fails usually stops early, so its
/// consumption says nothing about the instruction's real cost.
pub(crate) fn aggregate(
	samples: &[Sample],
	instructions: &[InstructionIr],
	path: &Path,
) -> (
	BTreeMap<String, InstructionMeasurement>,
	ComputeUnitRecording,
) {
	let discriminators = instruction_discriminators(instructions);
	let mut measured = BTreeMap::<String, InstructionMeasurement>::new();
	let mut unmatched = BTreeSet::new();
	let mut failed_samples = 0;

	for sample in samples {
		if !sample.success {
			failed_samples += 1;
			continue;
		}

		let Some((name, _)) = discriminators
			.iter()
			.find(|(_, discriminator)| sample.prefix.starts_with(discriminator))
		else {
			unmatched.insert(encode_hex(&sample.prefix));
			continue;
		};
		let entry = measured
			.entry(name.clone())
			.or_insert(InstructionMeasurement {
				compute_units: 0,
				samples: 0,
			});

		entry.compute_units = entry.compute_units.max(sample.compute_units);
		entry.samples += 1;
	}

	let mut unmeasured = discriminators
		.into_iter()
		.map(|(name, _)| name)
		.filter(|name| !measured.contains_key(name))
		.collect::<Vec<_>>();
	unmeasured.sort();

	let recording = ComputeUnitRecording {
		path: path.to_path_buf(),
		measured: measured.keys().cloned().collect(),
		unmeasured,
		unmatched: unmatched.into_iter().collect(),
		failed_samples,
	};

	(measured, recording)
}

/// Aggregate a finished suite's samples and write `compute-units.json`.
///
/// `artifact` is the SBF program the suite ran; its hash is recorded so a
/// later client generation can tell when the program was rebuilt.
pub(crate) fn write_recording(
	program_dir: &Path,
	program: &str,
	instructions: &[InstructionIr],
	samples_path: &Path,
	artifact: &Path,
) -> Result<ComputeUnitRecording, ComputeUnitsError> {
	let path = compute_units_path(program_dir);
	let text = read_optional(samples_path)?.unwrap_or_default();
	let samples = parse_samples(&text, program, samples_path)?;
	let (instructions, recording) = aggregate(&samples, instructions, &path);
	let artifact_bytes = std::fs::read(artifact).map_err(|source| read_error(artifact, source))?;
	let file = ComputeUnitsFile {
		schema_version: COMPUTE_UNITS_SCHEMA_VERSION,
		measurement: COMPUTE_UNITS_MEASUREMENT.to_owned(),
		artifact_sha256: Some(encode_hex(&Sha256::digest(&artifact_bytes))),
		instructions,
	};

	write_file(&path, &file)?;

	Ok(recording)
}

fn read_optional(path: &Path) -> Result<Option<String>, ComputeUnitsError> {
	match std::fs::read_to_string(path) {
		Ok(text) => Ok(Some(text)),
		Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
		Err(source) => Err(read_error(path, source)),
	}
}

fn read_error(path: &Path, source: std::io::Error) -> ComputeUnitsError {
	ComputeUnitsError::Read {
		path: path.to_path_buf(),
		source,
	}
}

pub(crate) fn write_error(path: &Path, source: std::io::Error) -> ComputeUnitsError {
	ComputeUnitsError::Write {
		path: path.to_path_buf(),
		source,
	}
}

/// Serialize with tab indentation and a trailing newline, the form the
/// repository formatter keeps, so recording again produces no diff when
/// nothing changed.
fn write_file(path: &Path, file: &ComputeUnitsFile) -> Result<(), ComputeUnitsError> {
	let mut bytes = Vec::new();
	let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
	let encoded = file.serialize(&mut serializer);

	encoded.map_err(|source| write_error(path, source.into()))?;
	bytes.push(b'\n');

	let mut output = AtomicWriteFile::open(path).map_err(|source| write_error(path, source))?;
	let written = output.write_all(&bytes);

	written.map_err(|source| write_error(path, source))?;
	output.commit().map_err(|source| write_error(path, source))
}

/// Read and validate the measurements recorded for the program in
/// `program_dir`, if it has any.
pub(crate) fn read_compute_units_file(
	program_dir: &Path,
) -> Result<Option<(PathBuf, ComputeUnitsFile)>, ComputeUnitsError> {
	let path = compute_units_path(program_dir);
	let Some(text) = read_optional(&path)? else {
		return Ok(None);
	};
	let file: ComputeUnitsFile = match serde_json::from_str(&text) {
		Ok(file) => file,
		Err(source) => return Err(ComputeUnitsError::Parse { path, source }),
	};

	if file.schema_version != COMPUTE_UNITS_SCHEMA_VERSION {
		return Err(ComputeUnitsError::UnsupportedSchema {
			path,
			found: file.schema_version,
		});
	}

	if file.measurement != COMPUTE_UNITS_MEASUREMENT {
		return Err(ComputeUnitsError::UnsupportedMeasurement {
			path,
			found: file.measurement,
		});
	}

	Ok(Some((path, file)))
}

/// What IDL generation does with the recorded measurements, which matters
/// once they name an instruction the program no longer declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MeasurementUse {
	/// Attach every measurement, and fail on one for an undeclared
	/// instruction. `pina generate` writes committed clients, where a
	/// silently dropped budget would hide the drift.
	Strict,
	/// Attach the measurements of declared instructions and report the rest,
	/// so renaming an instruction never stops a build, an IDL, or a test run.
	Lenient,
	/// Attach nothing. The recording run replaces the file, so its own build
	/// must not depend on what the old one says.
	Skip,
}

/// Attach the recorded budget of every measured instruction to `root`.
///
/// The margin comes from the `pina.toml` governing `program_dir`. Returns the
/// warning to print when [`MeasurementUse::Lenient`] ignored measurements of
/// undeclared instructions.
///
/// # Errors
///
/// Returns an error when the measurements cannot be read, when an instruction
/// cannot fit a transaction, or when [`MeasurementUse::Strict`] finds a
/// measurement for an instruction the program does not declare.
pub(crate) fn attach_compute_unit_budgets(
	program_dir: &Path,
	root: &mut RootNode,
	measurements: MeasurementUse,
) -> Result<Option<String>, ComputeUnitsError> {
	if measurements == MeasurementUse::Skip {
		return Ok(None);
	}

	let Some((path, file)) = read_compute_units_file(program_dir)? else {
		return Ok(None);
	};
	let margin_percent = compute_units_config(program_dir)?.margin_percent;
	let mut stale = Vec::new();

	for (name, measurement) in file.instructions {
		let Some(instruction) = root
			.program
			.instructions
			.iter_mut()
			.find(|instruction| instruction.name.as_ref() == name)
		else {
			stale.push(name);
			continue;
		};
		let measured = measurement.compute_units;
		let Some(limit) = compute_unit_limit(measured, margin_percent) else {
			return Err(ComputeUnitsError::LimitUnreachable { name, measured });
		};

		instruction
			.plugins
			.push(ComputeUnitBudget { measured, limit }.to_plugin());
	}

	if stale.is_empty() {
		return Ok(None);
	}

	if measurements == MeasurementUse::Strict {
		return Err(ComputeUnitsError::UnknownInstructions { path, names: stale });
	}

	Ok(Some(format!(
		"{} measures {}, which the program does not declare, so those budgets are ignored. Run \
		 `pina test --record-compute-units` to measure the current instructions.",
		path.display(),
		backticked(&stale),
	)))
}

/// Join names as a comma-separated list of code spans.
fn backticked(names: &[String]) -> String {
	names
		.iter()
		.map(|name| format!("`{name}`"))
		.collect::<Vec<_>>()
		.join(", ")
}

/// Explain how the recorded measurements relate to the program built at
/// `artifact`, when they came from a different build.
///
/// Returns `None` when the measurements carry no artifact hash, when nothing
/// is built yet, or when the build is the one that was measured.
pub(crate) fn stale_measurement_warning(
	program_dir: &Path,
	artifact: &Path,
) -> Result<Option<String>, ComputeUnitsError> {
	let Some((path, file)) = read_compute_units_file(program_dir)? else {
		return Ok(None);
	};
	let Some(recorded) = file.artifact_sha256 else {
		return Ok(None);
	};
	let bytes = match std::fs::read(artifact) {
		Ok(bytes) => bytes,
		Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
		Err(source) => return Err(read_error(artifact, source)),
	};

	if encode_hex(&Sha256::digest(&bytes)) == recorded {
		return Ok(None);
	}

	Ok(Some(format!(
		"{} was measured against a different build than {}; its compute unit limits may no \
		 longer fit. Run `pina test --record-compute-units` to measure the current program.",
		path.display(),
		artifact.display(),
	)))
}

#[cfg(test)]
mod tests {
	use std::error::Error;
	use std::fs;

	use codama_nodes::InstructionNode;
	use codama_nodes::ProgramNode;
	use serde_json::json;
	use tempfile::TempDir;

	use super::*;
	use crate::ir::DiscriminatorIr;

	type TestResult = Result<(), Box<dyn Error>>;

	fn instruction(name: &str, value: u64, repr_size: usize) -> InstructionIr {
		InstructionIr {
			name: name.to_owned(),
			rust_name: name.to_owned(),
			accounts: Vec::new(),
			arguments: Vec::new(),
			discriminator: DiscriminatorIr { value, repr_size },
			docs: Vec::new(),
		}
	}

	fn sample(prefix: &[u8], compute_units: u32, success: bool) -> Sample {
		Sample {
			prefix: prefix.to_vec(),
			compute_units,
			success,
		}
	}

	fn record_line(program: &str, prefix: &str, compute_units: u64, success: bool) -> String {
		json!({
			"program": program,
			"discriminator": 0,
			"discriminatorBytes": prefix,
			"computeUnits": compute_units,
			"success": success,
		})
		.to_string()
	}

	fn measurements_json(instructions: &serde_json::Value) -> String {
		json!({
			"schemaVersion": COMPUTE_UNITS_SCHEMA_VERSION,
			"measurement": COMPUTE_UNITS_MEASUREMENT,
			"instructions": instructions,
		})
		.to_string()
	}

	fn counter_root() -> RootNode {
		let instruction = |name: &str| {
			InstructionNode {
				name: name.into(),
				..InstructionNode::default()
			}
		};

		RootNode::new(
			ProgramNode::new(
				"counterProgram",
				"GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS",
			)
			.add_instruction(instruction("initialize"))
			.add_instruction(instruction("increment")),
		)
	}

	#[test]
	fn limits_round_the_margin_up_to_a_hundred_and_add_the_budget_overhead() {
		assert_eq!(COMPUTE_BUDGET_OVERHEAD, 300);
		// 379 × 1.2 = 454.8 → 500, plus 300.
		assert_eq!(compute_unit_limit(379, 20), Some(800));
		// 1,704 × 1.2 = 2,044.8 → 2,100, plus 300.
		assert_eq!(compute_unit_limit(1_704, 20), Some(2_400));
		// An exact hundred stays put.
		assert_eq!(compute_unit_limit(1_000, 20), Some(1_500));
		// The margin itself rounds up: 1,001 × 1.0 needs a second hundred.
		assert_eq!(compute_unit_limit(1_001, 0), Some(1_400));
		assert_eq!(compute_unit_limit(100, 0), Some(400));
		assert_eq!(compute_unit_limit(0, 20), Some(300));
		assert_eq!(compute_unit_limit(1_000, 150), Some(2_800));
	}

	#[test]
	fn limits_cap_at_the_transaction_maximum_and_refuse_what_cannot_fit() {
		assert_eq!(
			compute_unit_limit(1_200_000, 20),
			Some(MAX_COMPUTE_UNIT_LIMIT)
		);
		assert_eq!(
			compute_unit_limit(1_399_700, 20),
			Some(MAX_COMPUTE_UNIT_LIMIT)
		);
		assert_eq!(compute_unit_limit(1_399_701, 0), None);
		assert_eq!(compute_unit_limit(u32::MAX, 20), None);
		// The widest inputs cannot overflow the arithmetic.
		assert_eq!(
			compute_unit_limit(1, u32::MAX),
			Some(MAX_COMPUTE_UNIT_LIMIT)
		);
	}

	#[test]
	fn parses_samples_of_the_measured_program_only() -> TestResult {
		let text = [
			record_line("counter_program", "00fd", 1_704, true),
			String::new(),
			record_line("other_program", "01", 9, true),
			record_line("counter_program", "01", 43, false),
		]
		.join("\n");

		let samples = parse_samples(&text, "counter_program", Path::new("samples.jsonl"))?;

		assert_eq!(
			samples,
			[
				sample(&[0x00, 0xfd], 1_704, true),
				sample(&[0x01], 43, false)
			]
		);

		Ok(())
	}

	#[test]
	fn rejects_samples_it_cannot_attribute() {
		let parse = |text: &str| parse_samples(text, "counter_program", Path::new("samples.jsonl"));

		assert!(matches!(
			parse("\nnot json"),
			Err(ComputeUnitsError::InvalidSample { line: 2, .. })
		));
		assert!(matches!(
			parse(&record_line("counter_program", "0g", 1, true)),
			Err(ComputeUnitsError::InvalidSample { reason, .. }) if reason.contains("not hex")
		));
		assert!(matches!(
			parse(&record_line("counter_program", "012", 1, true)),
			Err(ComputeUnitsError::InvalidSample { reason, .. }) if reason.contains("not hex")
		));
		assert!(matches!(
			parse(&record_line("counter_program", "01", u64::from(u32::MAX) + 1, true)),
			Err(ComputeUnitsError::InvalidSample { reason, .. }) if reason.contains("exceeds 32 bits")
		));

		let outdated = json!({
			"program": "counter_program",
			"discriminator": 1,
			"computeUnits": 379,
		})
		.to_string();
		assert!(matches!(
			parse(&outdated),
			Err(ComputeUnitsError::OutdatedSample { line: 1, .. })
		));
	}

	#[test]
	fn aggregates_the_maximum_successful_sample_per_full_discriminator() {
		// Two-byte discriminators: `0x0102` is `[0x02, 0x01]` on the wire, and
		// a sample that only shares the first byte belongs to neither.
		let instructions = [
			instruction("make_offer", 0x0102, 2),
			instruction("take_offer", 0x0202, 2),
			instruction("cancel_offer", 0x0302, 2),
		];
		let samples = [
			sample(&[0x02, 0x01, 0xaa], 900, true),
			sample(&[0x02, 0x01, 0xbb], 1_200, true),
			sample(&[0x02, 0x01], 5_000, false),
			sample(&[0x02, 0x02], 700, true),
			sample(&[0x02, 0x09], 300, true),
			sample(&[0x02], 100, true),
		];

		let (measured, recording) =
			aggregate(&samples, &instructions, Path::new("compute-units.json"));

		assert_eq!(
			measured,
			BTreeMap::from([
				(
					"makeOffer".to_owned(),
					InstructionMeasurement {
						compute_units: 1_200,
						samples: 2,
					}
				),
				(
					"takeOffer".to_owned(),
					InstructionMeasurement {
						compute_units: 700,
						samples: 1,
					}
				),
			])
		);
		assert_eq!(
			recording,
			ComputeUnitRecording {
				path: PathBuf::from("compute-units.json"),
				measured: vec!["makeOffer".to_owned(), "takeOffer".to_owned()],
				unmeasured: vec!["cancelOffer".to_owned()],
				unmatched: vec!["02".to_owned(), "0209".to_owned()],
				failed_samples: 1,
			}
		);
	}

	#[test]
	fn writes_sorted_measurements_with_the_artifact_hash_and_reads_them_back() -> TestResult {
		let directory = TempDir::new()?;
		let samples = directory.path().join("samples.jsonl");
		let artifact = directory.path().join("counter_program.so");
		let lines = [
			record_line("counter_program", "01", 379, true),
			record_line("counter_program", "00fd", 1_704, true),
			record_line("counter_program", "01", 379, true),
		];
		fs::write(&samples, lines.join("\n"))?;
		fs::write(&artifact, "compiled-sbf")?;

		let recording = write_recording(
			directory.path(),
			"counter_program",
			&[
				instruction("initialize", 0, 1),
				instruction("increment", 1, 1),
			],
			&samples,
			&artifact,
		)?;
		let written = fs::read_to_string(directory.path().join(COMPUTE_UNITS_FILE_NAME))?;

		assert_eq!(
			recording.measured,
			["increment".to_owned(), "initialize".to_owned()]
		);
		assert_eq!(
			written,
			"{\n\t\"schemaVersion\": 1,\n\t\"measurement\": \"surfpool-simulation-max\",\n\t\"artifactSha256\": \"bc4dc0430d4d4b3fe0ccf1a150a8981c45bcc90db35a0fc3b00429c47cfbb9ab\",\n\t\"instructions\": {\n\t\t\"increment\": {\n\t\t\t\"computeUnits\": 379,\n\t\t\t\"samples\": 2\n\t\t},\n\t\t\"initialize\": {\n\t\t\t\"computeUnits\": 1704,\n\t\t\t\"samples\": 1\n\t\t}\n\t}\n}\n"
		);
		assert!(read_compute_units_file(directory.path())?.is_some());

		Ok(())
	}

	#[test]
	fn a_suite_that_records_nothing_measures_nothing() -> TestResult {
		let directory = TempDir::new()?;
		let artifact = directory.path().join("counter_program.so");
		fs::write(&artifact, "compiled-sbf")?;

		let recording = write_recording(
			directory.path(),
			"counter_program",
			&[instruction("increment", 1, 1)],
			&directory.path().join("never-written.jsonl"),
			&artifact,
		)?;

		assert!(recording.measured.is_empty());
		assert_eq!(recording.unmeasured, ["increment".to_owned()]);

		Ok(())
	}

	#[test]
	fn recording_reports_unreadable_inputs_and_unwritable_outputs() -> TestResult {
		let directory = TempDir::new()?;
		let samples = directory.path().join("samples.jsonl");
		fs::write(&samples, "")?;

		assert!(matches!(
			write_recording(
				directory.path(),
				"counter_program",
				&[],
				&samples,
				&directory.path().join("missing.so"),
			),
			Err(ComputeUnitsError::Read { .. })
		));
		assert!(matches!(
			write_recording(
				directory.path(),
				"counter_program",
				&[],
				directory.path(),
				&samples,
			),
			Err(ComputeUnitsError::Read { .. })
		));
		assert!(matches!(
			write_recording(
				&directory.path().join("missing-program"),
				"counter_program",
				&[],
				&samples,
				&samples,
			),
			Err(ComputeUnitsError::Write { .. })
		));

		Ok(())
	}

	#[test]
	fn reads_only_files_in_the_current_schema() -> TestResult {
		let directory = TempDir::new()?;
		let path = directory.path().join(COMPUTE_UNITS_FILE_NAME);
		let read = || read_compute_units_file(directory.path());

		assert!(read()?.is_none(), "a missing file is no measurement");

		fs::write(&path, "{")?;
		assert!(matches!(read(), Err(ComputeUnitsError::Parse { .. })));

		let unknown_field = json!({
			"schemaVersion": 1,
			"measurement": COMPUTE_UNITS_MEASUREMENT,
			"instructions": {},
			"docs": [],
		});
		fs::write(&path, unknown_field.to_string())?;
		assert!(matches!(read(), Err(ComputeUnitsError::Parse { .. })));

		let future_schema = json!({
			"schemaVersion": 2,
			"measurement": COMPUTE_UNITS_MEASUREMENT,
			"instructions": {},
		});
		fs::write(&path, future_schema.to_string())?;
		assert!(matches!(
			read(),
			Err(ComputeUnitsError::UnsupportedSchema { found: 2, .. })
		));

		let other_measurement = json!({
			"schemaVersion": 1,
			"measurement": "mainnet-p99",
			"instructions": {},
		});
		fs::write(&path, other_measurement.to_string())?;
		assert!(matches!(
			read(),
			Err(ComputeUnitsError::UnsupportedMeasurement { .. })
		));

		fs::remove_file(&path)?;
		fs::create_dir(&path)?;
		assert!(matches!(read(), Err(ComputeUnitsError::Read { .. })));

		Ok(())
	}

	#[test]
	fn attaches_each_measured_budget_as_a_plugin_with_the_configured_margin() -> TestResult {
		let directory = TempDir::new()?;
		let measurements = json!({ "increment": { "computeUnits": 379, "samples": 5 } });
		fs::write(
			directory.path().join("pina.toml"),
			"[project]\nprogram = \".\"\n\n[compute_units]\nmargin_percent = 50\n",
		)?;
		fs::write(
			directory.path().join(COMPUTE_UNITS_FILE_NAME),
			measurements_json(&measurements),
		)?;
		let mut root = counter_root();

		let warning =
			attach_compute_unit_budgets(directory.path(), &mut root, MeasurementUse::Strict)?;

		assert_eq!(warning, None);
		// 379 × 1.5 = 568.5 → 600, plus 300.
		assert_eq!(
			serde_json::to_value(&root.program.instructions)?,
			json!([
				{ "kind": "instructionNode", "name": "initialize" },
				{
					"kind": "instructionNode",
					"name": "increment",
					"plugins": [{
						"kind": "pluginNode",
						"name": "pinaComputeUnits",
						"payload": { "measured": 379, "limit": 900 },
					}],
				},
			])
		);

		Ok(())
	}

	#[test]
	fn leaves_the_idl_alone_without_measurements_and_uses_the_default_margin() -> TestResult {
		let directory = TempDir::new()?;
		let measurements = json!({ "initialize": { "computeUnits": 1704, "samples": 6 } });
		let mut root = counter_root();

		attach_compute_unit_budgets(directory.path(), &mut root, MeasurementUse::Strict)?;
		assert_eq!(root, counter_root());

		fs::write(
			directory.path().join(COMPUTE_UNITS_FILE_NAME),
			measurements_json(&measurements),
		)?;
		attach_compute_unit_budgets(directory.path(), &mut root, MeasurementUse::Lenient)?;

		assert_eq!(
			ComputeUnitBudget::from_instruction(&root.program.instructions[0])?,
			Some(ComputeUnitBudget {
				measured: 1_704,
				limit: 2_400,
			})
		);

		Ok(())
	}

	#[test]
	fn treats_measurements_of_undeclared_instructions_by_use() -> TestResult {
		let directory = TempDir::new()?;
		let stale = json!({
			"decrement": { "computeUnits": 10, "samples": 1 },
			"increment": { "computeUnits": 379, "samples": 5 },
			"reset": { "computeUnits": 20, "samples": 1 },
		});
		fs::write(
			directory.path().join(COMPUTE_UNITS_FILE_NAME),
			measurements_json(&stale),
		)?;

		// `pina generate` refuses, naming every stale entry and the remedies.
		let error = attach_compute_unit_budgets(
			directory.path(),
			&mut counter_root(),
			MeasurementUse::Strict,
		)
		.expect_err("generation must refuse stale measurements");
		assert!(matches!(
			&error,
			ComputeUnitsError::UnknownInstructions { names, .. }
				if names == &["decrement".to_owned(), "reset".to_owned()]
		));
		let message = error.to_string();
		assert!(message.contains("`decrement`, `reset`"));
		assert!(message.contains("Run `pina test --record-compute-units`"));
		assert!(message.contains("remove the stale entries"));

		// A build, an IDL, or a test keeps the declared budgets and warns.
		let mut root = counter_root();
		let warning =
			attach_compute_unit_budgets(directory.path(), &mut root, MeasurementUse::Lenient)?
				.ok_or("stale measurements must be reported")?;
		assert!(warning.contains("`decrement`, `reset`"));
		assert!(warning.contains("ignored"));
		assert!(root.program.instructions[0].plugins.is_empty());
		assert_eq!(
			ComputeUnitBudget::from_instruction(&root.program.instructions[1])?,
			Some(ComputeUnitBudget {
				measured: 379,
				limit: 800,
			})
		);

		// The recording run reads nothing, so even a corrupt file cannot stop
		// the run that replaces it.
		fs::write(directory.path().join(COMPUTE_UNITS_FILE_NAME), "{")?;
		let mut root = counter_root();
		assert_eq!(
			attach_compute_unit_budgets(directory.path(), &mut root, MeasurementUse::Skip)?,
			None
		);
		assert_eq!(root, counter_root());

		Ok(())
	}

	#[test]
	fn refuses_measurements_the_program_cannot_use() -> TestResult {
		let directory = TempDir::new()?;
		let path = directory.path().join(COMPUTE_UNITS_FILE_NAME);
		let attach = || {
			attach_compute_unit_budgets(
				directory.path(),
				&mut counter_root(),
				MeasurementUse::Lenient,
			)
		};

		fs::write(
			&path,
			measurements_json(&json!({ "increment": { "computeUnits": 1_399_900, "samples": 1 } })),
		)?;
		let error = attach().expect_err("an instruction that cannot fit must fail");
		assert!(matches!(
			&error,
			ComputeUnitsError::LimitUnreachable { name, measured: 1_399_900 } if name == "increment"
		));
		assert!(error.to_string().contains("1400000"));

		fs::write(
			directory.path().join("pina.toml"),
			"[compute_units]\nmargin = 5\n",
		)?;
		assert!(matches!(attach(), Err(ComputeUnitsError::Project(_))));

		Ok(())
	}

	#[test]
	fn warns_only_when_the_built_artifact_is_not_the_measured_one() -> TestResult {
		let directory = TempDir::new()?;
		let artifact = directory.path().join("counter_program.so");
		let path = directory.path().join(COMPUTE_UNITS_FILE_NAME);
		let warning = || stale_measurement_warning(directory.path(), &artifact);
		let hashed = json!({
			"schemaVersion": 1,
			"measurement": COMPUTE_UNITS_MEASUREMENT,
			"artifactSha256": encode_hex(&Sha256::digest(b"compiled-sbf")),
			"instructions": {},
		});

		assert_eq!(warning()?, None, "nothing was measured");

		fs::write(&path, measurements_json(&json!({})))?;
		fs::write(&artifact, "compiled-sbf")?;
		assert_eq!(
			warning()?,
			None,
			"measurements without a hash cannot go stale"
		);

		fs::write(&path, hashed.to_string())?;
		assert_eq!(warning()?, None, "the measured build is current");

		fs::write(&artifact, "rebuilt-sbf")?;
		let message = warning()?.ok_or("a rebuilt program must warn")?;
		assert!(message.contains("pina test --record-compute-units"));
		assert!(message.contains("counter_program.so"));

		fs::remove_file(&artifact)?;
		assert_eq!(warning()?, None, "nothing built yet");

		fs::create_dir(&artifact)?;
		assert!(matches!(warning(), Err(ComputeUnitsError::Read { .. })));

		Ok(())
	}

	#[test]
	fn delegated_failures_keep_their_exit_code() {
		#[cfg(unix)]
		{
			use std::os::unix::process::ExitStatusExt;

			let failed = ComputeUnitsError::Workflow(WorkflowError::CommandFailed {
				program: "cargo".to_owned(),
				status: std::process::ExitStatus::from_raw(23 << 8),
			});
			assert_eq!(failed.exit_code(), 23);
		}

		let unknown = ComputeUnitsError::UnknownInstructions {
			path: PathBuf::from(COMPUTE_UNITS_FILE_NAME),
			names: vec!["decrement".to_owned()],
		};
		assert_eq!(unknown.exit_code(), 1);
	}
}
