//! Tests for the pre-deploy migration cost preview.

use std::path::PathBuf;

use pina_abi::ContractHistory;
use pina_abi::ContractIdentity;
use pina_abi::ContractKind;
use pina_abi::DataSchema;
use pina_abi::FieldSchema;
use pina_abi::LayoutKind;
use pina_abi::MigrationManifest;
use pina_abi::MigrationVersionType;
use pina_abi::ProcessAccount;
use pina_abi::ProcessContract;
use pina_abi::SchemaVersion;
use pina_profile::FunctionProfile;
use pina_profile::ProgramProfile;

use super::*;

/// Exact rent figure the `make` warning quotes, repeated so a constant change
/// fails this test instead of silently shifting every expectation.
const RENT_PER_BYTE: u64 = 6_960;

fn schema(layout: LayoutKind, fields: &[(&str, &str)]) -> DataSchema {
	DataSchema::try_new(
		layout,
		fields
			.iter()
			.map(|(name, rust_type)| {
				FieldSchema {
					name: (*name).to_owned(),
					rust_type: (*rust_type).to_owned(),
				}
			})
			.collect(),
	)
	.unwrap_or_else(|error| panic!("valid test schema: {error}"))
}

fn fixed_schema(payload_bytes: usize) -> DataSchema {
	schema(
		LayoutKind::Fixed,
		&[("data", &format!("[u8; {payload_bytes}]"))],
	)
}

/// One version entry. The version number is its position in the history, so it
/// is not stored on the entry.
fn version(schema: DataSchema) -> SchemaVersion {
	SchemaVersion {
		schema,
		process: None,
		transition: None,
	}
}

fn account_history(
	discriminator: u64,
	rust_name: &str,
	schemas: Vec<DataSchema>,
) -> ContractHistory {
	ContractHistory {
		identity: ContractIdentity::try_new(ContractKind::Account, 1, discriminator)
			.unwrap_or_else(|error| panic!("valid account identity: {error}")),
		rust_name: rust_name.to_owned(),
		versions: schemas.into_iter().map(version).collect(),
	}
}

fn instruction_history(
	discriminator: u64,
	rust_name: &str,
	payload_bytes: usize,
	slots: &[&str],
) -> ContractHistory {
	instruction_history_with_slots(
		discriminator,
		rust_name,
		payload_bytes,
		slots
			.iter()
			.map(|name| (*name, true, false))
			.collect::<Vec<_>>()
			.as_slice(),
	)
}

/// Build an instruction whose slots carry explicit privileges, so tests can
/// model authorities (signers) and payers (writable signers) faithfully.
fn instruction_history_with_slots(
	discriminator: u64,
	rust_name: &str,
	payload_bytes: usize,
	slots: &[(&str, bool, bool)],
) -> ContractHistory {
	let schema = fixed_schema(payload_bytes);
	let process = ProcessContract {
		accounts: slots
			.iter()
			.map(|(name, writable, signer)| {
				ProcessAccount {
					name: (*name).to_owned(),
					writable: *writable,
					signer: *signer,
					optional: true,
					default_value: None,
					pda: None,
				}
			})
			.collect(),
	};
	let current = SchemaVersion {
		schema,
		process: Some(process),
		transition: None,
	};

	ContractHistory {
		identity: ContractIdentity::try_new(ContractKind::Instruction, 1, discriminator)
			.unwrap_or_else(|error| panic!("valid instruction identity: {error}")),
		rust_name: rust_name.to_owned(),
		versions: vec![current],
	}
}

/// An instruction version without a process contract, as an older manifest
/// format may hold.
fn instruction_without_process(
	discriminator: u64,
	rust_name: &str,
	payload_bytes: usize,
) -> ContractHistory {
	ContractHistory {
		identity: ContractIdentity::try_new(ContractKind::Instruction, 1, discriminator)
			.unwrap_or_else(|error| panic!("valid instruction identity: {error}")),
		rust_name: rust_name.to_owned(),
		versions: vec![version(fixed_schema(payload_bytes))],
	}
}

