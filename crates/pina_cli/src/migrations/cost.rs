//! Pre-deploy cost preview for checked-in migration histories.
//!
//! `pina migrations status` derives this preview from the manifest and the
//! static estimates `pina profile` reads out of the compiled SBF artifact. It
//! reports three levels: per account contract, per instruction process, and
//! one program-wide "most expensive touching transaction" summary so a
//! developer can size `max_lamports` and `MAX_INLINE_STEPS` deliberately.
//!
//! Every figure is a planning estimate. Rent uses the same
//! `RENT_EXEMPT_LAMPORTS_PER_BYTE` convention as the `make` growth warning,
//! compute units reuse `pina_profile`'s static estimate instead of deriving a
//! second cost model, and a figure that cannot be estimated says so instead of
//! printing a misleading zero.

use std::collections::BTreeMap;
use std::path::PathBuf;

use heck::ToSnakeCase as _;
use pina_abi::ContractHistory;
use pina_abi::ContractKind;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_profile::ProgramProfile;
use serde::Serialize;

use super::inspect::pending_hops;
use super::remedy::ACCOUNT_GROWTH_REMEDY;
use super::remedy::LAMPORT_BUDGET_REMEDY;
use super::transition::MAX_PERMITTED_DATA_INCREASE;
use super::transition::RENT_EXEMPT_LAMPORTS_PER_BYTE;

/// Framework cap on adjacent transitions one instruction may execute.
///
/// The migration macros generate `MAX_INLINE_STEPS = current.min(8)` for every
/// contract, so this mirrors the framework maximum without reading generated
/// code. An account further behind fails with `MigrationUnavailable` instead
/// of silently burning a longer ladder's compute budget.
pub(crate) const MAX_INLINE_STEPS: u32 = 8;

/// Human-readable description of the ladder a preview quotes.
pub(crate) const LADDER_MODEL: &str =
	"the oldest version within MAX_INLINE_STEPS (8) of the current version, climbing one adjacent \
	 transition per step; a version-0 (day-one) account is only reachable inline when the history \
	 has at most MAX_INLINE_STEPS transitions";

/// Human-readable description of the static CU estimate.
pub(crate) const CU_MODEL: &str = "sum of `pina profile` static estimates for the generated \
                                   adjacent `migrate` functions on the ladder; excludes executor \
                                   overhead (resize, rent transfer, validation) and runtime \
                                   branch or loop effects";

/// Program-wide cost picture for the checked-in migration history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationCostPreview {
	/// Rent convention shared with the `make` growth warning.
	pub rent_lamports_per_byte: u64,
	/// Framework cap on adjacent transitions per instruction.
	pub max_inline_steps: u32,
	/// Which ladder the per-instruction figures assume.
	pub ladder_model: String,
	/// What the static CU number sums.
	pub cu_model: String,
	/// Compiled SBF artifact the CU estimates were read from, when one was found.
	pub artifact: Option<PathBuf>,
	/// Remedy quoted when a ladder's rent must be funded, shared with `make`.
	pub lamport_budget_remedy: String,
	/// Remedy quoted when one step's growth exceeds the runtime realloc cap.
	pub account_growth_remedy: String,
	/// Per account contract, ordered by manifest identity.
	pub contracts: Vec<AccountCostPreview>,
	/// Per instruction process that names a migration-aware account, ordered by manifest identity.
	pub instructions: Vec<InstructionCostPreview>,
	/// The touching instruction whose worst-case ladders cost the most.
	pub most_expensive: MostExpensiveTransaction,
}

/// Current and day-one cost figures for one account contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountCostPreview {
	/// Manifest contract key.
	pub identity: String,
	/// Current Rust source name.
	pub rust_name: String,
	/// Current manifest version.
	pub current_version: u32,
	/// Adjacent transitions in the history.
	pub transition_count: u32,
	/// Header plus current payload size; compact payloads quote their capacity.
	pub current_size_bytes: usize,
	/// Bytes a version-0 account grows to reach the current version.
	pub day_one_growth_bytes: usize,
	/// Approximate rent the payer funds for that growth.
	pub day_one_rent_deficit_lamports: u64,
	/// The worst supported ladder, absent when the account never migrates.
	pub worst_case_ladder: Option<LadderCost>,
	/// Planning caveats a reader must see, such as an out-of-reach day-one account.
	pub notes: Vec<String>,
}

