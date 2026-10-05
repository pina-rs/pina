//! The rehearsal report: classification, the stable JSON document, and the
//! terminal rendering.
//!
//! Classification compares two runs of the same signed transaction on the same
//! forked state, so any difference is a property of the binaries:
//!
//! - `unchanged`: identical outcome, written state, and compute units.
//! - `cu_changed`: both runs succeed with identical state but different compute
//!   units. Reported, never a failure.
//! - `state_changed`: both runs succeed but leave a writable account different.
//! - `outcome_changed`: one run fails and the other succeeds, or both fail with
//!   different errors. Error codes are part of a program's observable contract,
//!   so a changed error is a behaviour change even when both runs fail.
//! - `skipped`: the transaction was not compared. The common case is a
//!   transaction that fails identically in both runs because the state it
//!   needed has moved on (an account it initialized now exists); it says
//!   nothing about the upgrade, so it is reported and never counted as a
//!   regression.
//!
//! A rehearsal that compared no transaction at all, because every one was
//! skipped or there was no traffic, verified nothing. It has its own exit code
//! so it can never pass for a clean upgrade.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use super::catalog::ProgramCatalog;
use super::catalog::compare_raw;

/// Version of the `--json` document. Bump it on any breaking change.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

/// Log lines kept from the end of each run for an outcome change.
const LOG_EXCERPT_LINES: usize = 12;

/// The complete result of one rehearsal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RehearsalReport {
	/// [`REPORT_SCHEMA_VERSION`].
	pub schema_version: u32,
	/// Named cluster, or the origin of a custom RPC URL.
	pub cluster: String,
	/// The rehearsed program.
	pub program_id: String,
	/// SHA-256 of the deployed executable, trailing zero padding removed.
	pub deployed_sha256: String,
	/// SHA-256 of the candidate executable, trailing zero padding removed.
	pub candidate_sha256: String,
	/// Absolute slot of the forked state both runs executed against.
	pub slot: u64,
	/// Transaction counts by status.
	pub summary: RehearsalSummary,
	/// Compute units per instruction, over transactions that succeeded in both
	/// runs.
	pub instructions: Vec<InstructionUnits>,
	/// Every requested transaction, in the order the cluster returned them
	/// (newest first) or the order `--signature` named them.
	pub transactions: Vec<TransactionRehearsal>,
}

/// Transaction counts by status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RehearsalSummary {
	pub total: usize,
	pub unchanged: usize,
	pub cu_changed: usize,
	pub state_changed: usize,
	pub outcome_changed: usize,
	pub skipped: usize,
}

/// Compute-unit statistics for one instruction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionUnits {
	/// Instruction name from the project's IR.
	pub name: String,
	/// Number of invocations measured in both runs.
	pub samples: usize,
	pub baseline: UnitStats,
	pub candidate: UnitStats,
	/// Candidate median minus baseline median.
	pub median_delta: i64,
}

/// Minimum, median, and maximum compute units. The median of an even sample
/// is the lower middle value, so every statistic is a measured value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitStats {
	pub min: u64,
	pub median: u64,
	pub max: u64,
}

/// How one transaction compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RehearsalStatus {
	Unchanged,
	CuChanged,
	StateChanged,
	OutcomeChanged,
	Skipped,
}

impl RehearsalStatus {
	/// The status spelling shared by the text report and the JSON document.
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Unchanged => "unchanged",
			Self::CuChanged => "cu_changed",
			Self::StateChanged => "state_changed",
			Self::OutcomeChanged => "outcome_changed",
			Self::Skipped => "skipped",
		}
	}
}

/// Why a transaction was not compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
	/// The RPC no longer returns the transaction.
	Unavailable,
	/// The transaction could not be decoded or fetched in a usable encoding.
	Undecodable,
	/// Surfpool refused to profile the transaction.
	NotProfiled,
	/// Both runs failed with the same error.
	FailedInBoth,
}