fn manifest(histories: Vec<ContractHistory>) -> MigrationManifest {
	let mut manifest = MigrationManifest::new("program".to_owned(), MigrationVersionType::U8);
	for history in histories {
		manifest.contracts.insert(history.identity.key(), history);
	}
	manifest
}

fn profile_source(functions: &[(&str, u64)]) -> ProfileSource {
	ProfileSource {
		profile: Some(ProgramProfile {
			program_name: "program".to_owned(),
			binary_size: 0,
			text_size: 0,
			total_instructions: 0,
			total_syscalls: 0,
			total_cu: 0,
			functions: functions
				.iter()
				.map(|(name, estimated_cu)| {
					FunctionProfile {
						name: (*name).to_owned(),
						offset: 0,
						size: 0,
						instruction_count: 0,
						syscall_count: 0,
						estimated_cu: *estimated_cu,
					}
				})
				.collect(),
		}),
		artifact: Some(PathBuf::from("target/deploy/program.so")),
		reason: String::new(),
	}
}

fn unavailable_source(reason: &str) -> ProfileSource {
	ProfileSource {
		profile: None,
		artifact: Some(PathBuf::from("target/deploy/program.so")),
		reason: reason.to_owned(),
	}
}

fn ladder_of(preview: &MigrationCostPreview) -> &LadderCost {
	preview.contracts[0]
		.worst_case_ladder
		.as_ref()
		.unwrap_or_else(|| panic!("account contract has a ladder"))
}

fn estimated_cu(estimate: &StaticCuEstimate) -> u64 {
	match estimate {
		StaticCuEstimate::Estimated { estimated_cu, .. } => *estimated_cu,
		StaticCuEstimate::Unavailable { reason } => {
			panic!("expected a static estimate, found: {reason}")
		}
	}
}

#[test]
fn fixed_account_reports_day_one_growth_and_rent() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(40), fixed_schema(41), fixed_schema(42)],
	)]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));

	assert_eq!(preview.contracts.len(), 1);
	let contract = &preview.contracts[0];
	assert_eq!(contract.identity, "account:1:01");
	assert_eq!(contract.rust_name, "State");
	assert_eq!(contract.current_version, 2);
	assert_eq!(contract.transition_count, 2);
	assert_eq!(contract.current_size_bytes, 44);
	assert_eq!(contract.day_one_growth_bytes, 2);
	assert_eq!(contract.day_one_rent_deficit_lamports, 2 * RENT_PER_BYTE);
	assert!(contract.notes.is_empty());

	let ladder = ladder_of(&preview);
	assert_eq!(ladder.from_version, 0);
	assert_eq!(ladder.to_version, 2);
	assert_eq!(ladder.steps, 2);
	assert!(ladder.day_one);
	assert_eq!(ladder.rent_deficit_lamports, 2 * RENT_PER_BYTE);
	assert_eq!(
		ladder.static_cu,
		StaticCuEstimate::Unavailable {
			reason: "no artifact".to_owned()
		}
	);
}

#[test]
fn compact_account_quotes_worst_case_capacity() {
	let manifest = manifest(vec![account_history(
		3,
		"CompactState",
		vec![
			schema(LayoutKind::Compact, &[("name", "String<5>")]),
			schema(LayoutKind::Compact, &[("name", "String<11>")]),
		],
	)]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));
	let contract = &preview.contracts[0];

	assert_eq!(contract.current_size_bytes, 14);
	assert_eq!(contract.day_one_growth_bytes, 6);
	assert_eq!(contract.day_one_rent_deficit_lamports, 6 * RENT_PER_BYTE);
	assert_eq!(ladder_of(&preview).rent_deficit_lamports, 6 * RENT_PER_BYTE);
}