/// One worst-case ladder estimate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LadderCost {
	/// Version the stale account starts at.
	pub from_version: u32,
	/// Version the ladder lands on.
	pub to_version: u32,
	/// Adjacent transitions in the ladder.
	pub steps: u32,
	/// Whether the ladder starts at version zero.
	pub day_one: bool,
	/// Approximate rent the payer funds for this ladder.
	pub rent_deficit_lamports: u64,
	/// Static CU estimate, or why none exists.
	pub static_cu: StaticCuEstimate,
}

/// One account ladder reached from an instruction process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionLadder {
	/// Manifest contract key of the account.
	pub account_identity: String,
	/// Current Rust source name of the account.
	pub account_rust_name: String,
	/// Ladder estimate for the account.
	#[serde(flatten)]
	pub ladder: LadderCost,
}

/// Worst-case cost of one instruction process touching stale accounts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionCostPreview {
	/// Manifest contract key.
	pub identity: String,
	/// Current Rust source name.
	pub rust_name: String,
	/// Current manifest version.
	pub current_version: u32,
	/// Per-account ladders this process can trigger.
	pub ladders: Vec<InstructionLadder>,
	/// Adjacent transitions across every ladder.
	pub total_steps: u32,
	/// Approximate rent the payer funds across every ladder.
	pub total_rent_deficit_lamports: u64,
	/// Static CU estimate summed across every ladder, or why none exists.
	pub static_cu: StaticCuEstimate,
	/// Migration-shaped slots (`writable`, not a signer) that name no checked-in
	/// account contract, so a rename or typo cannot silently drop a ladder.
	pub notes: Vec<String>,
}

/// A static CU estimate, or the explicit reason none exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
	tag = "status",
	rename_all = "camelCase",
	rename_all_fields = "camelCase"
)]
pub enum StaticCuEstimate {
	/// `pina profile` produced an estimate for every function the figure sums.
	Estimated {
		/// Estimated compute units.
		estimated_cu: u64,
		/// Which functions the number sums.
		model: String,
	},
	/// No estimate exists; the reason is printed instead of a zero.
	Unavailable {
		/// Why the estimate could not be produced.
		reason: String,
	},
}

/// The touching instruction with the largest worst-case ladder cost.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
	tag = "status",
	rename_all = "camelCase",
	rename_all_fields = "camelCase"
)]
pub enum MostExpensiveTransaction {
	/// One instruction process names at least one migration-aware account.
	///
	/// The rent maximum and the step maximum are reported independently: the
	/// instruction funding the most rent need not be the one with the longest
	/// ladder, and `MAX_INLINE_STEPS` must cover the latter.
	Identified {
		/// Manifest contract key of the instruction funding the most rent.
		instruction_identity: String,
		/// Current Rust source name of the instruction funding the most rent.
		instruction_rust_name: String,
		/// Account ladders the process can trigger.
		account_ladders: u32,
		/// Adjacent transitions across every ladder.
		steps: u32,
		/// Approximate rent the payer funds across every ladder.
		rent_deficit_lamports: u64,
		/// Static CU estimate summed across every ladder, or why none exists.
		static_cu: StaticCuEstimate,
		/// Manifest contract key of the instruction with the longest ladder.
		max_steps_instruction_identity: String,
		/// Current Rust source name of the instruction with the longest ladder.
		max_steps_instruction_rust_name: String,
		/// Adjacent transitions in that longest worst-case ladder set.
		max_steps: u32,
		/// Which ladder the figures assume.
		model: String,
	},
	/// No instruction process names a migration-aware account.
	Unavailable {
		/// Why no touching transaction could be identified.
		reason: String,
	},
}

