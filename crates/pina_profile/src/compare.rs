//! Baseline loading and delta-oriented profile comparison.
//!
//! A baseline is a saved profiler report. Reports written by
//! `pina profile --json --output` are accepted directly; reports wrapped in a
//! versioned [`BaselineDocument`] carry an explicit `schema_version` marker so
//! future format changes can be detected instead of silently misread.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use crate::cost::FunctionProfile;
use crate::cost::ProgramProfile;

/// Current schema version written into versioned baseline documents.
pub const BASELINE_SCHEMA_VERSION: u64 = 1;

/// Default absolute CU regression that triggers a failing exit status.
///
/// Mirrors the failure threshold in `scripts/compute-unit-policy.json` so a
/// local `pina profile compare` reproduces the CI compute-unit gate.
pub const DEFAULT_FAIL_DELTA_CU: u64 = 500;

/// Default percentage CU regression that triggers a failing exit status.
///
/// Both the absolute and percentage thresholds must be reached, matching the
/// CI policy in `scripts/compute-unit-policy.json`.
pub const DEFAULT_FAIL_DELTA_PERCENT: f64 = 10.0;

/// Schema version marker for the comparison report document.
pub const COMPARISON_SCHEMA_VERSION: u64 = 1;

/// A saved profile report wrapped with an explicit schema version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineDocument {
	/// Baseline format version. Must be [`BASELINE_SCHEMA_VERSION`].
	pub schema_version: u64,

	/// The saved profiler report.
	pub profile: ProgramProfile,
}

/// A baseline loaded from disk.
#[derive(Debug, Clone)]
pub struct Baseline {
	/// `Some` when the file carried an explicit schema version marker, `None`
	/// when the report was a bare legacy `ProgramProfile` document.
	pub schema_version: Option<u64>,

	/// The saved profiler report.
	pub profile: ProgramProfile,
}

/// Errors produced while loading a baseline report.
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
	/// The baseline file could not be read.
	#[error("failed to read baseline {path:?}: {source}")]
	Io {
		path: std::path::PathBuf,
		source: std::io::Error,
	},

	/// The baseline file is not valid JSON.
	#[error("baseline {path:?} is not valid JSON: {source}")]
	Json {
		path: std::path::PathBuf,
		source: serde_json::Error,
	},

	/// The baseline declares an unsupported schema version.
	#[error(
		"baseline {path:?} declares unsupported schema version {found} (expected {expected}); \
		 regenerate the baseline with this version of Pina"
	)]
	UnsupportedVersion {
		path: std::path::PathBuf,
		found: u64,
		expected: u64,
	},

	/// The baseline is neither a versioned document nor a bare profile report.
	#[error("baseline {path:?} is not a pina profile report: {message}")]
	NotAProfile {
		path: std::path::PathBuf,
		message: String,
	},
}

/// Read a baseline report from `path`.
///
/// Accepts either a versioned [`BaselineDocument`] or a bare
/// [`ProgramProfile`] report as previously written by `pina profile --json`.
///
/// # Errors
///
/// Returns [`BaselineError`] when the file cannot be read, is not valid JSON,
/// declares an unsupported schema version, or does not contain a profile.
pub fn load_baseline(path: &Path) -> Result<Baseline, BaselineError> {
	let text = std::fs::read_to_string(path).map_err(|source| {
		BaselineError::Io {
			path: path.to_path_buf(),
			source,
		}
	})?;
	let value: serde_json::Value = serde_json::from_str(&text).map_err(|source| {
		BaselineError::Json {
			path: path.to_path_buf(),
			source,
		}
	})?;

	if value.get("schema_version").is_some() {
		// Reject an unknown version before validating the rest of the document
		// so the error points at the real problem.
		let found = value
			.get("schema_version")
			.and_then(serde_json::Value::as_u64);

		if found.is_some_and(|found| found != BASELINE_SCHEMA_VERSION) {
			return Err(BaselineError::UnsupportedVersion {
				path: path.to_path_buf(),
				found: found.unwrap_or_default(),
				expected: BASELINE_SCHEMA_VERSION,
			});
		}

		let document: BaselineDocument = serde_json::from_value(value).map_err(|source| {
			BaselineError::NotAProfile {
				path: path.to_path_buf(),
				message: source.to_string(),
			}
		})?;

		return Ok(Baseline {
			schema_version: Some(document.schema_version),
			profile: document.profile,
		});
	}

	let profile: ProgramProfile = serde_json::from_value(value).map_err(|source| {
		BaselineError::NotAProfile {
			path: path.to_path_buf(),
			message: source.to_string(),
		}
	})?;

	Ok(Baseline {
		schema_version: None,
		profile,
	})
}

