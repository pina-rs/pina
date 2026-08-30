//! UI tests for the Pina lint catalog.
//!
//! Each fixture in `tests/ui/<lint>/` is compiled with the bundled
//! `pina_lint_driver` (the lints are statically linked into the driver), and
//! the emitted diagnostics are compared with the committed `.stderr` file.
//!
//! Supported fixture directives:
//!
//! - `// aux-build: <name>.rs` — compile `auxiliary/<name>.rs` first and pass
//!   it to the fixture through `--extern`.
//! - `// normalize-stderr-test: "<regex>" -> "<replacement>"` — rewrite the
//!   actual stderr before comparing it with the expectation.
//!
//! To update an expectation, run the test, copy the saved actual stderr over
//! the `.stderr` file, and re-run.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

use regex::Regex;

/// A stderr normalization rule parsed from a fixture.
struct Normalize {
	pattern: Regex,
	replacement: String,
}

/// Directives parsed from one fixture.
struct Directives {
	aux_builds: Vec<String>,
	normalize_stderr: Vec<Normalize>,
}

#[test]
fn ui_deny_heap_allocations_in_onchain_instruction_handlers() {
	run_ui_tests("deny_heap_allocations_in_onchain_instruction_handlers");
}

#[test]
fn ui_require_associated_token_address_before_ata_cast() {
	run_ui_tests("require_associated_token_address_before_ata_cast");
}

#[test]
fn ui_require_canonical_instruction_dispatch_for_idl() {
	run_ui_tests("require_canonical_instruction_dispatch_for_idl");
}

#[test]
fn ui_require_idl_root_to_define_one_program_id() {
	run_ui_tests("require_idl_root_to_define_one_program_id");
}

#[test]
fn ui_require_owner_before_token_cast() {
	run_ui_tests("require_owner_before_token_cast");
}

#[test]
fn ui_require_program_owned_before_lamport_mutation() {
	run_ui_tests("require_program_owned_before_lamport_mutation");
}

#[test]
fn ui_require_sysvar_assert_before_sysvar_use() {
	run_ui_tests("require_sysvar_assert_before_sysvar_use");
}

#[test]
fn ui_require_type_assert_before_zero_copy_cast() {
	run_ui_tests("require_type_assert_before_zero_copy_cast");
}

#[test]
fn ui_require_zeroed_before_close() {
	run_ui_tests("require_zeroed_before_close");
}

#[test]
fn ui_deny_account_borrows_across_cpi() {
	run_ui_tests("deny_account_borrows_across_cpi");
}

#[test]
fn ui_require_bounded_remaining_accounts() {
	run_ui_tests("require_bounded_remaining_accounts");
}

#[test]
fn ui_require_canonical_bump_before_pda_write() {
	run_ui_tests("require_canonical_bump_before_pda_write");
}

#[test]
fn ui_require_checked_asset_arithmetic() {
	run_ui_tests("require_checked_asset_arithmetic");
}

#[test]
fn ui_require_consistent_token_program() {
	run_ui_tests("require_consistent_token_program");
}

#[test]
fn ui_require_empty_before_init() {
	run_ui_tests("require_empty_before_init");
}

#[test]
fn ui_require_explicit_discriminators_and_seed_namespaces() {
	run_ui_tests("require_explicit_discriminators_and_seed_namespaces");
}

#[test]
fn ui_require_explicit_token_2022_extension_policy() {
	run_ui_tests("require_explicit_token_2022_extension_policy");
}

#[test]
fn ui_require_post_cpi_balance_reload() {
	run_ui_tests("require_post_cpi_balance_reload");
}

#[test]
fn ui_require_program_check_before_cpi() {
	run_ui_tests("require_program_check_before_cpi");
}

#[test]
fn ui_require_reason_for_duplicate_remaining_accounts() {
	run_ui_tests("require_reason_for_duplicate_remaining_accounts");
}

#[test]
fn ui_require_writable_before_account_resize() {
	run_ui_tests("require_writable_before_account_resize");
}