/// The compiled artifact selected for static estimates, or why none was used.
pub(crate) struct ProfileSource {
	/// Parsed profile, absent when discovery or parsing failed.
	pub(crate) profile: Option<ProgramProfile>,
	/// Artifact path the estimates were read from, when one was selected.
	pub(crate) artifact: Option<PathBuf>,
	/// Why no profile is present; empty when `profile` is `Some`.
	pub(crate) reason: String,
}

/// Resolve and profile the project's compiled SBF artifact when the manifest
/// contains at least one account contract.
pub(crate) fn load_profile_source(
	project: &crate::project::Project,
	manifest: Option<&MigrationManifest>,
) -> ProfileSource {
	let has_accounts = manifest.is_some_and(|manifest| {
		manifest
			.contracts
			.values()
			.any(|history| history.identity.kind == ContractKind::Account)
	});
	if !has_accounts {
		return ProfileSource {
			profile: None,
			artifact: None,
			reason: "the program has no migration-aware account contract to estimate".to_owned(),
		};
	}

	let artifact = project.sbf_artifact();
	if !artifact.is_file() {
		let reason = format!(
			"compiled SBF artifact not found at {}; build the program to include static CU \
			 estimates",
			artifact.display()
		);
		return ProfileSource {
			profile: None,
			artifact: Some(artifact),
			reason,
		};
	}

	match pina_profile::profile_program(&artifact) {
		Ok(profile) => {
			ProfileSource {
				profile: Some(profile),
				artifact: Some(artifact),
				reason: String::new(),
			}
		}
		Err(error) => {
			let reason = format!("could not profile {}: {error}", artifact.display());
			ProfileSource {
				profile: None,
				artifact: Some(artifact),
				reason,
			}
		}
	}
}

/// Build the preview from a manifest and, when available, a static profile.
pub(crate) fn build_cost_preview(
	manifest: Option<&MigrationManifest>,
	source: &ProfileSource,
) -> MigrationCostPreview {
	let mut preview = MigrationCostPreview {
		rent_lamports_per_byte: RENT_EXEMPT_LAMPORTS_PER_BYTE,
		max_inline_steps: MAX_INLINE_STEPS,
		ladder_model: LADDER_MODEL.to_owned(),
		cu_model: CU_MODEL.to_owned(),
		artifact: source.artifact.clone(),
		lamport_budget_remedy: LAMPORT_BUDGET_REMEDY.to_owned(),
		account_growth_remedy: ACCOUNT_GROWTH_REMEDY.to_owned(),
		contracts: Vec::new(),
		instructions: Vec::new(),
		most_expensive: MostExpensiveTransaction::Unavailable {
			reason: NO_TOUCHING_INSTRUCTION.to_owned(),
		},
	};
	let Some(manifest) = manifest else {
		return preview;
	};

	for history in manifest.contracts.values() {
		if history.identity.kind != ContractKind::Account {
			continue;
		}
		preview
			.contracts
			.push(account_cost_preview(history, manifest.version_type, source));
	}

	let accounts_by_slot = preview
		.contracts
		.iter()
		.map(|contract| (contract.rust_name.to_snake_case(), contract))
		.collect::<BTreeMap<_, _>>();
	for history in manifest.contracts.values() {
		if history.identity.kind != ContractKind::Instruction {
			continue;
		}

		// Only a `writable`, non-signer slot can hold an account the executor
		// migrates, so only those join against account contracts; every other
		// slot (authorities, payers, programs) is dropped soundly. An unlinked
		// candidate is recorded as a note instead of silently costing nothing.
		let mut candidates = BTreeMap::new();
		if let Some(process) = history
			.current()
			.and_then(|current| current.process.as_ref())
		{
			candidates.extend(
				process
					.accounts
					.iter()
					.filter(|account| account.writable && !account.signer)
					.map(|account| (account.name.to_snake_case(), account.name.clone())),
			);
		}
		let mut notes = Vec::new();
		let mut ladders = Vec::new();
		for (slot, name) in candidates {
			let Some(contract) = accounts_by_slot.get(&slot) else {
				notes.push(format!(
					"cannot link writable slot `{name}` to a checked-in account contract, so its \
					 ladder cost is not included"
				));
				continue;
			};
			let Some(ladder) = &contract.worst_case_ladder else {
				continue;
			};
			ladders.push(InstructionLadder {
				account_identity: contract.identity.clone(),
				account_rust_name: contract.rust_name.clone(),
				ladder: ladder.clone(),
			});
		}
		preview
			.instructions
			.push(instruction_cost_preview(history, ladders, notes));
	}

	preview.most_expensive = most_expensive_transaction(&preview.instructions);
	preview
}