#[test]
fn single_version_account_has_no_ladder_or_growth() {
	let manifest = manifest(vec![account_history(1, "State", vec![fixed_schema(40)])]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));
	let contract = &preview.contracts[0];

	assert_eq!(contract.current_version, 0);
	assert_eq!(contract.transition_count, 0);
	assert_eq!(contract.day_one_growth_bytes, 0);
	assert_eq!(contract.day_one_rent_deficit_lamports, 0);
	assert!(contract.worst_case_ladder.is_none());
}

#[test]
fn ladder_cu_sums_mangled_transition_functions() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(40), fixed_schema(41), fixed_schema(42)],
	)]);
	let source = profile_source(&[
		(
			"_ZN8my_crate31__pina_state_account_migrations8v0_to_v17migrate17h0000E",
			10,
		),
		(
			"_ZN8my_crate31__pina_state_account_migrations8v1_to_v27migrate17h1111E",
			20,
		),
		// Same module and step but not the transition function, so it is ignored.
		(
			"_ZN8my_crate31__pina_state_account_migrations8v1_to_v27plan17h2222E",
			5_000,
		),
	]);
	let preview = build_cost_preview(Some(&manifest), &source);
	let ladder = ladder_of(&preview);

	let StaticCuEstimate::Estimated {
		estimated_cu,
		model,
	} = &ladder.static_cu
	else {
		panic!("mangled transition functions are estimated");
	};
	assert_eq!(*estimated_cu, 30);
	assert!(model.contains("pina profile"), "model: {model}");
	assert!(preview.artifact.is_some());
}

#[test]
fn ladder_cu_matches_demangled_transition_functions() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(40), fixed_schema(41), fixed_schema(42)],
	)]);
	let source = profile_source(&[
		(
			"my_crate::__pina_state_account_migrations::v0_to_v1::migrate",
			7,
		),
		(
			"my_crate::__pina_state_account_migrations::v1_to_v2::migrate",
			11,
		),
	]);
	let preview = build_cost_preview(Some(&manifest), &source);

	assert_eq!(estimated_cu(&ladder_of(&preview).static_cu), 18);
}

#[test]
fn transition_matching_requires_the_exact_step() {
	let module = "__pina_state_account_migrations";

	assert!(matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations9v0_to_v12migrateE",
		module,
		"v0_to_v12"
	));
	// The length prefix on each mangled path component stops `v0_to_v12` from
	// satisfying a request for `v0_to_v1`.
	assert!(!matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations9v0_to_v12migrateE",
		module,
		"v0_to_v1"
	));
	assert!(matches_transition_function(
		"my_crate::__pina_state_account_migrations::v0_to_v1::migrate",
		module,
		"v0_to_v1"
	));
	assert!(!matches_transition_function(
		"my_crate::__pina_other_account_migrations::v0_to_v1::migrate",
		module,
		"v0_to_v1"
	));
}

#[test]
fn transition_matching_ignores_steps_embedded_in_longer_names() {
	let module = "__pina_state_account_migrations";

	// A component whose *count* continues a longer number is a different
	// function: `18v0_to_v1_migration` contains `8v0_to_v1` but is not the
	// `v0_to_v1` transition.
	assert!(!matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations18v0_to_v1_migrationmigrateE",
		module,
		"v0_to_v1"
	));
	// A longer component that merely starts with the step text.
	assert!(!matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations9v0_to_v12migrateE",
		module,
		"v0_to_v1"
	));
	// Demangled neighbour: `::` delimits, so the longer name cannot match.
	assert!(!matches_transition_function(
		"my_crate::__pina_state_account_migrations::v0_to_v12::migrate",
		module,
		"v0_to_v1"
	));
	// The exact forms still match.
	assert!(matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations8v0_to_v17migrate17h0000E",
		module,
		"v0_to_v1"
	));
	assert!(matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations8v0_to_v17migrateE",
		module,
		"v0_to_v1"
	));
}