/// Compile every fixture of one lint and compare its stderr with the
/// committed expectation.
fn run_ui_tests(lint: &str) {
	let src_base = test_root().join(lint);
	let driver = driver_path();

	let mut fixtures = collect_fixtures(&src_base);
	assert!(
		!fixtures.is_empty(),
		"no fixtures found under {}",
		src_base.display()
	);
	fixtures.sort();

	for fixture in fixtures {
		let stem = fixture
			.file_stem()
			.expect("fixture has a file stem")
			.to_string_lossy()
			.to_string();
		let expected_path = fixture.with_extension("stderr");
		let expected = read_to_string(&expected_path);

		let directives = parse_directives(&fixture);
		let aux = build_auxiliaries(&driver, &src_base, &directives);

		let output = compile_fixture(&driver, &fixture, &aux, lint);

		let actual = normalize_stderr(
			String::from_utf8_lossy(&output.stderr).to_string(),
			&src_base,
			&directives,
		);

		if actual != expected {
			let actual_path = std::env::temp_dir()
				.join("pina-lints-ui")
				.join(lint)
				.join(format!("{stem}.stderr"));
			std::fs::create_dir_all(actual_path.parent().expect("parent directory"))
				.expect("could not create actual-stderr directory");
			std::fs::write(&actual_path, &actual).expect("could not save actual stderr");
			// Print the report before asserting so it survives harnesses that
			// cannot unwind test panics.
			println!(
				"stderr mismatch for {}\n--- expected ({})\n{expected}\n--- actual\n{actual}\n--- \
				 actual saved to {}",
				fixture.display(),
				expected_path.display(),
				actual_path.display(),
			);
			panic!("stderr mismatch for {}", fixture.display());
		}
	}
}

/// Return the directory holding the per-lint fixture directories.
fn test_root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("tests")
		.join("ui")
}

/// Return the path of the driver built alongside the tests.
fn driver_path() -> PathBuf {
	PathBuf::from(env!("CARGO_BIN_EXE_pina_lint_driver"))
}

/// Return the `.rs` fixture files directly inside `src_base`.
fn collect_fixtures(src_base: &Path) -> Vec<PathBuf> {
	let entries = std::fs::read_dir(src_base)
		.unwrap_or_else(|source| panic!("could not read {}: {source}", src_base.display()));
	entries
		.filter_map(|entry| {
			let path = entry.expect("directory entry").path();
			(path.extension().is_some_and(|extension| extension == "rs")).then_some(path)
		})
		.collect()
}

fn read_to_string(path: &Path) -> String {
	std::fs::read_to_string(path)
		.unwrap_or_else(|source| panic!("could not read {}: {source}", path.display()))
}

/// Parse the `aux-build` and `normalize-stderr-test` directives of a fixture.
fn parse_directives(fixture: &Path) -> Directives {
	let source = read_to_string(fixture);
	let mut directives = Directives {
		aux_builds: Vec::new(),
		normalize_stderr: Vec::new(),
	};

	for line in source.lines() {
		if let Some(name) = line.trim().strip_prefix("// aux-build: ") {
			directives.aux_builds.push(name.trim().to_owned());
		}
		if let Some(rule) = line.trim().strip_prefix("// normalize-stderr-test: ") {
			let (pattern, replacement) = rule
				.split_once(" -> ")
				.unwrap_or_else(|| panic!("invalid normalize-stderr-test rule: {rule}"));
			directives.normalize_stderr.push(Normalize {
				pattern: Regex::new(&unquote(pattern))
					.unwrap_or_else(|source| panic!("invalid normalization pattern: {source}")),
				replacement: unquote(replacement),
			});
		}
	}
	directives
}

/// Strip the surrounding quotes of a directive argument.
fn unquote(value: &str) -> String {
	value.trim().trim_matches('"').to_owned()
}