impl SkipReason {
	const fn as_str(self) -> &'static str {
		match self {
			Self::Unavailable => "unavailable",
			Self::Undecodable => "undecodable",
			Self::NotProfiled => "not_profiled",
			Self::FailedInBoth => "failed_in_both",
		}
	}
}

/// A skipped transaction's reason and the message that explains it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTransaction {
	pub reason: SkipReason,
	pub detail: String,
}

/// One transaction's comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRehearsal {
	pub signature: String,
	pub status: RehearsalStatus,
	/// Present only for `skipped`.
	pub skip: Option<SkippedTransaction>,
	/// The program's own top-level instructions, in execution order.
	pub instructions: Vec<InstructionRehearsal>,
	/// Absent when the transaction was skipped before profiling.
	pub baseline: Option<RunOutcome>,
	pub candidate: Option<RunOutcome>,
	/// Writable accounts whose final state differs. Computed only when both
	/// runs succeed, because a failed run commits no state.
	pub accounts: Vec<AccountChange>,
	/// The end of each run's logs, present only for `outcome_changed`.
	pub logs: Option<LogExcerpt>,
}

/// One instruction of the program and its compute units in each run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionRehearsal {
	pub name: String,
	pub baseline_units: Option<u64>,
	pub candidate_units: Option<u64>,
}

/// The result of one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOutcome {
	/// `null` when the transaction succeeded.
	pub error: Option<String>,
	pub compute_units: u64,
}

/// A writable account left different by the two runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountChange {
	pub address: String,
	/// Account type matched by discriminator, for accounts the program owns.
	pub account_type: Option<String>,
	/// `null` when the account does not exist after the run.
	pub baseline: Option<AccountSummary>,
	pub candidate: Option<AccountSummary>,
	/// Decoded fields that differ, baseline value then candidate value.
	pub fields: Vec<FieldChange>,
	/// Differing bytes no decoded field covers.
	pub byte_ranges: Vec<ByteRange>,
	/// Differing ranges beyond those listed.
	pub omitted_byte_ranges: usize,
}

/// The non-data facts of an account image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
	pub lamports: u64,
	pub owner: String,
	pub data_len: usize,
}

/// One decoded field that differs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
	pub name: String,
	pub baseline: String,
	pub candidate: String,
}

/// A half-open range of differing account bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteRange {
	pub start: usize,
	pub end: usize,
}

/// The final log lines of each run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogExcerpt {
	pub baseline: Vec<String>,
	pub candidate: Vec<String>,
}

/// One profiled run as Surfpool reported it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Execution {
	pub(crate) error: Option<String>,
	pub(crate) compute_units: u64,
	pub(crate) logs: Vec<String>,
	/// Compute units of each top-level instruction, empty when Surfpool
	/// omitted instruction profiles.
	pub(crate) instruction_units: Vec<u64>,
	/// Final state of every writable account; `None` means it does not exist.
	pub(crate) accounts: BTreeMap<String, Option<AccountImage>>,
}

/// An account as a profile captured it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AccountImage {
	pub(crate) lamports: u64,
	pub(crate) owner: String,
	pub(crate) executable: bool,
	pub(crate) rent_epoch: u64,
	pub(crate) data: Vec<u8>,
}

impl AccountImage {
	/// Whether two images hold the same state. Rent epoch is bookkeeping, not
	/// state a program writes.
	fn same_state(&self, other: &Self) -> bool {
		self.lamports == other.lamports
			&& self.owner == other.owner
			&& self.executable == other.executable
			&& self.data == other.data
	}

	fn summary(&self) -> AccountSummary {
		AccountSummary {
			lamports: self.lamports,
			owner: self.owner.clone(),
			data_len: self.data.len(),
		}
	}
}

/// A program instruction's position in its transaction and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProgramInstruction {
	pub(crate) position: usize,
	pub(crate) name: String,
}

/// Facts about the rehearsal that head the report.
pub(crate) struct ReportHeader {
	pub(crate) cluster: String,
	pub(crate) program_id: String,
	pub(crate) deployed_sha256: String,
	pub(crate) candidate_sha256: String,
	pub(crate) slot: u64,
}