/// Reason printed when no instruction process names a migration-aware account.
const NO_TOUCHING_INSTRUCTION: &str =
	"no migration-aware instruction process names a migration-aware account contract";

/// Build one account's current and day-one cost figures.
fn account_cost_preview(
	history: &ContractHistory,
	version_type: MigrationVersionType,
	source: &ProfileSource,
) -> AccountCostPreview {
	let header = usize::from(history.identity.discriminator_bytes) + version_type.bytes();
	let current_version = history.current().map_or(0, |current| current.version);
	let transition_count =
		u32::try_from(history.versions.len().saturating_sub(1)).unwrap_or(u32::MAX);
	let payload_size = |version: Option<&pina_abi::SchemaVersion>| {
		version
			.and_then(|version| version.schema.maximum_payload_size())
			.map_or(header, |payload| header.saturating_add(payload))
	};
	let current_size_bytes = payload_size(history.current());
	let day_one_size_bytes = payload_size(history.versions.first());
	let day_one_growth_bytes = current_size_bytes.saturating_sub(day_one_size_bytes);
	let hops = pending_hops(
		history,
		0,
		usize::from(history.identity.discriminator_bytes),
		version_type.bytes(),
	);
	let day_one_rent_deficit_lamports = hops
		.iter()
		.map(|hop| hop.rent_delta_lamports)
		.fold(0_u64, u64::saturating_add);

	let mut notes = Vec::new();
	if hops.iter().any(|hop| {
		hop.byte_size_to.saturating_sub(hop.byte_size_from) > MAX_PERMITTED_DATA_INCREASE
	}) {
		notes.push(format!(
			"one step grows the account by more than `MAX_PERMITTED_DATA_INCREASE` \
			 ({MAX_PERMITTED_DATA_INCREASE} bytes), so {ACCOUNT_GROWTH_REMEDY}"
		));
	}

	let worst_case_ladder = (current_version > 0).then(|| {
		let worst_from = current_version.saturating_sub(MAX_INLINE_STEPS.min(current_version));
		if worst_from > 0 {
			notes.push(format!(
				"history has {transition_count} adjacent transitions, so a version-0 account is \
				 more than MAX_INLINE_STEPS ({MAX_INLINE_STEPS}) behind and fails with \
				 `MigrationUnavailable`; the quoted ladder starts at v{worst_from}"
			));
		}
		let hops = pending_hops(
			history,
			worst_from,
			usize::from(history.identity.discriminator_bytes),
			version_type.bytes(),
		);
		let rent_deficit_lamports = hops
			.iter()
			.map(|hop| hop.rent_delta_lamports)
			.fold(0_u64, u64::saturating_add);
		LadderCost {
			from_version: worst_from,
			to_version: current_version,
			steps: current_version - worst_from,
			day_one: worst_from == 0,
			rent_deficit_lamports,
			static_cu: estimate_ladder_cu(source, history, worst_from),
		}
	});

	AccountCostPreview {
		identity: history.identity.key(),
		rust_name: history.rust_name.clone(),
		current_version,
		transition_count,
		current_size_bytes,
		day_one_growth_bytes,
		day_one_rent_deficit_lamports,
		worst_case_ladder,
		notes,
	}
}