#[test]
fn a_rejected_match_near_the_end_terminates_the_search() {
	let module = "__pina_state_account_migrations";

	// The only occurrence continues a longer count and sits at the very end of
	// the name, so the walk must reject it and stop rather than reading a
	// component that runs past the end of the string.
	assert!(!matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations18v0_to_v1",
		module,
		"v0_to_v1"
	));
}

#[test]
fn the_module_must_be_a_whole_component_before_the_step() {
	let module = "__pina_state_account_migrations";

	// A non-migration component that contains the module text cannot lend its
	// cost to the estimate, even when the real step follows it.
	assert!(!matches_transition_function(
		"_ZN8my_crate41xx__pina_state_account_migrationsyy8v0_to_v1migrateE",
		module,
		"v0_to_v1"
	));
	// The demangled form demands the same adjacency.
	assert!(!matches_transition_function(
		"my_crate::xx__pina_state_account_migrationsyy::v0_to_v1::migrate",
		module,
		"v0_to_v1"
	));
	// The real adjacency still matches in both spellings.
	assert!(matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations8v0_to_v1migrateE",
		module,
		"v0_to_v1"
	));
	assert!(matches_transition_function(
		"my_crate::__pina_state_account_migrations::v0_to_v1::migrate",
		module,
		"v0_to_v1"
	));
}

#[test]
fn a_component_merely_containing_the_step_text_does_not_match() {
	let module = "__pina_state_account_migrations";

	// One component is literally named `foo8v0_to_v1` (length 12). The bytes
	// `8v0_to_v1` inside it follow a non-digit, but the component walk consumes
	// the whole length-prefixed name, so it is not the transition function.
	assert!(!matches_transition_function(
		"_ZN8my_crate31__pina_state_account_migrations12foo8v0_to_v1migrateE",
		module,
		"v0_to_v1"
	));
}

#[test]
fn a_zero_cost_transition_is_estimated_rather_than_reported_missing() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(40), fixed_schema(41)],
	)]);
	// The transition exists but its measured cost is zero.
	let source = profile_source(&[(
		"_ZN8my_crate31__pina_state_account_migrations8v0_to_v17migrate17h0000E",
		0,
	)]);
	let preview = build_cost_preview(Some(&manifest), &source);
	let ladder = ladder_of(&preview);

	assert_eq!(
		estimated_cu(&ladder.static_cu),
		0,
		"a present zero-cost transition is still an estimate"
	);
}

#[test]
fn a_decoy_symbol_does_not_satisfy_a_missing_transition() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(40), fixed_schema(41)],
	)]);
	// Only a decoy that embeds the step in a longer component is present, so
	// the real transition is missing and the estimate must say so.
	let source = profile_source(&[(
		"_ZN8my_crate31__pina_state_account_migrations18v0_to_v1_migrationmigrateE",
		500,
	)]);
	let preview = build_cost_preview(Some(&manifest), &source);
	let ladder = ladder_of(&preview);

	assert!(
		matches!(ladder.static_cu, StaticCuEstimate::Unavailable { .. }),
		"a decoy symbol must not stand in for the real transition"
	);
}

#[test]
fn missing_transition_function_reports_the_missing_step() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(40), fixed_schema(41), fixed_schema(42)],
	)]);
	let source = profile_source(&[(
		"_ZN8my_crate31__pina_state_account_migrations8v0_to_v17migrate17h0000E",
		10,
	)]);
	let preview = build_cost_preview(Some(&manifest), &source);

	let StaticCuEstimate::Unavailable { reason } = &ladder_of(&preview).static_cu else {
		panic!("a missing transition function cannot be estimated");
	};
	assert!(reason.contains("v1_to_v2"), "reason: {reason}");
	assert!(
		reason.contains("__pina_state_account_migrations"),
		"reason: {reason}"
	);
}