/// Wrap a profile report in a versioned baseline document.
#[must_use]
pub fn baseline_document(profile: ProgramProfile) -> BaselineDocument {
	BaselineDocument {
		schema_version: BASELINE_SCHEMA_VERSION,
		profile,
	}
}

/// Signed percentage change from `base` to `head`.
///
/// Positive values are regressions. A zero base is treated like the CI
/// compute-unit comparison: 0 to 0 is unchanged and anything appearing from a
/// zero base is a 100% increase.
#[must_use]
pub fn delta_percent(base: u64, head: u64) -> f64 {
	if base == 0 {
		return if head == 0 { 0.0 } else { 100.0 };
	}

	(((head as f64) - (base as f64)) / (base as f64)) * 100.0
}

/// Regression gate applied to the program total.
///
/// A threshold is reached only when the absolute CU increase and the
/// percentage increase both reach their limits, mirroring the CI
/// compute-unit policy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RegressionThreshold {
	/// Absolute CU increase.
	pub delta_cu: u64,
	/// Percentage CU increase.
	pub delta_percent: f64,
}

impl Default for RegressionThreshold {
	fn default() -> Self {
		Self {
			delta_cu: DEFAULT_FAIL_DELTA_CU,
			delta_percent: DEFAULT_FAIL_DELTA_PERCENT,
		}
	}
}

impl RegressionThreshold {
	/// Whether a total increase of `delta_cu` CU / `delta_percent` percent is
	/// at or above this threshold.
	#[must_use]
	pub fn is_exceeded(self, delta_cu: i64, delta_percent: f64) -> bool {
		if delta_cu <= 0 {
			return false;
		}

		let regression_cu = delta_cu as u64;

		regression_cu >= self.delta_cu && delta_percent >= self.delta_percent
	}
}

/// Overall comparison outcome for the program total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonStatus {
	/// Baseline and current totals are identical.
	Unchanged,
	/// The current total is strictly lower than the baseline.
	Improved,
	/// The current total increased but stayed below the failure threshold.
	Regression,
	/// The current increase reached the configured failure threshold.
	ThresholdRegression,
}

/// Whether a function was added, removed, changed, or unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionChange {
	/// Present only in the current profile.
	Added,
	/// Present only in the baseline profile.
	Removed,
	/// Present in both with a different estimated CU.
	Changed,
	/// Present in both with identical estimated CU.
	Unchanged,
}

/// Absolute totals of one profile report.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct ProgramTotals {
	/// Total estimated CU.
	pub total_cu: u64,
	/// Total instruction count.
	pub total_instructions: u64,
	/// Total syscall count.
	pub total_syscalls: u64,
	/// Binary size in bytes.
	pub binary_size: u64,
	/// Text section size in bytes.
	pub text_size: u64,
}

impl ProgramTotals {
	/// Collect the totals of a profile report.
	#[must_use]
	pub fn of(profile: &ProgramProfile) -> Self {
		Self {
			total_cu: profile.total_cu,
			total_instructions: profile.total_instructions,
			total_syscalls: profile.total_syscalls,
			binary_size: profile.binary_size,
			text_size: profile.text_size,
		}
	}
}