/// Sum one instruction process's ladders into a worst-case touching cost.
fn instruction_cost_preview(
	history: &ContractHistory,
	ladders: Vec<InstructionLadder>,
	notes: Vec<String>,
) -> InstructionCostPreview {
	let total_steps = ladders.iter().map(|ladder| ladder.ladder.steps).sum();
	let total_rent_deficit_lamports = ladders
		.iter()
		.map(|ladder| ladder.ladder.rent_deficit_lamports)
		.fold(0_u64, u64::saturating_add);

	InstructionCostPreview {
		identity: history.identity.key(),
		rust_name: history.rust_name.clone(),
		current_version: history.current().map_or(0, |current| current.version),
		static_cu: total_static_cu(&ladders),
		ladders,
		total_steps,
		total_rent_deficit_lamports,
		notes,
	}
}

/// Sum ladder estimates, or explain every ladder that has none.
fn total_static_cu(ladders: &[InstructionLadder]) -> StaticCuEstimate {
	if ladders.is_empty() {
		return StaticCuEstimate::Unavailable {
			reason: "no checked-in account contract matches this instruction's process account \
			         slots by name"
				.to_owned(),
		};
	}

	let mut estimated_cu = 0_u64;
	let mut reasons = Vec::new();
	for ladder in ladders {
		match &ladder.ladder.static_cu {
			StaticCuEstimate::Estimated {
				estimated_cu: cu, ..
			} => {
				estimated_cu = estimated_cu.saturating_add(*cu);
			}
			StaticCuEstimate::Unavailable { reason } => {
				// Several ladders usually share one reason (a missing artifact,
				// say); quote it once instead of once per ladder.
				if !reasons.contains(reason) {
					reasons.push(reason.clone());
				}
			}
		}
	}
	if reasons.is_empty() {
		StaticCuEstimate::Estimated {
			estimated_cu,
			model: CU_MODEL.to_owned(),
		}
	} else {
		StaticCuEstimate::Unavailable {
			reason: reasons.join("; "),
		}
	}
}

/// Pick the touching transaction figures that size the on-chain budgets.
///
/// Rent and steps are maximized independently: the instruction funding the
/// most rent need not run the longest ladder, and sizing `max_lamports` from
/// one while sizing `MAX_INLINE_STEPS` from the other must not understate
/// either budget.
fn most_expensive_transaction(instructions: &[InstructionCostPreview]) -> MostExpensiveTransaction {
	let candidates: Vec<&InstructionCostPreview> = instructions
		.iter()
		.filter(|instruction| !instruction.ladders.is_empty())
		.collect();
	let most_rent = candidates.iter().copied().max_by(|left, right| {
		(
			left.total_rent_deficit_lamports,
			left.total_steps,
			&left.identity,
		)
			.cmp(&(
				right.total_rent_deficit_lamports,
				right.total_steps,
				&right.identity,
			))
	});
	let most_steps = candidates.iter().copied().max_by(|left, right| {
		(
			left.total_steps,
			left.total_rent_deficit_lamports,
			&left.identity,
		)
			.cmp(&(
				right.total_steps,
				right.total_rent_deficit_lamports,
				&right.identity,
			))
	});
	let (Some(most_rent), Some(most_steps)) = (most_rent, most_steps) else {
		return MostExpensiveTransaction::Unavailable {
			reason: NO_TOUCHING_INSTRUCTION.to_owned(),
		};
	};

	MostExpensiveTransaction::Identified {
		instruction_identity: most_rent.identity.clone(),
		instruction_rust_name: most_rent.rust_name.clone(),
		account_ladders: u32::try_from(most_rent.ladders.len()).unwrap_or(u32::MAX),
		steps: most_rent.total_steps,
		rent_deficit_lamports: most_rent.total_rent_deficit_lamports,
		static_cu: most_rent.static_cu.clone(),
		max_steps_instruction_identity: most_steps.identity.clone(),
		max_steps_instruction_rust_name: most_steps.rust_name.clone(),
		max_steps: most_steps.total_steps,
		model: LADDER_MODEL.to_owned(),
	}
}

