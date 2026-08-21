use std::fs;
use std::path::Path;

use pina_cli::error::IdlError;
use pina_cli::parse::parse_program;

const PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";

fn write_program(root: &Path, manifest: &str, lib: &str) {
	let src = root.join("src");
	fs::create_dir_all(&src).unwrap_or_else(|error| panic!("create src: {error}"));
	fs::write(root.join("Cargo.toml"), manifest)
		.unwrap_or_else(|error| panic!("write manifest: {error}"));
	fs::write(src.join("lib.rs"), lib).unwrap_or_else(|error| panic!("write lib.rs: {error}"));
}

fn manifest() -> &'static str {
	"[package]\nname = \"fail_closed_fixture\"\nversion = \"0.0.0\"\n"
}

fn expect_idl_error(root: &Path, context: &str) -> IdlError {
	match parse_program(root, None) {
		Ok(ir) => {
			std::mem::forget(ir);
			panic!("{context}");
		}
		Err(error) => error,
	}
}

fn valid_program(prefix: &str, process_body: &str) -> String {
	format!(
		r#"
		{prefix}
		declare_id!("{PROGRAM_ID}");

		#[discriminator]
		pub enum ExampleInstruction {{ Run = 0 }}

		#[instruction(discriminator = ExampleInstruction, variant = Run)]
		pub struct RunInstruction {{}}

		#[derive(Accounts)]
		pub struct RunAccounts<'a> {{
			pub authority: &'a AccountView,
		}}

		impl<'a> ProcessAccountInfos<'a> for RunAccounts<'a> {{
			fn process(self, _data: &[u8]) -> ProgramResult {{
				{process_body}
				Ok(())
			}}
		}}

		pub fn process_instruction(
			program_id: &Address,
			accounts: &mut [AccountView],
			data: &[u8],
		) -> ProgramResult {{
			let instruction: ExampleInstruction = parse_instruction(program_id, &ID, data)?;
			match instruction {{
				ExampleInstruction::Run => RunAccounts::try_from(accounts)?.process(data),
			}}
		}}
	"#,
	)
}

#[test]
fn rejects_missing_unconditional_module_files() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	write_program(dir.path(), manifest(), &valid_program("mod missing;", ""));

	let error = expect_idl_error(dir.path(), "missing modules must fail closed");
	assert!(matches!(error, IdlError::Io { .. }));
	assert!(error.to_string().contains("missing.rs"));
}

#[test]
fn permits_missing_cfg_gated_module_files() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	write_program(
		dir.path(),
		manifest(),
		&valid_program("#[cfg(feature = \"client\")]\nmod client;", ""),
	);

	let ir = parse_program(dir.path(), None)
		.unwrap_or_else(|error| panic!("cfg-gated module should be optional: {error}"));
	assert_eq!(ir.instructions.len(), 1);
}

#[test]
fn resolves_explicit_path_modules() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	let source = format!(
		r#"
		#[path = "generated/accounts.rs"]
		mod accounts;
		declare_id!("{PROGRAM_ID}");

		#[discriminator]
		pub enum ExampleInstruction {{ Run = 0 }}
		#[instruction(discriminator = ExampleInstruction, variant = Run)]
		pub struct RunInstruction {{}}

		pub fn process_instruction(
			program_id: &Address,
			accounts: &mut [AccountView],
			data: &[u8],
		) -> ProgramResult {{
			let instruction: ExampleInstruction = parse_instruction(program_id, &ID, data)?;
			match instruction {{
				ExampleInstruction::Run => RunAccounts::try_from(accounts)?.process(data),
			}}
		}}
	"#,
	);
	write_program(dir.path(), manifest(), &source);
	let generated = dir.path().join("src/generated");
	fs::create_dir_all(&generated).unwrap_or_else(|error| panic!("create generated: {error}"));
	fs::write(
		generated.join("accounts.rs"),
		r#"
		#[derive(Accounts)]
		pub struct RunAccounts<'a> { pub authority: &'a AccountView }
	"#,
	)
	.unwrap_or_else(|error| panic!("write explicit module: {error}"));

	let ir = parse_program(dir.path(), None)
		.unwrap_or_else(|error| panic!("explicit path should resolve: {error}"));
	assert_eq!(ir.instructions[0].accounts.len(), 1);
}

#[test]
fn rejects_manifests_without_a_package_name() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	write_program(
		dir.path(),
		"[package]\nversion = \"0.0.0\"\n",
		&valid_program("", ""),
	);

	let error = expect_idl_error(dir.path(), "package name must be required");
	assert!(error.to_string().contains("package name"));
}

#[test]
fn rejects_programs_without_an_entrypoint_dispatch() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	let source = format!(
		r#"
		declare_id!("{PROGRAM_ID}");
		#[discriminator]
		pub enum ExampleInstruction {{ Run = 0 }}
		#[instruction(discriminator = ExampleInstruction, variant = Run)]
		pub struct RunInstruction {{}}
	"#,
	);
	write_program(dir.path(), manifest(), &source);

	let error = expect_idl_error(dir.path(), "missing dispatch must fail closed");
	assert!(matches!(error, IdlError::NoEntrypoint));
}

#[test]
fn rejects_multiple_entrypoint_dispatch_sources() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	write_program(dir.path(), manifest(), &valid_program("mod duplicate;", ""));
	fs::write(
		dir.path().join("src/duplicate.rs"),
		r#"
		pub fn process_instruction(
			program_id: &Address,
			accounts: &mut [AccountView],
			data: &[u8],
		) -> ProgramResult {
			let instruction: ExampleInstruction = parse_instruction(program_id, &ID, data)?;
			match instruction {
				ExampleInstruction::Run => RunAccounts::try_from(accounts)?.process(data),
			}
		}
	"#,
	)
	.unwrap_or_else(|error| panic!("write duplicate module: {error}"));

	let error = expect_idl_error(dir.path(), "ambiguous dispatch must fail closed");
	assert!(error.to_string().contains("multiple entrypoint dispatch"));
}

#[test]
fn rejects_malformed_pda_attributes() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	let prefix = r#"
		#[discriminator]
		pub enum ExampleAccount { Vault = 0 }
		#[account(discriminator = ExampleAccount, variant = Vault)]
		#[pda(seeds)]
		pub struct VaultState { pub bump: u8 }
	"#;
	write_program(dir.path(), manifest(), &valid_program(prefix, ""));

	let error = expect_idl_error(dir.path(), "malformed PDA must fail closed");
	assert!(error.to_string().contains("VaultState"));
	assert!(error.to_string().contains("pda"));
}

#[test]
fn rejects_pda_validation_without_a_resolved_definition() {
	let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
	write_program(
		dir.path(),
		manifest(),
		&valid_program("", "self.authority.assert_seeds(&[], &ID)?;"),
	);

	let error = expect_idl_error(dir.path(), "unresolved PDA link must fail closed");
	assert!(error.to_string().contains("authority"));
	assert!(error.to_string().contains("PDA"));
}