/// Total-level deltas between two profile reports.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TotalsDelta {
	/// Baseline totals.
	pub baseline: ProgramTotals,
	/// Current totals.
	pub current: ProgramTotals,
	/// Change in estimated CU (current minus baseline).
	pub delta_cu: i64,
	/// Percentage change in estimated CU.
	pub delta_percent: f64,
	/// Change in instruction count.
	pub delta_instructions: i64,
	/// Change in syscall count.
	pub delta_syscalls: i64,
	/// Change in binary size in bytes.
	pub delta_binary_size: i64,
	/// Change in text section size in bytes.
	pub delta_text_size: i64,
}

/// Per-function delta between two profile reports.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDelta {
	/// Function symbol name.
	pub name: String,
	/// How the function changed.
	pub change: FunctionChange,
	/// Baseline estimated CU, or `None` for added functions.
	pub baseline_cu: Option<u64>,
	/// Current estimated CU, or `None` for removed functions.
	pub current_cu: Option<u64>,
	/// Change in estimated CU (current minus baseline).
	pub delta_cu: i64,
	/// Percentage change in estimated CU.
	pub delta_percent: f64,
	/// Baseline instruction count, or `None` for added functions.
	pub baseline_instructions: Option<u64>,
	/// Current instruction count, or `None` for removed functions.
	pub current_instructions: Option<u64>,
	/// Change in instruction count.
	pub delta_instructions: i64,
}

/// Machine-readable comparison of a current profile against a baseline.
///
/// Functions are matched by symbol name and sorted by the magnitude of their
/// CU delta (descending), breaking ties by name, so the document is stable
/// and ordered.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
	/// Comparison document schema version.
	pub schema_version: u64,
	/// Program name recorded in the baseline.
	pub baseline_program_name: String,
	/// Program name of the current profile.
	pub current_program_name: String,
	/// Total-level snapshots and deltas.
	pub totals: TotalsDelta,
	/// Overall outcome including threshold evaluation.
	pub status: ComparisonStatus,
	/// The threshold that was applied.
	pub threshold: RegressionThreshold,
	/// Whether `status` is [`ComparisonStatus::ThresholdRegression`].
	pub exceeds_threshold: bool,
	/// Per-function deltas sorted by delta magnitude (descending).
	pub functions: Vec<FunctionDelta>,
}

/// Compare a current profile against a baseline under `threshold`.
#[must_use]
pub fn compare_profiles(
	baseline: &ProgramProfile,
	current: &ProgramProfile,
	threshold: RegressionThreshold,
) -> ComparisonReport {
	let baseline_functions: BTreeMap<&str, &FunctionProfile> = baseline
		.functions
		.iter()
		.map(|function| (function.name.as_str(), function))
		.collect();
	let current_functions: BTreeMap<&str, &FunctionProfile> = current
		.functions
		.iter()
		.map(|function| (function.name.as_str(), function))
		.collect();

	let mut functions = Vec::new();

	for (name, baseline_function) in &baseline_functions {
		let delta = match current_functions.get(name) {
			Some(current_function) => function_delta(baseline_function, Some(current_function)),
			None => function_delta(baseline_function, None),
		};
		functions.push(delta);
	}

	for (name, current_function) in &current_functions {
		if !baseline_functions.contains_key(name) {
			functions.push(added_function_delta(current_function));
		}
	}

	functions.sort_by(|left, right| {
		right
			.delta_cu
			.abs()
			.cmp(&left.delta_cu.abs())
			.then_with(|| left.name.cmp(&right.name))
			.then_with(|| change_rank(left.change).cmp(&change_rank(right.change)))
	});

	let totals = TotalsDelta {
		baseline: ProgramTotals::of(baseline),
		current: ProgramTotals::of(current),
		delta_cu: signed_delta(baseline.total_cu, current.total_cu),
		delta_percent: delta_percent(baseline.total_cu, current.total_cu),
		delta_instructions: signed_delta(baseline.total_instructions, current.total_instructions),
		delta_syscalls: signed_delta(baseline.total_syscalls, current.total_syscalls),
		delta_binary_size: signed_delta(baseline.binary_size, current.binary_size),
		delta_text_size: signed_delta(baseline.text_size, current.text_size),
	};
	let exceeds_threshold = threshold.is_exceeded(totals.delta_cu, totals.delta_percent);
	let status = if totals.delta_cu == 0 {
		ComparisonStatus::Unchanged
	} else if totals.delta_cu < 0 {
		ComparisonStatus::Improved
	} else if exceeds_threshold {
		ComparisonStatus::ThresholdRegression
	} else {
		ComparisonStatus::Regression
	};

	ComparisonReport {
		schema_version: COMPARISON_SCHEMA_VERSION,
		baseline_program_name: baseline.program_name.clone(),
		current_program_name: current.program_name.clone(),
		totals,
		status,
		threshold,
		exceeds_threshold,
		functions,
	}
}