#[test]
fn day_one_ladder_beyond_max_inline_steps_is_called_out() {
	let schemas = (40_usize..51).map(fixed_schema).collect::<Vec<_>>();
	let manifest = manifest(vec![account_history(1, "State", schemas)]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));
	let contract = &preview.contracts[0];

	assert_eq!(contract.current_version, 10);
	assert_eq!(contract.day_one_growth_bytes, 10);
	assert_eq!(contract.day_one_rent_deficit_lamports, 10 * RENT_PER_BYTE);
	let ladder = ladder_of(&preview);
	assert_eq!(ladder.from_version, 2);
	assert_eq!(ladder.to_version, 10);
	assert_eq!(ladder.steps, MAX_INLINE_STEPS);
	assert!(!ladder.day_one);
	assert_eq!(ladder.rent_deficit_lamports, 8 * RENT_PER_BYTE);
	assert_eq!(contract.notes.len(), 1);
	assert!(
		contract.notes[0].contains("MAX_INLINE_STEPS (8)"),
		"note: {}",
		contract.notes[0]
	);
	assert!(
		contract.notes[0].contains("starts at v2"),
		"note: {}",
		contract.notes[0]
	);
}

#[test]
fn oversized_step_growth_quotes_the_shared_growth_remedy() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(100), fixed_schema(20_000)],
	)]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));
	let contract = &preview.contracts[0];

	assert_eq!(contract.day_one_growth_bytes, 19_900);
	assert_eq!(
		contract.day_one_rent_deficit_lamports,
		19_900 * RENT_PER_BYTE
	);
	assert_eq!(contract.notes.len(), 1);
	assert!(
		contract.notes[0].contains("`MAX_PERMITTED_DATA_INCREASE` (10240 bytes)"),
		"note: {}",
		contract.notes[0]
	);
	assert!(
		contract.notes[0].contains("keep every released version within"),
		"note: {}",
		contract.notes[0]
	);
}

#[test]
fn instruction_ladders_match_account_slots_by_name() {
	let manifest = manifest(vec![
		account_history(1, "State", vec![fixed_schema(40), fixed_schema(41)]),
		account_history(2, "ManualState", vec![fixed_schema(3), fixed_schema(4)]),
		account_history(3, "UnusedState", vec![fixed_schema(1), fixed_schema(2)]),
		instruction_history(
			0,
			"UpdateInstruction",
			10,
			&["authority", "state", "manual_state", "payer"],
		),
	]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));

	assert_eq!(preview.instructions.len(), 1);
	let instruction = &preview.instructions[0];
	assert_eq!(instruction.identity, "instruction:1:00");
	assert_eq!(instruction.rust_name, "UpdateInstruction");
	assert_eq!(instruction.ladders.len(), 2);
	assert_eq!(instruction.total_steps, 2);
	assert_eq!(instruction.total_rent_deficit_lamports, 2 * RENT_PER_BYTE);
	assert_eq!(instruction.ladders[0].account_rust_name, "ManualState");
	assert_eq!(instruction.ladders[1].account_rust_name, "State");
	assert_eq!(
		instruction.static_cu,
		StaticCuEstimate::Unavailable {
			reason: "no artifact".to_owned()
		}
	);
	// `authority` and `payer` are writable, name no account contract, and must
	// not disappear silently.
	assert_eq!(instruction.notes.len(), 2);
	assert!(
		instruction.notes[0].contains("cannot link writable slot `authority`"),
		"note: {}",
		instruction.notes[0]
	);
	assert!(
		instruction.notes[1].contains("cannot link writable slot `payer`"),
		"note: {}",
		instruction.notes[1]
	);
}

#[test]
fn instruction_slots_skip_settled_accounts_and_missing_processes() {
	let manifest = manifest(vec![
		account_history(1, "State", vec![fixed_schema(40), fixed_schema(41)]),
		// A matched slot whose account never migrates contributes no ladder.
		account_history(2, "SettledState", vec![fixed_schema(10)]),
		instruction_history(0, "UpdateInstruction", 10, &["state", "settled_state"]),
		// A version without a process contract has no slots to match.
		instruction_without_process(1, "RelayInstruction", 10),
	]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));

	assert_eq!(preview.instructions.len(), 2);
	assert_eq!(preview.instructions[0].ladders.len(), 1);
	assert_eq!(
		preview.instructions[0].ladders[0].account_rust_name,
		"State"
	);
	assert!(preview.instructions[1].ladders.is_empty());
}