impl TransactionRehearsal {
	/// A transaction that was not profiled at all.
	pub(crate) fn skipped(
		signature: String,
		instructions: &[ProgramInstruction],
		reason: SkipReason,
		detail: String,
	) -> Self {
		Self {
			signature,
			status: RehearsalStatus::Skipped,
			skip: Some(SkippedTransaction { reason, detail }),
			instructions: instructions
				.iter()
				.map(|instruction| {
					InstructionRehearsal {
						name: instruction.name.clone(),
						baseline_units: None,
						candidate_units: None,
					}
				})
				.collect(),
			baseline: None,
			candidate: None,
			accounts: Vec::new(),
			logs: None,
		}
	}

	/// Classify two runs of the same transaction.
	pub(crate) fn compare(
		signature: String,
		instructions: &[ProgramInstruction],
		baseline: &Execution,
		candidate: &Execution,
		catalog: &ProgramCatalog,
		program_id: &str,
	) -> Self {
		let mut rehearsal = Self {
			signature,
			status: RehearsalStatus::Unchanged,
			skip: None,
			instructions: instructions
				.iter()
				.map(|instruction| {
					InstructionRehearsal {
						name: instruction.name.clone(),
						baseline_units: baseline
							.instruction_units
							.get(instruction.position)
							.copied(),
						candidate_units: candidate
							.instruction_units
							.get(instruction.position)
							.copied(),
					}
				})
				.collect(),
			baseline: Some(RunOutcome {
				error: baseline.error.clone(),
				compute_units: baseline.compute_units,
			}),
			candidate: Some(RunOutcome {
				error: candidate.error.clone(),
				compute_units: candidate.compute_units,
			}),
			accounts: Vec::new(),
			logs: None,
		};

		match (&baseline.error, &candidate.error) {
			(Some(before), Some(after)) if before == after => {
				rehearsal.status = RehearsalStatus::Skipped;
				rehearsal.skip = Some(SkippedTransaction {
					reason: SkipReason::FailedInBoth,
					detail: before.clone(),
				});
			}
			(None, None) => {
				rehearsal.accounts = account_changes(baseline, candidate, catalog, program_id);
				let units_changed = baseline.compute_units != candidate.compute_units
					|| baseline.instruction_units != candidate.instruction_units;
				rehearsal.status = if !rehearsal.accounts.is_empty() {
					RehearsalStatus::StateChanged
				} else if units_changed {
					RehearsalStatus::CuChanged
				} else {
					RehearsalStatus::Unchanged
				};
			}
			_ => {
				rehearsal.status = RehearsalStatus::OutcomeChanged;
				rehearsal.logs = Some(LogExcerpt {
					baseline: log_excerpt(&baseline.logs),
					candidate: log_excerpt(&candidate.logs),
				});
			}
		}

		rehearsal
	}
}

fn log_excerpt(logs: &[String]) -> Vec<String> {
	logs[logs.len().saturating_sub(LOG_EXCERPT_LINES)..].to_vec()
}

fn account_changes(
	baseline: &Execution,
	candidate: &Execution,
	catalog: &ProgramCatalog,
	program_id: &str,
) -> Vec<AccountChange> {
	let addresses = baseline
		.accounts
		.keys()
		.chain(candidate.accounts.keys())
		.collect::<std::collections::BTreeSet<_>>();
	let mut changes = Vec::new();

	for address in addresses {
		let before = baseline.accounts.get(address).cloned().flatten();
		let after = candidate.accounts.get(address).cloned().flatten();

		if let (Some(before), Some(after)) = (&before, &after)
			&& before.same_state(after)
		{
			continue;
		}

		if before.is_none() && after.is_none() {
			continue;
		}

		changes.push(account_change(
			address,
			before.as_ref(),
			after.as_ref(),
			catalog,
			program_id,
		));
	}

	changes
}