/// Number of functions whose estimated CU changed, were added, or removed.
#[must_use]
pub fn changed_function_count(report: &ComparisonReport) -> usize {
	report
		.functions
		.iter()
		.filter(|function| function.change != FunctionChange::Unchanged)
		.count()
}

fn function_delta(baseline: &FunctionProfile, current: Option<&FunctionProfile>) -> FunctionDelta {
	let Some(current) = current else {
		return FunctionDelta {
			name: baseline.name.clone(),
			change: FunctionChange::Removed,
			baseline_cu: Some(baseline.estimated_cu),
			current_cu: None,
			delta_cu: -(baseline.estimated_cu as i64),
			delta_percent: delta_percent(baseline.estimated_cu, 0),
			baseline_instructions: Some(baseline.instruction_count),
			current_instructions: None,
			delta_instructions: -(baseline.instruction_count as i64),
		};
	};

	let delta_cu = signed_delta(baseline.estimated_cu, current.estimated_cu);
	let delta_percent = delta_percent(baseline.estimated_cu, current.estimated_cu);
	let change = if delta_cu == 0 {
		FunctionChange::Unchanged
	} else {
		FunctionChange::Changed
	};

	FunctionDelta {
		name: baseline.name.clone(),
		change,
		baseline_cu: Some(baseline.estimated_cu),
		current_cu: Some(current.estimated_cu),
		delta_cu,
		delta_percent,
		baseline_instructions: Some(baseline.instruction_count),
		current_instructions: Some(current.instruction_count),
		delta_instructions: signed_delta(baseline.instruction_count, current.instruction_count),
	}
}

fn added_function_delta(current: &FunctionProfile) -> FunctionDelta {
	FunctionDelta {
		name: current.name.clone(),
		change: FunctionChange::Added,
		baseline_cu: None,
		current_cu: Some(current.estimated_cu),
		delta_cu: current.estimated_cu as i64,
		delta_percent: delta_percent(0, current.estimated_cu),
		baseline_instructions: None,
		current_instructions: Some(current.instruction_count),
		delta_instructions: current.instruction_count as i64,
	}
}

fn signed_delta(base: u64, head: u64) -> i64 {
	head as i64 - base as i64
}

fn change_rank(change: FunctionChange) -> u8 {
	match change {
		FunctionChange::Unchanged => 0,
		FunctionChange::Changed => 1,
		FunctionChange::Added | FunctionChange::Removed => 2,
	}
}