#[test]
fn instruction_without_matching_account_slots_is_explicit() {
	let manifest = manifest(vec![
		account_history(1, "State", vec![fixed_schema(40), fixed_schema(41)]),
		instruction_history(0, "RelayInstruction", 10, &["authority", "system_program"]),
	]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));
	let instruction = &preview.instructions[0];

	assert!(instruction.ladders.is_empty());
	assert_eq!(instruction.total_steps, 0);
	assert_eq!(instruction.total_rent_deficit_lamports, 0);
	assert_eq!(instruction.notes.len(), 2);
	assert!(
		instruction.notes[0].contains("cannot link writable slot `authority`"),
		"note: {}",
		instruction.notes[0]
	);
	let StaticCuEstimate::Unavailable { reason } = &instruction.static_cu else {
		panic!("an unmatched instruction cannot be estimated");
	};
	assert!(
		reason.contains("no checked-in account contract"),
		"reason: {reason}"
	);
	let MostExpensiveTransaction::Unavailable { reason } = &preview.most_expensive else {
		panic!("no touching transaction can be identified");
	};
	assert!(
		reason.contains("no migration-aware instruction process"),
		"reason: {reason}"
	);
}

#[test]
fn signer_and_readonly_slots_cannot_miss_account_contracts() {
	let manifest = manifest(vec![
		account_history(1, "State", vec![fixed_schema(40), fixed_schema(41)]),
		// A signer slot and a read-only slot can never hold an account the
		// executor migrates, so neither is join-candidate and neither is noted.
		instruction_history_with_slots(
			0,
			"RelayInstruction",
			10,
			&[
				("authority", false, true),
				("system_program", false, false),
				("state", true, false),
			],
		),
	]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));
	let instruction = &preview.instructions[0];

	assert!(instruction.notes.is_empty());
	assert_eq!(instruction.ladders.len(), 1);
	assert_eq!(instruction.ladders[0].account_rust_name, "State");
}

#[test]
fn most_expensive_transaction_picks_the_largest_rent() {
	let manifest = manifest(vec![
		account_history(1, "State", vec![fixed_schema(40), fixed_schema(41)]),
		account_history(2, "ManualState", vec![fixed_schema(3), fixed_schema(8)]),
		instruction_history(0, "CheapInstruction", 10, &["state"]),
		instruction_history(1, "ExpensiveInstruction", 10, &["manual_state"]),
	]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));

	let MostExpensiveTransaction::Identified {
		instruction_rust_name,
		account_ladders,
		steps,
		rent_deficit_lamports,
		max_steps_instruction_rust_name,
		max_steps,
		..
	} = &preview.most_expensive
	else {
		panic!("an instruction touches a migration-aware account");
	};
	assert_eq!(instruction_rust_name, "ExpensiveInstruction");
	assert_eq!(*account_ladders, 1);
	assert_eq!(*steps, 1);
	assert_eq!(*rent_deficit_lamports, 5 * RENT_PER_BYTE);
	// When one instruction holds both maxima, the summary names it twice.
	assert_eq!(max_steps_instruction_rust_name, "ExpensiveInstruction");
	assert_eq!(*max_steps, 1);
}