fn account_change(
	address: &str,
	before: Option<&AccountImage>,
	after: Option<&AccountImage>,
	catalog: &ProgramCatalog,
	program_id: &str,
) -> AccountChange {
	let before_data = before.map_or(&[][..], |image| image.data.as_slice());
	let after_data = after.map_or(&[][..], |image| image.data.as_slice());
	// Discriminators only identify accounts the program owns; another owner's
	// first byte can collide with any of them.
	let layout = [before, after]
		.into_iter()
		.flatten()
		.find(|image| image.owner == program_id)
		.and_then(|image| catalog.account_layout(&image.data));
	let comparison = match layout {
		Some(layout) => layout.compare(before_data, after_data),
		None => compare_raw(before_data, after_data),
	};

	AccountChange {
		address: address.to_owned(),
		account_type: layout.map(|layout| layout.name().to_owned()),
		baseline: before.map(AccountImage::summary),
		candidate: after.map(AccountImage::summary),
		fields: comparison.fields,
		byte_ranges: comparison.ranges,
		omitted_byte_ranges: comparison.omitted_ranges,
	}
}

impl RehearsalReport {
	/// Assemble the report from compared transactions.
	pub(crate) fn new(header: ReportHeader, transactions: Vec<TransactionRehearsal>) -> Self {
		let mut summary = RehearsalSummary {
			total: transactions.len(),
			..RehearsalSummary::default()
		};

		for transaction in &transactions {
			let counter = match transaction.status {
				RehearsalStatus::Unchanged => &mut summary.unchanged,
				RehearsalStatus::CuChanged => &mut summary.cu_changed,
				RehearsalStatus::StateChanged => &mut summary.state_changed,
				RehearsalStatus::OutcomeChanged => &mut summary.outcome_changed,
				RehearsalStatus::Skipped => &mut summary.skipped,
			};
			*counter += 1;
		}

		Self {
			schema_version: REPORT_SCHEMA_VERSION,
			cluster: header.cluster,
			program_id: header.program_id,
			deployed_sha256: header.deployed_sha256,
			candidate_sha256: header.candidate_sha256,
			slot: header.slot,
			summary,
			instructions: instruction_units(&transactions),
			transactions,
		}
	}

	/// Whether any transaction's outcome or written state changed.
	pub fn behaviour_changed(&self) -> bool {
		self.summary.state_changed + self.summary.outcome_changed > 0
	}

	/// How many transactions ran in both binaries and were compared.
	pub fn compared(&self) -> usize {
		self.summary.total - self.summary.skipped
	}

	/// The process exit code:
	///
	/// - 2 when behaviour changed, unless `allow_changes` accepts it (then 0);
	/// - 3 when no transaction was compared, so nothing was verified;
	/// - 0 otherwise. Compute-unit changes alone never fail a rehearsal.
	pub fn exit_code(&self, allow_changes: bool) -> i32 {
		if self.behaviour_changed() {
			return if allow_changes { 0 } else { 2 };
		}

		if self.compared() == 0 { 3 } else { 0 }
	}

	/// Render the human-readable report, as `pina rehearse` prints it.
	pub fn render_text(&self) -> String {
		self.render("--allow-changes", "--limit or --signature")
	}

	/// Render the human-readable report as `pina deploy --rehearse` prints it:
	/// its verdict names the deploy flags that accept changes or replay more
	/// traffic.
	pub fn render_deploy_text(&self) -> String {
		self.render("--allow-rehearsal-changes", "--rehearse-limit")
	}