/// Build the auxiliary crates requested by a fixture and return the
/// `--extern` arguments referencing them.
fn build_auxiliaries(driver: &Path, src_base: &Path, directives: &Directives) -> Vec<String> {
	let mut extern_args = Vec::new();

	for name in &directives.aux_builds {
		let source = src_base.join("auxiliary").join(name);
		let crate_name = Path::new(name)
			.file_stem()
			.expect("auxiliary has a file stem")
			.to_string_lossy()
			.to_string();
		let output_dir = std::env::temp_dir()
			.join("pina-lints-ui")
			.join(src_base.file_name().expect("lint dir has a name"))
			.join("auxiliary");
		std::fs::create_dir_all(&output_dir)
			.unwrap_or_else(|source| panic!("could not create {}: {source}", output_dir.display()));

		// The auxiliary source decides its own crate type through
		// `#![crate_type]` (for example proc-macro fixtures); only fall back to
		// a plain library for sources without an inner attribute.
		let aux_source = read_to_string(&source);
		let has_crate_type = aux_source
			.lines()
			.any(|line| line.trim().starts_with("#![crate_type"));
		let compile_flags = aux_source
			.lines()
			.filter_map(|line| line.trim().strip_prefix("// compile-flags: "))
			.collect::<Vec<_>>()
			.join(" ");

		let mut command = Command::new(driver);
		command
			.arg("--edition=2024")
			.arg("--crate-name")
			.arg(&crate_name);
		if !has_crate_type {
			command.arg("--crate-type=lib");
		}
		if !compile_flags.is_empty() {
			for flag in compile_flags.split_whitespace() {
				command.arg(flag);
			}
		}
		command
			.arg("--cap-lints")
			.arg("allow")
			.arg("--out-dir")
			.arg(&output_dir)
			.arg(&source);
		let status = command
			.status()
			.unwrap_or_else(|source| panic!("could not run driver: {source}"));
		assert!(status.success(), "auxiliary crate {name} failed to compile");

		let artifact = find_artifact(&output_dir, &crate_name);
		extern_args.push("--extern".to_owned());
		extern_args.push(format!("{}={}", crate_name, artifact.display()));
	}
	extern_args
}

/// Locate the compiled artifact of an auxiliary crate inside `output_dir`.
///
/// The driver is invoked through plain `rustc` arguments, so artifacts keep
/// their unhashed names: `lib{crate_name}.rlib` for libraries and
/// `lib{crate_name}.{dylib,so}` for proc-macro crates.
fn find_artifact(output_dir: &Path, crate_name: &str) -> PathBuf {
	for file_name in [
		format!("lib{crate_name}.rlib"),
		format!("lib{crate_name}.dylib"),
		format!("lib{crate_name}.so"),
	] {
		let candidate = output_dir.join(&file_name);
		if candidate.is_file() {
			return candidate;
		}
	}
	panic!(
		"could not find compiled auxiliary crate {crate_name} in {}",
		output_dir.display()
	)
}

/// Compile one fixture with the driver and return its output.
///
/// `PINA_LINT_ONLY` restricts the driver to the lint under test so fixtures
/// observe the same single-lint behavior the Dylint libraries had.
fn compile_fixture(driver: &Path, fixture: &Path, extern_args: &[String], lint: &str) -> Output {
	Command::new(driver)
		.arg("--edition=2024")
		.arg("--crate-type=lib")
		.arg("--emit=metadata")
		.arg("-Zui-testing")
		.env("PINA_LINT_ONLY", lint)
		.args(extern_args)
		.arg(fixture)
		.output()
		.unwrap_or_else(|source| panic!("could not run driver: {source}"))
}

/// Apply the fixture's stderr normalization rules.
///
/// Paths under `src_base` are replaced with `$DIR` first, mirroring the
/// convention of the Rust repository's UI tests.
fn normalize_stderr(mut actual: String, src_base: &Path, directives: &Directives) -> String {
	let src_base = src_base.to_string_lossy().to_string();
	if !src_base.is_empty() {
		actual = actual.replace(&src_base, "$DIR");
	}
	for rule in &directives.normalize_stderr {
		actual = rule
			.pattern
			.replace_all(&actual, rule.replacement.as_str())
			.to_string();
	}
	actual
}