#[test]
fn longest_ladder_is_reported_independently_of_the_rent_maximum() {
	let manifest = manifest(vec![
		// `State` climbs 4 steps for 1 grown byte; `ManualState` climbs 1 step
		// for 5 grown bytes, so the rent and step maxima differ.
		account_history(
			1,
			"State",
			vec![
				fixed_schema(40),
				fixed_schema(40),
				fixed_schema(40),
				fixed_schema(40),
				fixed_schema(41),
			],
		),
		account_history(2, "ManualState", vec![fixed_schema(3), fixed_schema(8)]),
		instruction_history(0, "LongInstruction", 10, &["state"]),
		instruction_history(1, "RichInstruction", 10, &["manual_state"]),
	]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));

	let MostExpensiveTransaction::Identified {
		instruction_rust_name,
		rent_deficit_lamports,
		max_steps_instruction_rust_name,
		max_steps,
		..
	} = &preview.most_expensive
	else {
		panic!("an instruction touches a migration-aware account");
	};
	assert_eq!(instruction_rust_name, "RichInstruction");
	assert_eq!(*rent_deficit_lamports, 5 * RENT_PER_BYTE);
	assert_eq!(max_steps_instruction_rust_name, "LongInstruction");
	assert_eq!(*max_steps, 4);
}

#[test]
fn abstract_model_fields_and_remedies_are_published() {
	let manifest = manifest(vec![account_history(
		1,
		"State",
		vec![fixed_schema(40), fixed_schema(41)],
	)]);
	let preview = build_cost_preview(Some(&manifest), &unavailable_source("no artifact"));

	assert_eq!(preview.rent_lamports_per_byte, RENT_PER_BYTE);
	assert_eq!(preview.max_inline_steps, 8);
	assert!(preview.ladder_model.contains("MAX_INLINE_STEPS"));
	assert!(preview.cu_model.contains("pina profile"));
	assert!(preview.lamport_budget_remedy.contains("max_lamports"));
	assert!(
		preview
			.account_growth_remedy
			.contains("MAX_PERMITTED_DATA_INCREASE")
	);
}

#[test]
fn missing_manifest_builds_an_empty_preview() {
	let source =
		unavailable_source("the program has no migration-aware account contract to estimate");
	let preview = build_cost_preview(None, &source);

	assert!(preview.contracts.is_empty());
	assert!(preview.instructions.is_empty());
	assert!(matches!(
		preview.most_expensive,
		MostExpensiveTransaction::Unavailable { .. }
	));
}

#[test]
fn serialized_preview_uses_additive_camel_case_keys() {
	let manifest = manifest(vec![
		account_history(1, "State", vec![fixed_schema(40), fixed_schema(41)]),
		instruction_history(0, "UpdateInstruction", 10, &["state"]),
	]);
	let preview = build_cost_preview(
		Some(&manifest),
		&profile_source(&[(
			"_ZN8my_crate31__pina_state_account_migrations8v0_to_v17migrate17h0000E",
			12,
		)]),
	);
	let json = serde_json::to_value(&preview)
		.unwrap_or_else(|error| panic!("serialize cost preview: {error}"));

	assert_eq!(json["rentLamportsPerByte"], RENT_PER_BYTE);
	assert_eq!(json["maxInlineSteps"], 8);
	assert_eq!(json["contracts"][0]["dayOneGrowthBytes"], 1);
	assert_eq!(
		json["contracts"][0]["dayOneRentDeficitLamports"],
		RENT_PER_BYTE
	);
	assert_eq!(json["contracts"][0]["worstCaseLadder"]["fromVersion"], 0);
	assert_eq!(
		json["contracts"][0]["worstCaseLadder"]["staticCu"]["status"],
		"estimated"
	);
	assert_eq!(
		json["contracts"][0]["worstCaseLadder"]["staticCu"]["estimatedCu"],
		12
	);
	assert_eq!(json["instructions"][0]["ladders"][0]["steps"], 1);
	assert_eq!(json["instructions"][0]["staticCu"]["estimatedCu"], 12);
	assert_eq!(json["instructions"][0]["notes"], serde_json::json!([]));
	assert_eq!(json["mostExpensive"]["status"], "identified");
	assert_eq!(
		json["mostExpensive"]["instructionRustName"],
		"UpdateInstruction"
	);
	assert_eq!(json["mostExpensive"]["maxSteps"], 1);
	assert_eq!(
		json["mostExpensive"]["maxStepsInstructionRustName"],
		"UpdateInstruction"
	);
}