	fn render(&self, accept_flag: &str, traffic_flags: &str) -> String {
		let mut output = String::new();
		let summary = &self.summary;

		let _ = writeln!(
			output,
			"Rehearsal of {} on {}",
			self.program_id, self.cluster
		);
		let _ = writeln!(output, "  deployed   sha256 {}", self.deployed_sha256);
		let _ = writeln!(output, "  candidate  sha256 {}", self.candidate_sha256);
		let _ = writeln!(output, "  fork slot  {}", self.slot);
		let _ = writeln!(output);
		let _ = writeln!(
			output,
			"{} transaction{}: {} unchanged, {} cu_changed, {} state_changed, {} outcome_changed, \
			 {} skipped",
			summary.total,
			if summary.total == 1 { "" } else { "s" },
			summary.unchanged,
			summary.cu_changed,
			summary.state_changed,
			summary.outcome_changed,
			summary.skipped
		);

		if summary.total == 0 {
			let _ = writeln!(
				output,
				"No transactions were rehearsed; the program has no recent traffic to replay."
			);
		}

		self.render_units(&mut output);
		self.render_changes(&mut output);
		self.render_skips(&mut output);

		let _ = writeln!(output);
		if self.behaviour_changed() {
			let _ = writeln!(
				output,
				"Behaviour changed in {} transaction(s). Review the differences, or rerun with \
				 {accept_flag} to accept them.",
				summary.state_changed + summary.outcome_changed
			);
		} else if self.compared() == 0 {
			let _ = writeln!(
				output,
				"No transaction was compared, so this rehearsal verified nothing about the \
				 upgrade. Replay more or newer traffic with {traffic_flags}."
			);
		} else {
			let _ = writeln!(
				output,
				"No behaviour changes in {} compared transaction(s). Compute-unit changes are \
				 informational.",
				self.compared()
			);
		}

		output
	}

	fn render_units(&self, output: &mut String) {
		if self.instructions.is_empty() {
			return;
		}

		let mut table = comfy_table::Table::new();
		table.load_style(comfy_table::presets::UTF8_FULL_CONDENSED);
		table.set_header(vec![
			"Instruction",
			"Samples",
			"Baseline min/median/max",
			"Candidate min/median/max",
			"Median delta",
		]);

		for row in &self.instructions {
			table.add_row(vec![
				row.name.clone(),
				row.samples.to_string(),
				stats(row.baseline),
				stats(row.candidate),
				signed(row.median_delta),
			]);
		}

		let _ = writeln!(output);
		let _ = writeln!(
			output,
			"Compute units (transactions that succeeded in both runs):"
		);
		let _ = writeln!(output, "{table}");
	}

	fn render_changes(&self, output: &mut String) {
		let changed = self.transactions.iter().filter(|transaction| {
			!matches!(
				transaction.status,
				RehearsalStatus::Unchanged | RehearsalStatus::Skipped
			)
		});

		for (index, transaction) in changed.enumerate() {
			if index == 0 {
				let _ = writeln!(output);
				let _ = writeln!(output, "Changed transactions:");
			}

			render_transaction(output, transaction);
		}
	}

	fn render_skips(&self, output: &mut String) {
		let skipped = self
			.transactions
			.iter()
			.filter_map(|transaction| transaction.skip.as_ref().map(|skip| (transaction, skip)));

		for (index, (transaction, skip)) in skipped.enumerate() {
			if index == 0 {
				let _ = writeln!(output);
				let _ = writeln!(output, "Skipped transactions:");
			}

			let _ = writeln!(
				output,
				"  {}  {}: {}",
				transaction.signature,
				skip.reason.as_str(),
				skip.detail
			);
		}
	}
}

fn render_transaction(output: &mut String, transaction: &TransactionRehearsal) {
	let names = transaction
		.instructions
		.iter()
		.map(|instruction| instruction.name.as_str())
		.collect::<Vec<_>>()
		.join(", ");
	let _ = writeln!(
		output,
		"  {}  {}  [{}]",
		transaction.signature,
		transaction.status.as_str(),
		names
	);

	if let (Some(baseline), Some(candidate)) = (&transaction.baseline, &transaction.candidate) {
		if baseline.error != candidate.error {
			let _ = writeln!(output, "    baseline   {}", outcome(baseline));
			let _ = writeln!(output, "    candidate  {}", outcome(candidate));
		}

		let _ = writeln!(
			output,
			"    compute units {} -> {} ({})",
			baseline.compute_units,
			candidate.compute_units,
			signed(candidate.compute_units as i64 - baseline.compute_units as i64)
		);
	}

	for account in &transaction.accounts {
		render_account(output, account);
	}

	if let Some(logs) = &transaction.logs {
		for (label, lines) in [("baseline", &logs.baseline), ("candidate", &logs.candidate)] {
			let _ = writeln!(output, "    {label} logs:");

			for line in lines {
				let _ = writeln!(output, "      {line}");
			}
		}
	}
}

