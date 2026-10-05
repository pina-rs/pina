//! `pina deploy --rehearse`: the rehearsal network, the artifact pin, and the
//! JSON document. The rehearsal itself runs in the CLI suites, which provide a
//! fake Surfpool.

use super::*;
use crate::rehearse::REPORT_SCHEMA_VERSION;
use crate::rehearse::RehearsalSummary;

/// A project whose declared ID matches its program keypair, with the artifact
/// and keypairs passed explicitly so no target directory is involved.
struct RehearsedProject {
	_directory: tempfile::TempDir,
	root: PathBuf,
	program: PathBuf,
	program_keypair: PathBuf,
	authority: PathBuf,
}

impl RehearsedProject {
	fn new() -> Self {
		let directory = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
		let root = directory.path().join("rehearsed");
		fs::create_dir_all(root.join("src")).unwrap_or_else(|error| panic!("create src: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			"[package]\nname = \"rehearsed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
		)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
		let program = root.join("rehearsed.so");
		fs::write(&program, b"\x7fELF planned").unwrap_or_else(|error| panic!("program: {error}"));
		let program_keypair = root.join("program-keypair.json");
		let authority = root.join("authority.json");
		let program_id = write_keypair(&program_keypair, 21);
		write_keypair(&authority, 22);
		fs::write(
			root.join("src/lib.rs"),
			format!("use pina::prelude::*;\ndeclare_id!(\"{program_id}\");\n"),
		)
		.unwrap_or_else(|error| panic!("write source: {error}"));

		Self {
			_directory: directory,
			root,
			program,
			program_keypair,
			authority,
		}
	}

	fn plan(&self, cluster: &str) -> DeploymentPlan {
		prepare_deployment(&DeploymentRequest {
			project: self.root.clone(),
			remote_command: None,
			program: Some(self.program.clone()),
			program_keypair: Some(self.program_keypair.clone()),
			upgrade_authority: self.authority.clone(),
			payer: self.authority.clone(),
			target: DeploymentTarget::from_cluster_arg(cluster),
		})
		.unwrap_or_else(|error| panic!("plan a deployment to {cluster}: {error}"))
	}
}

/// Write an owner-only keypair from `seed` and return its address.
fn write_keypair(path: &Path, seed: u8) -> String {
	let signing_key = SigningKey::from_bytes(&[seed; 32]);
	let mut bytes = signing_key.to_bytes().to_vec();
	bytes.extend_from_slice(&signing_key.verifying_key().to_bytes());
	fs::write(path, serde_json::to_vec(&bytes).unwrap_or_default())
		.unwrap_or_else(|error| panic!("write keypair: {error}"));

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt as _;

		fs::set_permissions(path, fs::Permissions::from_mode(0o600))
			.unwrap_or_else(|error| panic!("protect keypair: {error}"));
	}

	bs58::encode(signing_key.verifying_key().to_bytes()).into_string()
}

#[test]
fn rehearsals_replay_the_cluster_the_deployment_targets() {
	let project = RehearsedProject::new();

	for (cluster, network) in [
		(
			"localnet",
			RehearseNetwork::Cluster(RehearseCluster::Localnet),
		),
		("devnet", RehearseNetwork::Cluster(RehearseCluster::Devnet)),
		(
			"testnet",
			RehearseNetwork::Cluster(RehearseCluster::Testnet),
		),
		(
			"mainnet-beta",
			RehearseNetwork::Cluster(RehearseCluster::Mainnet),
		),
		(
			"https://rpc.example/v2/provider-key",
			RehearseNetwork::RpcUrl("https://rpc.example/v2/provider-key".to_owned()),
		),
	] {
		assert_eq!(project.plan(cluster).rehearsal_network(), network);
	}
}

#[test]
fn rehearsals_read_only_the_planned_artifact() {
	let project = RehearsedProject::new();
	let plan = project.plan("localnet");
	fs::write(&project.program, b"\x7fELF replaced after planning")
		.unwrap_or_else(|error| panic!("replace program: {error}"));
	let mut progress = Vec::new();
	let replaced = rehearse_deployment(&plan, 1, &mut progress)
		.err()
		.unwrap_or_else(|| panic!("a replaced artifact is never rehearsed"));

	assert!(
		matches!(
			replaced,
			DeploymentRehearsalError::Deploy(DeployError::InputsChanged)
		),
		"{replaced}"
	);
	assert_eq!(replaced.exit_code(), 1);
	assert!(progress.is_empty());

	fs::remove_file(&project.program).unwrap_or_else(|error| panic!("remove program: {error}"));
	let removed = rehearse_deployment(&plan, 1, &mut progress)
		.err()
		.unwrap_or_else(|| panic!("a removed artifact is never rehearsed"));

	assert!(
		matches!(
			&removed,
			DeploymentRehearsalError::Deploy(DeployError::InvalidFile { kind: "program", path })
				if path.as_os_str() == plan.program()
		),
		"{removed}"
	);
	assert_eq!(removed.exit_code(), 1);
}

#[test]
fn first_deployments_are_unverified_and_failures_are_operational() {
	let first = DeploymentRehearsalError::FirstDeployment {
		program_id: "Program1111".to_owned(),
		cluster: "devnet".to_owned(),
	};

	assert_eq!(first.exit_code(), 3);
	assert_eq!(
		first.to_string(),
		"program Program1111 is not deployed on devnet yet, so there is no deployed program to \
		 rehearse against; deploy it the first time without --rehearse"
	);

	let failed =
		DeploymentRehearsalError::Rehearse(Box::new(RehearseError::InvalidLimit { limit: 0 }));

	assert_eq!(failed.exit_code(), 1);
	assert!(
		failed.to_string().starts_with("the rehearsal failed: "),
		"{failed}"
	);
}

#[test]
fn rehearsed_plans_keep_every_plan_key_and_add_the_report() {
	let project = RehearsedProject::new();
	let plan = project.plan("localnet");
	let report = RehearsalReport {
		schema_version: REPORT_SCHEMA_VERSION,
		cluster: "localnet".to_owned(),
		program_id: plan.program_id().to_owned(),
		deployed_sha256: "deployed".to_owned(),
		candidate_sha256: "candidate".to_owned(),
		slot: 7,
		summary: RehearsalSummary::default(),
		instructions: Vec::new(),
		transactions: Vec::new(),
	};
	let rehearsed = serde_json::to_value(RehearsedDeploymentPlan {
		plan: &plan,
		rehearsal: &report,
	})
	.unwrap_or_else(|error| panic!("serialize the rehearsed plan: {error}"));
	let mut expected =
		serde_json::to_value(&plan).unwrap_or_else(|error| panic!("serialize the plan: {error}"));
	expected["rehearsal"] = serde_json::to_value(&report)
		.unwrap_or_else(|error| panic!("serialize the report: {error}"));

	assert_eq!(rehearsed, expected);
	assert_eq!(rehearsed["rehearsal"]["schemaVersion"], 1);
	assert_eq!(rehearsed["rehearsal"]["slot"], 7);
}
