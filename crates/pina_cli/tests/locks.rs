//! Write-lock analysis of real example programs.

use std::path::Path;

use pina_cli::locks::LockReport;
use pina_cli::locks::analyze_project;

fn example(name: &str) -> LockReport {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../examples")
		.join(name);

	analyze_project(&path).unwrap_or_else(|error| panic!("locks for {name} failed: {error}"))
}

#[test]
fn privacy_pool_serializes_its_global_state() {
	let report = example("privacy_pool_program");
	let hotspots = report
		.hotspots
		.iter()
		.map(|hotspot| hotspot.name.as_str())
		.collect::<Vec<_>>();

	assert_eq!(
		hotspots,
		[
			"pool_config",
			"pool_vault",
			"merkle_tree",
			"nullifier_set",
			"custodian_registry",
			"requester_registry",
			"disclosure_log",
		]
	);
	insta::assert_snapshot!("privacy_pool_program_text", report.render_text());
	insta::assert_json_snapshot!("privacy_pool_program_json", report);
}

#[test]
fn multisig_has_one_admin_hotspot() {
	let report = example("multisig_program");

	assert_eq!(report.hotspots.len(), 1);
	assert_eq!(report.hotspots[0].name, "program_config");
	assert_eq!(
		report.hotspots[0].writers,
		["config_initialize", "config_update"]
	);
	assert_eq!(
		report.hotspots[0].readers,
		["multisig_create", "multisig_import"]
	);
	insta::assert_snapshot!("multisig_program_text", report.render_text());
	insta::assert_json_snapshot!("multisig_program_json", report);
}

#[test]
fn counter_and_escrow_only_contend_per_user() {
	for name in ["counter_program", "escrow_program"] {
		let report = example(name);

		assert!(report.hotspots.is_empty(), "{name} has no global state");
		insta::assert_snapshot!(format!("{name}_text"), report.render_text());
		insta::assert_json_snapshot!(format!("{name}_json"), report);
	}
}
