use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::ConstantValueNode;
use codama_nodes::EventNode;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::ProgramNode;
use codama_nodes::RootNode;
use codama_nodes::StructFieldTypeNode;
use codama_nodes::StructTypeNode;

use super::MigrationPlan;
use super::js_migrate_instruction_module;
use crate::js_events::emit_js_event_log_module;

fn run_client_test(
	workspace: &Path,
	test_source: &str,
	write_generated_files: impl FnOnce(&Path) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
	let tsc = workspace.join("node_modules/typescript/bin/tsc");
	let dependencies_installed = tsc.is_file();

	// Full, coverage, and mutation runs install the workspace's JS dependencies.
	// The Windows CLI portability job runs Cargo without them.
	assert!(
		dependencies_installed || !cfg!(coverage),
		"generated client coverage requires pnpm install --frozen-lockfile"
	);

	dependencies_installed
		.then(|| {
			let fixture = tempfile::tempdir_in(workspace)?;
			let root = fixture.path();
			fs::write(root.join("contract.ts"), test_source)?;
			write_generated_files(root)?;

			let typecheck = Command::new("node")
				.current_dir(root)
				.arg(tsc)
				.args([
					"--strict",
					"--skipLibCheck",
					"--target",
					"es2022",
					"--module",
					"esnext",
					"--moduleResolution",
					"bundler",
					"--noEmit",
					"contract.ts",
				])
				.output()?;
			assert!(
				typecheck.status.success(),
				"generated TypeScript failed type checking:\n{}{}",
				String::from_utf8_lossy(&typecheck.stdout),
				String::from_utf8_lossy(&typecheck.stderr),
			);

			let runtime = Command::new("node")
				.current_dir(root)
				.args(["--import", "tsx", "contract.ts"])
				.output()?;
			assert!(
				runtime.status.success(),
				"generated TypeScript failed at runtime:\n{}",
				String::from_utf8_lossy(&runtime.stderr),
			);

			Ok::<(), Box<dyn Error>>(())
		})
		.transpose()
		.expect("generated client contract test failed");

	Ok(())
}

#[test]
fn generated_migrate_instruction_keeps_account_roles_and_optional_slots()
-> Result<(), Box<dyn Error>> {
	let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

	run_client_test(
		&workspace,
		include_str!("migrate_instruction_test.ts"),
		|fixture| {
			let idl = fs::read(workspace.join("codama/idls/migrations_program.json"))?;
			let RootNode { program, .. } = serde_json::from_slice(&idl)?;
			let plan = MigrationPlan::read(&program)?.ok_or("expected migratable accounts")?;
			let program_address = serde_json::to_string(&program.public_key)?;
			fs::create_dir(fixture.join("programs"))?;
			fs::write(
				fixture.join("programs/index.ts"),
				format!(
					"import {{ address }} from '@solana/kit';\nexport const \
					 MIGRATIONS_PROGRAM_PROGRAM_ADDRESS = address({program_address});\n"
				),
			)?;
			fs::create_dir(fixture.join("instructions"))?;
			fs::write(
				fixture.join("instructions/migrate.ts"),
				js_migrate_instruction_module("migrations_program", &plan),
			)?;

			Ok(())
		},
	)
}

#[test]
fn generated_event_parser_distinguishes_truncated_and_unknown_versions()
-> Result<(), Box<dyn Error>> {
	let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

	run_client_test(
		&workspace,
		include_str!("event_version_test.ts"),
		|fixture| {
			let discriminator = NumberValueNode::new(0x1234_u16);
			let discriminator_type = NumberTypeNode::le(NumberFormat::U16);
			let mut discriminator_field =
				StructFieldTypeNode::new("discriminator", discriminator_type.clone());
			discriminator_field.default_value = Box::new(Some(discriminator.into()));
			let mut version_field =
				StructFieldTypeNode::new("migrationVersion", NumberTypeNode::le(NumberFormat::U8));
			version_field.default_value = Box::new(Some(NumberValueNode::new(7_u8).into()));
			let value_field =
				StructFieldTypeNode::new("value", NumberTypeNode::le(NumberFormat::U8));
			let mut event = EventNode::new(
				"changed",
				StructTypeNode::new(vec![discriminator_field, version_field, value_field]),
			);
			event.discriminators.push(
				ConstantDiscriminatorNode::new(
					ConstantValueNode::new(discriminator_type, discriminator),
					0,
				)
				.into(),
			);
			let program = ProgramNode {
				public_key: "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS".to_owned(),
				events: vec![event],
				..ProgramNode::default()
			};
			emit_js_event_log_module(fixture, "migrations_program", &RootNode::new(program))?;
			fs::write(
				fixture.join("events/changed.ts"),
				include_str!("event_codec_fixture.ts"),
			)?;

			Ok(())
		},
	)
}