/// Estimate the CU of every adjacent transition in `from_version..=current`.
fn estimate_ladder_cu(
	source: &ProfileSource,
	history: &ContractHistory,
	from_version: u32,
) -> StaticCuEstimate {
	let Some(profile) = &source.profile else {
		return StaticCuEstimate::Unavailable {
			reason: source.reason.clone(),
		};
	};
	let current_version = history.current().map_or(0, |current| current.version);
	let module = format!(
		"__pina_{}_{}_migrations",
		history.rust_name.to_snake_case(),
		history.identity.kind.as_str()
	);
	let mut estimated_cu = 0_u64;
	let mut missing = Vec::new();
	for to in from_version.saturating_add(1)..=current_version {
		let from = to - 1;
		let step = format!("v{from}_to_v{to}");
		let mut found = false;
		let mut step_cu = 0_u64;
		for function in profile.functions.iter().filter(|function| {
			matches_transition_function(&function.name, &module, &step)
				&& function.name.contains("migrate")
		}) {
			// A transition that genuinely costs zero still exists, so its
			// presence is tracked separately from its estimate.
			found = true;
			step_cu = step_cu.saturating_add(function.estimated_cu);
		}
		if !found {
			missing.push(step);
		}
		estimated_cu = estimated_cu.saturating_add(step_cu);
	}
	if !missing.is_empty() {
		return StaticCuEstimate::Unavailable {
			reason: format!(
				"`pina profile` has no estimate for {} in {module}; rebuild the SBF artifact with \
				 the generated transition modules",
				missing.join(", ")
			),
		};
	}

	StaticCuEstimate::Estimated {
		estimated_cu,
		model: CU_MODEL.to_owned(),
	}
}

/// Match one generated transition function inside a profile symbol name.
///
/// Rust mangles every path component with a length prefix (`8v0_to_v1`), which
/// keeps a search for `v0_to_v1` from matching the prefix of `v0_to_v12`.
/// Demangled or hand-written symbols separate components instead.
///
/// Each form must match a whole component: a symbol that merely embeds the step
/// (a suffix, a longer name, or a differently prefixed component) is a different
/// function and must not contribute to the estimate.
fn matches_transition_function(name: &str, module: &str, step: &str) -> bool {
	if !name.contains(module) {
		return false;
	}
	if name.starts_with("_ZN") {
		return legacy_mangled_walk_contains(name, module, step);
	}
	// Demangled: the module must be a whole component directly before the step,
	// either at the start of the name or after another component's `::`.
	let sequence = format!("{module}::{step}::");
	name.starts_with(&sequence) || name.contains(&format!("::{sequence}"))
}

/// Whether the legacy `_ZN…E` mangled symbol spells `step` as one of its path
/// components.
///
/// Each component is prefixed with its decimal length, so the walk reads a
/// count, takes exactly that many bytes, and compares. That makes the match
/// exact even where a boundary heuristic would be fooled: inside
/// `13foo8v0_to_v1` the bytes `8v0_to_v1` follow a non-digit, but the walk
/// consumes `foo8v0_to_v1` as the single component it is.
fn legacy_mangled_walk_contains(name: &str, module: &str, step: &str) -> bool {
	let Some(mut rest) = name.strip_prefix("_ZN") else {
		return false;
	};
	let mut previous_component: Option<&str> = None;

	loop {
		let digits = rest.len().saturating_sub(
			rest.trim_start_matches(|character: char| character.is_ascii_digit())
				.len(),
		);
		if digits == 0 {
			// Not a component start: the terminator, a hash tail, or a
			// hand-truncated symbol. Either way the step was not seen.
			return false;
		}
		let (count, after) = rest.split_at(digits);
		let Ok(length) = count.parse::<usize>() else {
			return false;
		};
		let Some(component) = after.get(..length) else {
			// The declared component runs past the end of the symbol.
			return false;
		};
		// The step only counts when it sits directly after the migration
		// module, so a non-migration component that merely contains the
		// module text cannot smuggle its cost into the estimate.
		if previous_component == Some(module) && component == step {
			return true;
		}
		previous_component = Some(component);
		rest = &after[length..];
		if rest.is_empty() {
			return false;
		}
	}
}

/// Test-only helpers used by the cost tests to build histories and profiles.
#[cfg(test)]
#[path = "cost_tests.rs"]
mod tests;