/// Write the comparison report as JSON.
///
/// The document is stable: struct fields serialize in declaration order and
/// `functions` is sorted, so identical inputs always produce identical bytes.
///
/// # Errors
///
/// Returns an IO error if writing fails, or a serialization error if JSON
/// formatting fails.
pub fn write_comparison_json(
	report: &ComparisonReport,
	writer: &mut dyn Write,
) -> Result<(), crate::output::OutputError> {
	let json = serde_json::to_string_pretty(report).map_err(crate::output::OutputError::Json)?;
	writeln!(writer, "{json}")?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::io::Write;

	use super::*;

	fn function(name: &str, offset: u64, size: u64, instructions: u64, cu: u64) -> FunctionProfile {
		FunctionProfile {
			name: name.to_owned(),
			offset,
			size,
			instruction_count: instructions,
			syscall_count: 0,
			estimated_cu: cu,
		}
	}

	fn profile(program_name: &str, functions: Vec<FunctionProfile>) -> ProgramProfile {
		let total_instructions = functions.iter().map(|f| f.instruction_count).sum();
		let total_syscalls = functions.iter().map(|f| f.syscall_count).sum();
		let total_cu = functions.iter().map(|f| f.estimated_cu).sum();

		ProgramProfile {
			program_name: program_name.to_owned(),
			binary_size: 1000,
			text_size: total_instructions * 8,
			total_instructions,
			total_syscalls,
			total_cu,
			functions,
		}
	}

	fn strict_threshold() -> RegressionThreshold {
		RegressionThreshold {
			delta_cu: 5,
			delta_percent: 10.0,
		}
	}

	fn write_json_file(content: &[u8]) -> tempfile::NamedTempFile {
		let mut file = tempfile::Builder::new()
			.suffix(".json")
			.tempfile()
			.unwrap_or_else(|error| panic!("temp file failed: {error}"));
		file.write_all(content)
			.unwrap_or_else(|error| panic!("write failed: {error}"));
		file.flush()
			.unwrap_or_else(|error| panic!("flush failed: {error}"));
		file
	}

	#[test]
	fn identical_profiles_report_unchanged() {
		let baseline = profile(
			"demo",
			vec![
				function("process_instruction", 0, 160, 20, 20),
				function("helper", 160, 80, 10, 10),
			],
		);
		let current = baseline.clone();
		let report = compare_profiles(&baseline, &current, strict_threshold());

		assert_eq!(report.status, ComparisonStatus::Unchanged);
		assert!(!report.exceeds_threshold);
		assert_eq!(report.totals.delta_cu, 0);
		assert!(report.totals.delta_percent.abs() < f64::EPSILON);
		assert_eq!(changed_function_count(&report), 0);
		assert!(
			report
				.functions
				.iter()
				.all(|function| function.change == FunctionChange::Unchanged)
		);
	}

	#[test]
	fn regressions_improvements_additions_and_removals_are_classified() {
		let baseline = profile(
			"demo",
			vec![
				function("grew", 0, 160, 20, 20),
				function("shrank", 160, 160, 20, 20),
				function("removed", 320, 80, 10, 10),
				function("same", 400, 80, 10, 10),
			],
		);
		let current = profile(
			"demo",
			vec![
				function("grew", 0, 240, 30, 30),
				function("shrank", 0, 80, 10, 10),
				function("same", 80, 80, 10, 10),
				function("added", 160, 80, 10, 10),
			],
		);
		let report = compare_profiles(&baseline, &current, RegressionThreshold::default());

		let by_name = |name: &str| {
			report
				.functions
				.iter()
				.find(|function| function.name == name)
				.unwrap_or_else(|| panic!("missing function {name}"))
		};

		assert_eq!(by_name("grew").change, FunctionChange::Changed);
		assert_eq!(by_name("grew").delta_cu, 10);
		assert!((by_name("grew").delta_percent - 50.0).abs() < f64::EPSILON);
		assert_eq!(by_name("shrank").change, FunctionChange::Changed);
		assert_eq!(by_name("shrank").delta_cu, -10);
		assert!((by_name("shrank").delta_percent + 50.0).abs() < f64::EPSILON);
		assert_eq!(by_name("removed").change, FunctionChange::Removed);
		assert_eq!(by_name("removed").baseline_cu, Some(10));
		assert_eq!(by_name("removed").current_cu, None);
		assert_eq!(by_name("removed").delta_cu, -10);
		assert!((by_name("removed").delta_percent + 100.0).abs() < f64::EPSILON);
		assert_eq!(by_name("added").change, FunctionChange::Added);
		assert_eq!(by_name("added").baseline_cu, None);
		assert_eq!(by_name("added").current_cu, Some(10));
		assert_eq!(by_name("added").delta_cu, 10);
		assert_eq!(by_name("same").change, FunctionChange::Unchanged);

		// +10 -10 +10 -10 cancel out; totals are unchanged overall.
		assert_eq!(report.totals.delta_cu, 0);
		assert_eq!(report.status, ComparisonStatus::Unchanged);
	}

	#[test]
	fn functions_sort_by_delta_magnitude_then_name() {
		let baseline = profile(
			"demo",
			vec![
				function("small_a", 0, 80, 10, 10),
				function("small_b", 80, 80, 10, 10),
				function("big", 160, 800, 100, 100),
			],
		);
		let current = profile(
			"demo",
			vec![
				function("small_b", 0, 160, 20, 20),
				function("small_a", 160, 160, 20, 20),
				function("big", 320, 1600, 200, 200),
			],
		);
		let report = compare_profiles(&baseline, &current, RegressionThreshold::default());

		let names: Vec<&str> = report.functions.iter().map(|f| f.name.as_str()).collect();

		// Equal-magnitude deltas tie-break by name instead of input order.
		assert_eq!(names, vec!["big", "small_a", "small_b"]);
	}

	#[test]
	fn threshold_requires_both_absolute_and_percentage_increases() {
		let threshold = RegressionThreshold {
			delta_cu: 100,
			delta_percent: 10.0,
		};

		// +100 CU from 10_000 CU is only +1%.
		assert!(!threshold.is_exceeded(100, 1.0));
		// +100 CU from 500 CU is +20%.
		assert!(threshold.is_exceeded(100, 20.0));
		// Huge percentage from a tiny absolute increase.
		assert!(!threshold.is_exceeded(10, 100.0));
		// Improvements never exceed the threshold.
		assert!(!threshold.is_exceeded(-500, -50.0));
		// Zero delta never exceeds the threshold.
		assert!(!threshold.is_exceeded(0, 0.0));
	}

	#[test]
	fn improvement_beats_a_strict_threshold() {
		let baseline = profile("demo", vec![function("entry", 0, 800, 100, 100)]);
		let current = profile("demo", vec![function("entry", 0, 160, 20, 20)]);
		let report = compare_profiles(&baseline, &current, strict_threshold());

		assert_eq!(report.status, ComparisonStatus::Improved);
		assert!(!report.exceeds_threshold);
		assert_eq!(report.totals.delta_cu, -80);
	}

	#[test]
	fn regression_beyond_threshold_is_flagged() {
		let baseline = profile("demo", vec![function("entry", 0, 160, 20, 20)]);
		let current = profile("demo", vec![function("entry", 0, 800, 100, 100)]);
		let report = compare_profiles(&baseline, &current, strict_threshold());

		assert_eq!(report.status, ComparisonStatus::ThresholdRegression);
		assert!(report.exceeds_threshold);
		assert_eq!(report.totals.delta_cu, 80);
		assert!((report.totals.delta_percent - 400.0).abs() < f64::EPSILON);

		let relaxed = RegressionThreshold {
			delta_cu: 500,
			delta_percent: 10.0,
		};
		let small = compare_profiles(&baseline, &current, relaxed);

		assert_eq!(small.status, ComparisonStatus::Regression);
		assert!(!small.exceeds_threshold);
	}

	#[test]
	fn default_threshold_matches_the_ci_fail_policy() {
		let threshold = RegressionThreshold::default();

		assert_eq!(threshold.delta_cu, DEFAULT_FAIL_DELTA_CU);
		assert!((threshold.delta_percent - DEFAULT_FAIL_DELTA_PERCENT).abs() < f64::EPSILON);
	}

	#[test]
	fn delta_percent_treats_a_zero_base_like_the_ci_policy() {
		assert!(delta_percent(0, 0).abs() < f64::EPSILON);
		assert!((delta_percent(0, 40) - 100.0).abs() < f64::EPSILON);
		assert!(delta_percent(20, 20).abs() < f64::EPSILON);
		assert!((delta_percent(20, 40) - 100.0).abs() < f64::EPSILON);
		assert!((delta_percent(40, 20) + 50.0).abs() < f64::EPSILON);
	}

	#[test]
	fn comparison_json_is_stable_and_ordered() {
		let baseline = profile(
			"demo",
			vec![
				function("small_a", 0, 80, 10, 10),
				function("small_b", 80, 80, 10, 10),
			],
		);
		let current = profile(
			"demo",
			vec![
				function("small_b", 0, 160, 20, 20),
				function("small_a", 160, 96, 12, 12),
			],
		);
		let report = compare_profiles(&baseline, &current, strict_threshold());

		let mut first = Vec::new();
		write_comparison_json(&report, &mut first)
			.unwrap_or_else(|error| panic!("json write failed: {error}"));
		let mut second = Vec::new();
		write_comparison_json(&report, &mut second)
			.unwrap_or_else(|error| panic!("json write failed: {error}"));

		assert_eq!(first, second);

		let parsed: serde_json::Value =
			serde_json::from_slice(&first).unwrap_or_else(|error| panic!("invalid JSON: {error}"));
		let names = parsed["functions"]
			.as_array()
			.unwrap_or_else(|| panic!("functions must be an array"))
			.iter()
			.map(|function| function["name"].as_str().unwrap_or_default().to_owned())
			.collect::<Vec<_>>();

		// small_b grew by +10 CU and must sort before small_a's +2 CU.
		assert_eq!(names, vec!["small_b", "small_a"]);
		assert_eq!(parsed["schema_version"], COMPARISON_SCHEMA_VERSION);
		assert_eq!(parsed["status"], "threshold-regression");
		assert_eq!(parsed["exceeds_threshold"], true);
		assert_eq!(parsed["totals"]["delta_cu"], 12);
		assert_eq!(parsed["functions"][0]["change"], "changed");
	}

	#[test]
	fn load_baseline_accepts_bare_profile_reports() {
		let profile = profile("demo", vec![function("entry", 0, 160, 20, 20)]);
		let json = serde_json::to_string_pretty(&profile)
			.unwrap_or_else(|error| panic!("serialize failed: {error}"));
		let file = write_json_file(json.as_bytes());
		let baseline = load_baseline(file.path())
			.unwrap_or_else(|error| panic!("legacy baseline rejected: {error}"));

		assert_eq!(baseline.schema_version, None);
		assert_eq!(baseline.profile, profile);
	}

	#[test]
	fn load_baseline_accepts_versioned_documents() {
		let profile = profile("demo", vec![function("entry", 0, 160, 20, 20)]);
		let document = baseline_document(profile.clone());
		let json = serde_json::to_string_pretty(&document)
			.unwrap_or_else(|error| panic!("serialize failed: {error}"));
		let file = write_json_file(json.as_bytes());
		let baseline = load_baseline(file.path())
			.unwrap_or_else(|error| panic!("versioned baseline rejected: {error}"));

		assert_eq!(baseline.schema_version, Some(BASELINE_SCHEMA_VERSION));
		assert_eq!(baseline.profile, profile);
	}

	#[test]
	fn load_baseline_rejects_invalid_json_clearly() {
		let file = write_json_file(b"not json at all");
		let error = load_baseline(file.path()).expect_err("invalid JSON must fail");

		assert!(matches!(error, BaselineError::Json { .. }));
		assert!(error.to_string().contains("is not valid JSON"));
	}

	#[test]
	fn load_baseline_rejects_unsupported_schema_versions() {
		let file = write_json_file(b"{\"schema_version\": 999}");
		let error = load_baseline(file.path()).expect_err("future version must fail");

		assert!(matches!(
			error,
			BaselineError::UnsupportedVersion {
				found: 999,
				expected: BASELINE_SCHEMA_VERSION,
				..
			}
		));
		assert!(error.to_string().contains("unsupported schema version 999"));
	}

	#[test]
	fn load_baseline_rejects_non_profile_documents() {
		let file = write_json_file(b"{\"hello\": \"world\"}");
		let error = load_baseline(file.path()).expect_err("non-profile must fail");

		assert!(matches!(error, BaselineError::NotAProfile { .. }));
		assert!(error.to_string().contains("is not a pina profile report"));
	}

	#[test]
	fn load_baseline_reports_missing_files() {
		let error = load_baseline(Path::new("/nonexistent/pina-baseline.json"))
			.expect_err("missing file must fail");

		assert!(matches!(error, BaselineError::Io { .. }));
	}
}