fn render_account(output: &mut String, account: &AccountChange) {
	let account_type = account
		.account_type
		.as_deref()
		.map_or_else(String::new, |name| format!(" ({name})"));
	let _ = writeln!(output, "    account {}{account_type}", account.address);

	match (&account.baseline, &account.candidate) {
		(Some(before), Some(after)) => {
			if before.lamports != after.lamports {
				let _ = writeln!(
					output,
					"      lamports: {} -> {}",
					before.lamports, after.lamports
				);
			}

			if before.owner != after.owner {
				let _ = writeln!(output, "      owner: {} -> {}", before.owner, after.owner);
			}

			if before.data_len != after.data_len {
				let _ = writeln!(
					output,
					"      data length: {} -> {}",
					before.data_len, after.data_len
				);
			}
		}
		(before, after) => {
			let _ = writeln!(
				output,
				"      exists: {} -> {}",
				before.is_some(),
				after.is_some()
			);
		}
	}

	for field in &account.fields {
		let _ = writeln!(
			output,
			"      {}: {} -> {}",
			field.name, field.baseline, field.candidate
		);
	}

	if !account.byte_ranges.is_empty() {
		let ranges = account
			.byte_ranges
			.iter()
			.map(|range| format!("{}..{}", range.start, range.end))
			.collect::<Vec<_>>()
			.join(", ");
		let more = if account.omitted_byte_ranges == 0 {
			String::new()
		} else {
			format!(" (+{} more)", account.omitted_byte_ranges)
		};
		let _ = writeln!(output, "      bytes differ: {ranges}{more}");
	}
}

fn outcome(run: &RunOutcome) -> String {
	run.error.as_ref().map_or_else(
		|| "succeeded".to_owned(),
		|error| format!("failed: {error}"),
	)
}

fn stats(stats: UnitStats) -> String {
	format!("{}/{}/{}", stats.min, stats.median, stats.max)
}

fn signed(value: i64) -> String {
	if value > 0 {
		format!("+{value}")
	} else {
		value.to_string()
	}
}

/// Aggregate per-instruction compute units over transactions that succeeded in
/// both runs; a failed run's units measure an aborted path.
fn instruction_units(transactions: &[TransactionRehearsal]) -> Vec<InstructionUnits> {
	let mut samples: BTreeMap<&str, (Vec<u64>, Vec<u64>)> = BTreeMap::new();
	let compared = transactions.iter().filter(|transaction| {
		matches!(
			transaction.status,
			RehearsalStatus::Unchanged | RehearsalStatus::CuChanged | RehearsalStatus::StateChanged
		)
	});

	for transaction in compared {
		for instruction in &transaction.instructions {
			let (Some(baseline), Some(candidate)) =
				(instruction.baseline_units, instruction.candidate_units)
			else {
				continue;
			};
			let entry = samples.entry(instruction.name.as_str()).or_default();
			entry.0.push(baseline);
			entry.1.push(candidate);
		}
	}

	samples
		.into_iter()
		.map(|(name, (mut baseline, mut candidate))| {
			let count = baseline.len();
			let baseline = unit_stats(&mut baseline);
			let candidate = unit_stats(&mut candidate);

			InstructionUnits {
				name: name.to_owned(),
				samples: count,
				median_delta: candidate.median as i64 - baseline.median as i64,
				baseline,
				candidate,
			}
		})
		.collect()
}

/// Sort `values` and summarize them. Callers only pass non-empty samples.
fn unit_stats(values: &mut [u64]) -> UnitStats {
	values.sort_unstable();

	UnitStats {
		min: values[0],
		median: values[(values.len() - 1) / 2],
		max: values[values.len() - 1],
	}
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
