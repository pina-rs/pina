use std::fs;
use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::generate;
use comfy_table::Table;
use owo_colors::OwoColorize;

use crate::cli::Cli;
use crate::cli::ClientArg;
use crate::cli::CodamaCommands;
use crate::cli::Commands;
use crate::cli::ExportEncodingArg;
use crate::cli::KeysCommands;
use crate::cli::MigrationCommands;
use crate::cli::ProfileCommands;
use crate::cli::SurfpoolCluster;
use crate::cli::VerifyCommands;
use crate::idl_command;

pub(crate) fn run(cli: Cli) {
	match cli.command {
		Commands::Build {
			project,
			features,
			no_default_features,
			verify,
			solana_verify,
		} => {
			run_build(
				project,
				features,
				no_default_features,
				verify,
				solana_verify,
			);
		}
		Commands::Lint { project, fix } => run_lint(project, fix),
		Commands::Migrations { command } => run_migrations(command),
		Commands::Generate {
			project,
			clients,
			output,
			mode,
			no_scaffold,
			npx,
		} => run_generate(project, clients, output, mode, no_scaffold, npx),
		Commands::Cpi {
			idl,
			stdin,
			output,
			mode,
			no_scaffold,
			npx,
		} => run_cpi(idl.as_deref(), stdin, &output, mode, no_scaffold, &npx),
		Commands::Idl { command, generate } => idl_command::run_idl_command(command, &generate),
		Commands::Docs { topic } => run_docs(topic.as_deref()),
		Commands::Init { name, path, force } => run_init(name.as_str(), path.as_deref(), force),
		Commands::Keys {
			path,
			keypair,
			json,
			command,
		} => run_keys(&path, keypair.as_deref(), json, command.as_ref()),
		Commands::Doctor { path, json } => run_doctor(&path, json),
		Commands::Completions { shell } => {
			generate(shell, &mut Cli::command(), "pina", &mut std::io::stdout());
		}
		Commands::Test {
			project,
			unit,
			compatibility,
			filter,
		} => run_test(project, unit, compatibility, filter),
		Commands::Dev {
			project,
			network,
			rpc_url,
			yes,
		} => run_dev(project, network, rpc_url, yes),
		Commands::Profile {
			path,
			project,
			json,
			output,
			command,
		} => {
			match command {
				None => run_profile(path.as_deref(), &project, json, output.as_deref()),
				Some(ProfileCommands::Compare {
					baseline,
					path,
					project,
					json,
					fail_cu,
					fail_percent,
				}) => {
					run_profile_compare(
						baseline.as_path(),
						path.as_deref(),
						&project,
						json,
						pina_profile::RegressionThreshold {
							delta_cu: fail_cu,
							delta_percent: fail_percent,
						},
					);
				}
			}
		}
		Commands::Verify {
			command,
			solana_verify,
		} => run_verify(command, solana_verify),
		Commands::Deploy {
			project,
			program,
			build,
			program_keypair,
			upgrade_authority,
			payer,
			cluster,
			dry_run,
			json,
			yes,
			allow_mainnet,
			record_publication,
			remote_command,
		} => {
			run_deploy(
				project,
				program,
				build,
				program_keypair,
				upgrade_authority,
				payer,
				&cluster,
				dry_run,
				json,
				yes,
				allow_mainnet,
				record_publication,
				remote_command,
			);
		}
		Commands::Codama { command } => {
			match command {
				CodamaCommands::Generate {
					examples_dir,
					idls_dir,
					rust_out,
					cpi_out,
					js_out,
					dart_out,
					cli_rust_out,
					cli_ts_out,
					cli_dart_out,
					examples,
					npx,
				} => {
					run_codama_generate(&pina_cli::CodamaGenerateOptions {
						examples_dir,
						idls_dir,
						rust_out,
						cpi_out,
						js_out,
						dart_out,
						cli_rust_out,
						cli_ts_out,
						cli_dart_out,
						examples,
						npx,
					});
				}
			}
		}
	}
}

fn run_migrations(command: MigrationCommands) {
	match command {
		MigrationCommands::Sync {
			project,
			renames,
			assume_removed,
			no_interactive,
			json,
		} => {
			// The one-command loop: make -> build -> generate. Each stage
			// keeps its own gate; make fails with the same question payloads
			// as the standalone command.
			let answers = match build_migration_answers(
				&project,
				&renames,
				&assume_removed,
				no_interactive,
			) {
				Ok(answers) => answers,
				Err(reason) => {
					if json {
						print_json(&pina_cli::migrations::JsonErrorEnvelope {
							error: reason.clone(),
							questions: None,
						});
					}
					eprintln!("{} {reason}", "Error".red().bold());
					std::process::exit(1);
				}
			};
			let output =
				match pina_cli::migrations::make_migrations_with_answers(&project, &answers) {
					Ok(output) => output,
					Err(error) => {
						if json {
							let questions = match &error {
								pina_cli::migrations::MigrationError::DisambiguationRequired {
									questions,
								} => Some(questions.as_slice()),
								_ => None,
							};
							print_json(&pina_cli::migrations::JsonErrorEnvelope {
								error: error.to_string(),
								questions,
							});
						}
						eprintln!("{} {error}", "Error".red().bold());
						std::process::exit(1);
					}
				};
			for warning in &output.data_warnings {
				println!("{} {warning}", "⚠".yellow().bold());
			}
			run_build(
				project.clone(),
				Vec::new(),
				false,
				false,
				std::ffi::OsString::from("solana"),
			);
			run_generate(project, Vec::new(), None, None, false, "node".to_owned());
			if json {
				print_json(&output);
			}
			println!("{} Sync complete", "✔".green());
		}
		MigrationCommands::Inspect {
			address,
			project,
			url,
			json,
		} => {
			let report = pina_cli::migrations::inspect::run_inspect(&project, &address, &url);
			match report {
				Ok((report, exit)) => {
					if json {
						print_json(&report);
					} else {
						print_inspect_report(&report);
					}
					if exit.code != 0 {
						std::process::exit(exit.code);
					}
				}
				Err(error) => {
					if json {
						print_json(&pina_cli::migrations::JsonErrorEnvelope {
							error: error.to_string(),
							questions: None,
						});
					}
					eprintln!("{} {error}", "Error".red().bold());
					std::process::exit(2);
				}
			}
		}
		MigrationCommands::Make {
			project,
			renames,
			assume_removed,
			no_interactive,
			json,
		} => {
			let answers = match build_migration_answers(
				&project,
				&renames,
				&assume_removed,
				no_interactive,
			) {
				Ok(answers) => answers,
				Err(reason) => {
					if json {
						print_json(&pina_cli::migrations::JsonErrorEnvelope {
							error: reason.clone(),
							questions: None,
						});
					}
					eprintln!("{} {reason}", "Error".red().bold());
					std::process::exit(1);
				}
			};
			let output =
				match pina_cli::migrations::make_migrations_with_answers(&project, &answers) {
					Ok(output) => output,
					Err(error) => {
						if json {
							let questions = match &error {
								pina_cli::migrations::MigrationError::DisambiguationRequired {
									questions,
								} => Some(questions.as_slice()),
								_ => None,
							};
							print_json(&pina_cli::migrations::JsonErrorEnvelope {
								error: error.to_string(),
								questions,
							});
						}
						eprintln!("{} {error}", "Error".red().bold());
						std::process::exit(1);
					}
				};
			if json {
				print_json(&output);
				return;
			}
			println!("{} Updated {}", "✔".green(), escaped_path(&output.manifest));
			for contract in output.created_contracts {
				println!("Created {contract}@0");
			}
			for contract in output.advanced_versions {
				println!("Advanced {contract}");
			}
			for contract in output.updated_drafts {
				println!("Updated draft {contract}");
			}
			for path in output.manual_transitions {
				println!("Manual migration required: {}", escaped_path(&path));
			}
			for warning in output.data_warnings {
				println!("{} {warning}", "⚠".yellow().bold());
			}
		}
		MigrationCommands::Check { project, json }
		| MigrationCommands::Status { project, json } => {
			let statuses = unwrap_or_exit(pina_cli::migrations::migration_status(&project));
			if json {
				print_json(&statuses);
				return;
			}
			if statuses.is_empty() {
				println!("No migration-aware contracts.");
				return;
			}
			for status in statuses {
				let publication = if status.publication_pending {
					"publication pending"
				} else if status.published {
					"published"
				} else {
					"draft"
				};
				println!(
					"{} {} v{} ({publication})",
					status.kind, status.rust_name, status.current_version
				);
			}
			println!("{} Migration history is consistent", "✔".green());
		}
		MigrationCommands::Reconcile {
			project,
			abandon,
			json,
		} => {
			let output = unwrap_or_exit(pina_cli::migrations::reconcile_publication(
				&project, abandon,
			));
			if json {
				print_json(&output);
				return;
			}
			if output.no_pending {
				println!("No pending deployment.");
				return;
			}
			if output.abandoned {
				println!(
					"{} Pending deployment recorded as abandoned. Its versions stay frozen.",
					"✔".green()
				);
				return;
			}
			let cluster = output.cluster.as_deref().unwrap_or("unknown");
			let rpc_url = output.rpc_url.as_deref().unwrap_or("unknown");
			let program_id = output.program_id.as_deref().unwrap_or("unknown");
			let digest = output.executable_sha256.as_deref().unwrap_or("unknown");
			println!("Pending deployment for {program_id} on {cluster} ({rpc_url})");
			println!("Planned executable digest: {digest}");
			println!(
				"Rerun the exact same {cluster} deployment to reconcile it, or pass --abandon \
				 once you are certain it never went live."
			);
		}
	}
}

fn run_lint(project: PathBuf, fix: bool) {
	let output = unwrap_or_exit(pina_cli::lint::lint_project(&pina_cli::lint::LintOptions {
		project,
		fix,
	}));

	if output.fix {
		println!(
			"{} Applied available Pina security lint fixes for {}",
			"✔".green(),
			escaped_text(&output.package_name)
		);
	} else {
		println!(
			"{} Pina security lints passed for {}",
			"✔".green(),
			escaped_text(&output.package_name)
		);
	}
}

fn run_keys(path: &Path, keypair: Option<&Path>, json: bool, command: Option<&KeysCommands>) {
	match command {
		None | Some(KeysCommands::Show) => {
			let inspection = unwrap_or_exit(pina_cli::keys::inspect_keys(path, keypair));

			if json {
				print_json(&inspection);
				return;
			}

			println!(
				"Program: {}",
				escaped_text(&inspection.project.package_name)
			);
			println!(
				"Source: {}",
				escaped_path(&inspection.project.library_source)
			);
			println!("Declared program ID: {}", inspection.declared_program_id);
			println!("Keypair: {}", escaped_path(&inspection.keypair));

			match (&inspection.keypair_program_id, inspection.matches) {
				(Some(program_id), Some(true)) => {
					println!("Keypair program ID: {program_id}");
					println!("Status: source and keypair match");
				}
				(Some(program_id), Some(false)) => {
					println!("Keypair program ID: {program_id}");
					println!("Status: mismatch; review and run `pina keys sync`");
				}
				_ => println!("Status: keypair not found; source was not changed"),
			}
		}
		Some(KeysCommands::Sync) => {
			let sync = unwrap_or_exit(pina_cli::keys::sync_keys(path, keypair));

			if json {
				print_json(&sync);
				return;
			}

			if sync.changed {
				println!("{} Updated {}", "✔".green(), escaped_path(&sync.source));
				println!("Previous program ID: {}", sync.previous_program_id);
				println!("Program ID: {}", sync.program_id);
			} else {
				println!("Program ID already matches {}", sync.program_id);
			}
		}
		Some(KeysCommands::New { force }) => {
			let generation = unwrap_or_exit(pina_cli::keys::generate_keys(path, keypair, *force));

			if json {
				print_json(&generation);
				return;
			}

			println!(
				"{} Created {}",
				"✔".green(),
				escaped_path(&generation.keypair)
			);
			println!("Updated: {}", escaped_path(&generation.source));
			println!("Program ID: {}", generation.program_id);
		}
	}
}

fn run_doctor(path: &Path, json: bool) {
	let report = pina_cli::doctor::diagnose(path);

	if json {
		print_json(&report);
	} else {
		print!("{}", report.render_text());
	}

	if !report.is_usable() {
		std::process::exit(1);
	}
}

fn print_json(value: &impl serde::Serialize) {
	let json = unwrap_or_exit(serde_json::to_string_pretty(value));
	println!("{json}");
}

/// Layer CLI disambiguation flags over the persisted `[migrations.answers]`
/// table discovered with the project.
fn build_migration_answers(
	project_path: &Path,
	renames: &[String],
	removed: &[String],
	no_interactive: bool,
) -> Result<pina_cli::migrations::MigrationAnswers, String> {
	let project =
		pina_cli::project::Project::discover(project_path).map_err(|error| error.to_string())?;
	pina_cli::migrations::MigrationAnswers::from_layers(
		&project.migration_answers,
		renames,
		removed,
		no_interactive,
	)
}

fn unwrap_or_exit<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
	match result {
		Ok(value) => value,
		Err(error) => {
			eprintln!(
				"{} {}",
				"Error".red().bold(),
				escaped_text(&error.to_string())
			);
			std::process::exit(1);
		}
	}
}

fn run_verify(command: VerifyCommands, solana_verify: std::ffi::OsString) {
	let executor = pina_cli::verification::SystemVerifyExecutor::new(solana_verify);

	match command {
		VerifyCommands::Check {
			program_id,
			cluster,
			program,
			project,
		} => run_verify_check(&executor, program_id, &cluster, program, project),
		VerifyCommands::Record {
			program_id,
			cluster,
			build_record,
			authority,
			export,
			output,
			export_encoding,
			yes,
			acknowledge_mainnet,
		} => {
			run_verify_record(
				&executor,
				program_id,
				&cluster,
				build_record,
				authority,
				export,
				output,
				export_encoding,
				yes,
				acknowledge_mainnet,
			);
		}
		VerifyCommands::Submit {
			program_id,
			uploader,
		} => {
			let output = pina_cli::verification::submit_program(&executor, &program_id, &uploader)
				.unwrap_or_else(|error| exit_verify_error(error));
			write_process_output(&output, true);
		}
		VerifyCommands::Status { program_id } => {
			let output = pina_cli::verification::status_program(&executor, &program_id)
				.unwrap_or_else(|error| exit_verify_error(error));
			write_process_output(&output, true);
		}
	}
}

fn run_verify_check(
	executor: &pina_cli::verification::SystemVerifyExecutor,
	program_id: String,
	cluster: &str,
	program: Option<PathBuf>,
	project: PathBuf,
) {
	let cluster = cluster
		.parse::<pina_cli::verification::Cluster>()
		.unwrap_or_else(|error| exit_verify_error(error));
	let result = pina_cli::verification::check_program(
		executor,
		&pina_cli::verification::CheckOptions {
			program_id,
			cluster,
			program,
			project_dir: project,
		},
	)
	.unwrap_or_else(|error| exit_verify_error(error));

	match result {
		pina_cli::verification::CheckResult::Match { hash } => {
			println!("{} Program executable matches", "✔".green());
			println!("  Hash {hash}");
		}
		pina_cli::verification::CheckResult::Mismatch { local, deployed } => {
			eprintln!("{} Program executables differ", "Error".red().bold());
			eprintln!("  Local    {local}");
			eprintln!("  Deployed {deployed}");
			std::process::exit(2);
		}
	}
}

#[allow(clippy::too_many_arguments)]
fn run_verify_record(
	executor: &pina_cli::verification::SystemVerifyExecutor,
	program_id: String,
	cluster: &str,
	build_record: PathBuf,
	authority: Option<PathBuf>,
	export: Option<String>,
	output: Option<PathBuf>,
	export_encoding: Option<ExportEncodingArg>,
	yes: bool,
	acknowledge_mainnet: bool,
) {
	let cluster = cluster
		.parse::<pina_cli::verification::Cluster>()
		.unwrap_or_else(|error| exit_verify_error(error));
	let is_export = export.is_some();
	let export_encoding = match export_encoding.unwrap_or(ExportEncodingArg::Base58) {
		ExportEncodingArg::Base64 => pina_cli::verification::ExportEncoding::Base64,
		ExportEncodingArg::Base58 => pina_cli::verification::ExportEncoding::Base58,
	};
	let options = pina_cli::verification::RecordOptions {
		program_id,
		cluster,
		build_record,
		authority,
		export_authority: export,
		export_output: output.clone(),
		export_encoding,
		confirmed: is_export || yes,
		mainnet_acknowledged: acknowledge_mainnet,
	};

	let mut input = std::io::stdin().lock();
	let mut diagnostics = std::io::stderr().lock();
	let plan = prepare_and_confirm_record(
		&options,
		std::io::stdin().is_terminal(),
		&mut input,
		&mut diagnostics,
	)
	.unwrap_or_else(|error| exit_verify_error(error));
	drop(input);
	drop(diagnostics);

	let result = pina_cli::verification::execute_record(executor, &plan)
		.unwrap_or_else(|error| exit_verify_error(error));

	if is_export {
		let _ = std::io::stderr().lock().write_all(&result.stdout);
		write_process_output(&result, false);
		if let Some(path) = output {
			println!(
				"{} Exported verification transaction to {}",
				"✔".green(),
				escaped_path(&path)
			);
		}
	} else {
		write_process_output(&result, true);
	}
}

fn prepare_and_confirm_record(
	options: &pina_cli::verification::RecordOptions,
	is_terminal: bool,
	input: &mut impl std::io::BufRead,
	diagnostics: &mut impl Write,
) -> Result<pina_cli::verification::RecordPlan, pina_cli::verification::VerifyError> {
	if !options.confirmed && !is_terminal {
		return Err(pina_cli::verification::VerifyError::ConfirmationRequired);
	}

	let mut plan = pina_cli::verification::prepare_record(options)?;

	if !options.confirmed {
		let review = plan.review();
		let _ = writeln!(diagnostics, "Verification record plan:");
		let _ = writeln!(
			diagnostics,
			"  Build record {}",
			escaped_path(&review.build_record)
		);
		let _ = writeln!(diagnostics, "  Program      {}", review.program_id);
		let _ = writeln!(diagnostics, "  Cluster      {}", review.cluster);
		let _ = writeln!(diagnostics, "  Repository   {}", review.repository);
		let _ = writeln!(diagnostics, "  Revision     {}", review.revision);
		let _ = writeln!(
			diagnostics,
			"  Mount        {}",
			escaped_text(&review.mount_path)
		);
		let _ = writeln!(
			diagnostics,
			"  Workspace    {}",
			escaped_text(&review.workspace_path)
		);
		let _ = writeln!(diagnostics, "  Library      {}", review.library_name);
		let features = display_features(&review.features);
		let _ = writeln!(diagnostics, "  Features     {features}");
		let _ = writeln!(
			diagnostics,
			"  Defaults     {}",
			if review.default_features {
				"enabled"
			} else {
				"disabled"
			}
		);
		let _ = writeln!(diagnostics, "  Hash         {}", review.executable_hash);
		let _ = writeln!(diagnostics, "  Authority    {}", review.authority);
		let _ = writeln!(
			diagnostics,
			"  Keypair      {}",
			escaped_path(&review.authority_path)
		);
		let _ = write!(diagnostics, "Type `record` to submit this transaction: ");
		let _ = diagnostics.flush();
		let mut answer = String::new();

		if !input
			.read_line(&mut answer)
			.is_ok_and(|_| answer.trim() == "record")
		{
			return Err(pina_cli::verification::VerifyError::ConfirmationRequired);
		}
		plan.confirm();
	}

	Ok(plan)
}

#[allow(clippy::unnecessary_debug_formatting)]
fn escaped_path(path: &Path) -> String {
	format!("{path:?}")
}

fn escaped_text(value: &str) -> String {
	value.chars().flat_map(char::escape_default).collect()
}

fn display_features(features: &[String]) -> String {
	if features.is_empty() {
		"(none)".to_owned()
	} else {
		features.join(",")
	}
}

fn write_process_output(output: &pina_cli::verification::ProcessOutput, include_stdout: bool) {
	if include_stdout {
		let _ = std::io::stdout().lock().write_all(&output.stdout);
	}

	let _ = std::io::stderr().lock().write_all(&output.stderr);
}

#[allow(clippy::needless_pass_by_value)]
fn exit_verify_error(error: pina_cli::verification::VerifyError) -> ! {
	eprintln!("{} {}", "Error".red().bold(), error);
	std::process::exit(error.exit_code());
}

fn run_build(
	project: PathBuf,
	features: Vec<String>,
	no_default_features: bool,
	verify: bool,
	solana_verify: std::ffi::OsString,
) {
	let options = pina_cli::build::BuildOptions {
		project_dir: project,
		features,
		no_default_features,
	};
	let output = if verify {
		pina_cli::build::build_project_verified_with_options(
			&options,
			&pina_cli::build::VerifyBuildOptions {
				executable: solana_verify,
			},
		)
		.map(|verified| {
			(
				verified.build,
				Some((verified.verifiable_artifact, verified.verification_manifest)),
			)
		})
	} else {
		pina_cli::build::build_project_with_options(&options).map(|build| (build, None))
	};
	let (output, verification_manifest) = match output {
		Ok(output) => output,
		Err(error) => {
			eprintln!("{} {}", "Error".red().bold(), error);
			std::process::exit(1);
		}
	};
	println!("{} Built {}", "✔".green(), output.package_name);
	println!("  SBF  {}", output.sbf_artifact.display());
	println!("  IDL  {}", output.idl.display());
	if let Some((artifact, manifest)) = verification_manifest {
		println!("  Verified SBF {}", artifact.display());
		println!("  Build record {}", manifest.display());
	}
}

fn run_generate(
	project: PathBuf,
	clients: Vec<ClientArg>,
	output: Option<PathBuf>,
	mode: Option<pina_cli::GenerationMode>,
	no_scaffold: bool,
	npx: String,
) {
	let clients = clients
		.into_iter()
		.map(|client| {
			match client {
				ClientArg::Cpi => pina_cli::project::ClientLanguage::Cpi,
				ClientArg::Rust => pina_cli::project::ClientLanguage::Rust,
				ClientArg::Typescript => pina_cli::project::ClientLanguage::Typescript,
				ClientArg::Dart => pina_cli::project::ClientLanguage::Dart,
				ClientArg::CliRust => pina_cli::project::ClientLanguage::CliRust,
				ClientArg::CliTs => pina_cli::project::ClientLanguage::CliTs,
				ClientArg::CliDart => pina_cli::project::ClientLanguage::CliDart,
			}
		})
		.collect();
	let options = pina_cli::ProjectGenerateOptions {
		project_dir: project,
		clients,
		output,
		mode,
		scaffold: no_scaffold.then_some(false),
		npx,
	};
	let generated = match pina_cli::generate_project_clients(&options) {
		Ok(generated) => generated,
		Err(error) => {
			eprintln!("{} {}", "Error".red().bold(), error);
			std::process::exit(1);
		}
	};
	let clients = generated
		.clients
		.iter()
		.map(|client| client.as_str())
		.collect::<Vec<_>>()
		.join(", ");

	println!(
		"{} Generated {} client(s) for {}: {}",
		"✔".green(),
		generated.clients.len(),
		generated.package_name,
		clients
	);
	println!("  IDL     {}", generated.idl.display());
	println!("  Clients {}", generated.clients_dir.display());
}

fn run_cpi(
	idl: Option<&Path>,
	stdin: bool,
	output: &Path,
	mode: pina_cli::GenerationMode,
	no_scaffold: bool,
	npx: &str,
) {
	let result = if stdin {
		pina_cli::generate_cpi_crate_from_reader_with_config(
			std::io::stdin().lock(),
			output,
			mode,
			!no_scaffold,
		)
	} else {
		let idl = idl.unwrap_or_else(|| unreachable!("clap requires --idl or --stdin"));
		pina_cli::generate_cpi_crate(&pina_cli::CpiGenerateOptions {
			idl: idl.to_path_buf(),
			output: output.to_path_buf(),
			mode,
			scaffold: !no_scaffold,
			npx: npx.to_string(),
		})
	};

	unwrap_or_exit(result);
	println!(
		"{} Generated CPI crate at {}",
		"✔".green(),
		output.display()
	);
}

fn run_test(project: PathBuf, unit: bool, compatibility: bool, filter: Option<String>) {
	let options = pina_cli::workflow::TestOptions {
		project,
		unit,
		compatibility,
		filter,
	};

	if let Err(error) = pina_cli::workflow::test_project(&options) {
		eprintln!("{} {}", "Error".red().bold(), error);
		std::process::exit(error.exit_code());
	}
}

fn run_dev(
	project: PathBuf,
	network: Option<SurfpoolCluster>,
	rpc_url: Option<String>,
	accept_runbook_changes: bool,
) {
	let network = match (network, rpc_url) {
		(_, Some(rpc_url)) => pina_cli::workflow::SurfpoolNetwork::RpcUrl(rpc_url),
		(Some(network), None) => {
			pina_cli::workflow::SurfpoolNetwork::Cluster(network.as_str().to_owned())
		}
		(None, None) => pina_cli::workflow::SurfpoolNetwork::Offline,
	};
	let options = pina_cli::workflow::DevOptions {
		project,
		network,
		accept_runbook_changes,
	};

	if let Err(error) = pina_cli::workflow::dev_project(&options) {
		eprintln!("{} {}", "Error".red().bold(), error);
		std::process::exit(error.exit_code());
	}
}

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn run_deploy(
	project: PathBuf,
	program: Option<PathBuf>,
	build: bool,
	program_keypair: Option<PathBuf>,
	upgrade_authority: PathBuf,
	payer: PathBuf,
	cluster: &str,
	dry_run: bool,
	json: bool,
	yes: bool,
	allow_mainnet: bool,
	record_publication: bool,
	remote_command: Option<String>,
) {
	let target = pina_cli::deploy::DeploymentTarget::from_cluster_arg(cluster);
	let request = pina_cli::deploy::DeploymentRequest {
		project,
		remote_command,
		program,
		program_keypair,
		upgrade_authority,
		payer,
		target,
	};
	let mut runner = pina_cli::deploy::SystemCommandRunner;
	if let Err(error) = pina_cli::deploy::validate_deployment_target(&request.target) {
		eprintln!("{} {}", "Error".red().bold(), error);
		std::process::exit(1);
	}

	if build && let Err(error) = pina_cli::deploy::build_deployment_program(&request.project) {
		eprintln!("{} {}", "Error".red().bold(), error);
		std::process::exit(1);
	}

	let plan = match pina_cli::deploy::prepare_deployment(&request) {
		Ok(plan) => plan,
		Err(error) => {
			eprintln!("{} {}", "Error".red().bold(), error);
			std::process::exit(1);
		}
	};

	if json {
		#[rustfmt::skip]
		let output = serde_json::to_string_pretty(&plan).unwrap_or_else(|error| panic!("deployment plans contain only JSON-compatible values: {error}"));
		println!("{output}");
	} else {
		print!("{}", plan.render_text());
	}

	if dry_run {
		return;
	}
	let mut confirmer = StdinDeploymentConfirmer;
	let approved =
		match pina_cli::deploy::approve_deployment(&plan, yes, allow_mainnet, &mut confirmer) {
			Ok(approved) => approved,
			Err(error) => {
				eprintln!("{} {}", "Error".red().bold(), error);
				std::process::exit(1);
			}
		};
	unwrap_or_exit(plan.verify_inputs_unchanged());

	let pending_publication = if plan.is_local() && !record_publication {
		None
	} else {
		match pina_cli::migrations::begin_publication(
			Path::new(plan.project_root()),
			plan.cluster(),
			plan.rpc_url(),
			plan.program_id(),
			Path::new(plan.program()),
			plan.program_digest(),
		) {
			Ok(pending) => pending,
			Err(error) => {
				eprintln!(
					"{} Could not reserve migration publication: {}",
					"Error".red().bold(),
					error
				);
				std::process::exit(1);
			}
		}
	};

	if let Err(error) = approved.execute(&mut runner) {
		if pending_publication.is_some() {
			eprintln!(
				"{} The recoverable pending publication was retained; rerun this exact deployment \
				 to reconcile whether it became live.",
				"Warning".yellow().bold()
			);
		}
		eprintln!("{} {}", "Error".red().bold(), error);
		std::process::exit(1);
	}

	if pending_publication.is_some() {
		if let Err(error) = plan.verify_inputs_unchanged() {
			eprintln!(
				"{} Deployment succeeded, but its inputs changed before migration publication \
				 could be recorded: {}",
				"Error".red().bold(),
				error
			);
			std::process::exit(1);
		}
		unwrap_or_exit(
			pina_cli::migrations::record_publication(
				Path::new(plan.project_root()),
				plan.cluster(),
				plan.rpc_url(),
				plan.program_id(),
				Path::new(plan.program()),
				plan.program_digest(),
			)
			.map_err(publication_record_error),
		);
	}

	println!("{} Deployment complete", "✔".green());
}

fn publication_record_error(error: impl std::fmt::Display) -> String {
	format!("Deployment succeeded, but migration publication could not be recorded: {error}")
}

struct StdinDeploymentConfirmer;

impl pina_cli::deploy::DeploymentConfirmer for StdinDeploymentConfirmer {
	fn confirm(&mut self, prompt: &str) -> std::io::Result<bool> {
		let stdin = std::io::stdin();
		let stderr = std::io::stderr();
		let is_terminal = stdin.is_terminal();
		pina_cli::deploy::read_deployment_confirmation(
			prompt,
			is_terminal,
			&mut stdin.lock(),
			&mut stderr.lock(),
		)
	}
}

pub(crate) fn run_idl(path: &Path, output: Option<&Path>, name: Option<&str>, pretty: bool) {
	let root = match pina_cli::generate_idl(path, name) {
		Ok(r) => r,
		Err(e) => {
			eprintln!("{} {}", "Error".red().bold(), e);
			std::process::exit(1);
		}
	};

	let mut table = Table::new();
	table.load_style(comfy_table::presets::UTF8_FULL_CONDENSED);
	table.set_header(vec!["Component", "Count"]);
	table.add_row(vec![
		"Instructions",
		&root.program.instructions.len().to_string(),
	]);
	table.add_row(vec!["Accounts", &root.program.accounts.len().to_string()]);
	table.add_row(vec!["PDAs", &root.program.pdas.len().to_string()]);
	table.add_row(vec!["Errors", &root.program.errors.len().to_string()]);

	eprintln!("{}", "IDL generation complete".green().bold());
	eprintln!("{table}");

	let json = if pretty {
		serde_json::to_string_pretty(&root)
	} else {
		serde_json::to_string(&root)
	};

	// The IDL model contains no fallible map keys or custom serializers.
	let json = json.unwrap_or_else(|error| panic!("IDL serialization failed: {error}"));

	if let Some(output) = output {
		if let Err(e) = fs::write(output, &json) {
			eprintln!(
				"{} Failed to write {}: {}",
				"Error".red().bold(),
				output.display(),
				e
			);
			std::process::exit(1);
		}

		eprintln!("Wrote {}", output.display());

		return;
	}

	println!("{json}");
}

fn run_docs(topic: Option<&str>) {
	let Some(topic) = topic else {
		println!("Bundled documentation topics:");

		for (name, description) in BUNDLED_DOC_TOPICS {
			println!("  {name:<16} {description}");
		}

		println!("\nRun `pina docs <topic>` to open a topic.");
		println!("Set PINA_TEMPLATES_DIR to load additional `<topic>.t.md` files.");

		return;
	};

	let mut attempted_paths = Vec::new();

	if let Ok(template_dir) = std::env::var("PINA_TEMPLATES_DIR") {
		let template_path = PathBuf::from(template_dir).join(format!("{topic}.t.md"));
		attempted_paths.push(template_path.clone());

		if template_path.is_file() {
			let content = match fs::read_to_string(&template_path) {
				Ok(c) => c,
				Err(e) => {
					eprintln!(
						"{} Failed to read template {}: {}",
						"Error".red().bold(),
						template_path.display(),
						e
					);
					std::process::exit(1);
				}
			};
			render_docs(&content);
			return;
		}
	}

	if let Some(content) = bundled_docs(topic) {
		render_docs(content);
		return;
	}

	eprintln!("{} Topic `{}` not found.", "Error".red().bold(), topic);
	eprintln!("Bundled topics:");

	for (name, description) in BUNDLED_DOC_TOPICS {
		eprintln!("  {name:<16} {description}");
	}

	if !attempted_paths.is_empty() {
		eprintln!("Attempted template paths:");
		for path in attempted_paths {
			eprintln!("  - {}", path.display());
		}
	}

	eprintln!(
		"Set PINA_TEMPLATES_DIR to a directory containing `<topic>.t.md` to load custom docs."
	);
	std::process::exit(1);
}

const BUNDLED_DOC_TOPICS: &[(&str, &str)] = &[
	("pina-idl", "IDL extraction rules and supported Rust shapes"),
	(
		"pina-overview",
		"framework concepts, crates, features, and workflows",
	),
	(
		"pina-validation",
		"declarative rules, manual alternatives, and code generation",
	),
];

fn bundled_docs(topic: &str) -> Option<&'static str> {
	match topic {
		"pina-idl" => Some(include_str!("../templates/pina-idl.md")),
		"pina-overview" => Some(include_str!("../templates/pina-overview.md")),
		"pina-validation" => Some(include_str!("../templates/pina-validation.md")),
		_ => None,
	}
}

fn render_docs(content: &str) {
	let skin = termimad::MadSkin::default();
	skin.print_text(content);
}

fn run_init(name: &str, path: Option<&Path>, force: bool) {
	let project_path = path.map_or_else(|| PathBuf::from(name), PathBuf::from);

	if let Err(err) = pina_cli::init_project(&project_path, name, force) {
		eprintln!("{} {}", "Error".red().bold(), err);
		std::process::exit(1);
	}

	println!(
		"{} Initialized new Pina project at {}",
		"✔".green(),
		project_path.display()
	);
	pina_cli::print_next_steps(&project_path, name);
}

fn run_profile(explicit_path: Option<&Path>, project: &Path, json: bool, output: Option<&Path>) {
	let path = unwrap_or_exit(pina_cli::profile::resolve_profile_input(
		explicit_path,
		project,
	));
	let profile = match pina_profile::profile_program(&path) {
		Ok(p) => p,
		Err(e) => {
			eprintln!("{} {}", "Error".red().bold(), e);
			std::process::exit(1);
		}
	};

	let format = if json {
		pina_profile::OutputFormat::Json
	} else {
		pina_profile::OutputFormat::Text
	};

	if let Some(output_path) = output {
		unwrap_or_exit(pina_cli::profile::write_profile_output(
			&profile,
			format,
			&path,
			output_path,
		));

		return;
	}

	let mut stdout = std::io::stdout().lock();

	unwrap_or_exit(pina_profile::output::write_profile(
		&profile,
		format,
		&mut stdout,
	));
}

fn run_profile_compare(
	baseline_path: &Path,
	explicit_path: Option<&Path>,
	project: &Path,
	json: bool,
	threshold: pina_profile::RegressionThreshold,
) {
	if !threshold.delta_percent.is_finite()
		|| threshold.delta_percent < 0.0
		|| threshold.delta_cu == 0
	{
		eprintln!(
			"{} --fail-cu must be at least 1, and --fail-percent must be a finite number of at \
			 least 0",
			"Error".red().bold()
		);
		std::process::exit(1);
	}
	let path = unwrap_or_exit(pina_cli::profile::resolve_profile_input(
		explicit_path,
		project,
	));
	let current = match pina_profile::profile_program(&path) {
		Ok(p) => p,
		Err(e) => {
			eprintln!("{} {}", "Error".red().bold(), e);
			std::process::exit(1);
		}
	};
	let baseline = unwrap_or_exit(pina_profile::compare::load_baseline(baseline_path));
	let report = pina_profile::compare::compare_profiles(&baseline.profile, &current, threshold);

	let mut stdout = std::io::stdout().lock();

	if json {
		unwrap_or_exit(pina_profile::compare::write_comparison_json(
			&report,
			&mut stdout,
		));
	} else {
		print_comparison_text(&report);
	}

	if report.exceeds_threshold {
		std::process::exit(2);
	}
}

fn print_comparison_text(report: &pina_profile::ComparisonReport) {
	let totals = report.totals;

	if report.baseline_program_name != report.current_program_name {
		println!(
			"{} baseline program '{}' differs from current program '{}'",
			"Warning".yellow().bold(),
			escaped_text(&report.baseline_program_name),
			escaped_text(&report.current_program_name)
		);
	}

	println!(
		"Profile comparison for {}",
		escaped_text(&report.current_program_name)
	);
	println!(
		"  Total estimated CU: {} -> {} ({})",
		totals.baseline.total_cu,
		totals.current.total_cu,
		color_delta(totals.delta_cu, format_signed(totals.delta_cu, " CU"))
	);
	println!(
		"  Instructions: {} -> {} ({})",
		totals.baseline.total_instructions,
		totals.current.total_instructions,
		format_signed(totals.delta_instructions, "")
	);
	println!(
		"  Syscalls: {} -> {} ({})",
		totals.baseline.total_syscalls,
		totals.current.total_syscalls,
		format_signed(totals.delta_syscalls, "")
	);
	println!(
		"  Binary size: {} -> {} ({})",
		totals.baseline.binary_size,
		totals.current.binary_size,
		format_signed(totals.delta_binary_size, " bytes")
	);
	println!(
		"  Text section: {} -> {} ({})",
		totals.baseline.text_size,
		totals.current.text_size,
		format_signed(totals.delta_text_size, " bytes")
	);
	println!();

	let changed: Vec<&pina_profile::FunctionDelta> = report
		.functions
		.iter()
		.filter(|function| function.change != pina_profile::FunctionChange::Unchanged)
		.collect();

	if changed.is_empty() {
		println!("No function-level changes.");
	} else {
		println!("Function deltas (sorted by absolute CU change):");
		println!(
			"  {:<46} {:>8} {:>8} {:>9} {:>10}",
			"Function", "Base", "Current", "Delta", "Pct"
		);

		for function in changed {
			print_function_row(function);
		}
	}

	println!();
	print_comparison_status(report);
}

/// Colorize a signed delta: increases are red and decreases are green.
fn color_delta(delta: i64, text: String) -> String {
	match delta.cmp(&0) {
		std::cmp::Ordering::Greater => text.red().to_string(),
		std::cmp::Ordering::Less => text.green().to_string(),
		std::cmp::Ordering::Equal => text,
	}
}

fn format_signed(value: i64, suffix: &str) -> String {
	if value > 0 {
		format!("+{value}{suffix}")
	} else {
		format!("{value}{suffix}")
	}
}

fn print_function_row(function: &pina_profile::FunctionDelta) {
	let optional = |value: Option<u64>| value.map_or_else(|| "-".to_owned(), |cu| cu.to_string());
	let percent = match function.change {
		pina_profile::FunctionChange::Added => "new".to_owned(),
		pina_profile::FunctionChange::Removed => "gone".to_owned(),
		_ => format!("{:+.1}%", function.delta_percent),
	};

	println!(
		"  {:<46} {:>8} {:>8} {:>9} {:>10}",
		pina_profile::output::truncate_name(&function.name, 46),
		optional(function.baseline_cu),
		optional(function.current_cu),
		color_delta(function.delta_cu, format_signed(function.delta_cu, "")),
		percent
	);
}

fn print_comparison_status(report: &pina_profile::ComparisonReport) {
	let totals = report.totals;
	let threshold = report.threshold;
	let threshold_text = format!(
		">= {} CU and >= {:.1}%",
		threshold.delta_cu, threshold.delta_percent
	);

	match report.status {
		pina_profile::ComparisonStatus::Unchanged => {
			println!("{} No compute unit changes", "✔".green());
		}
		pina_profile::ComparisonStatus::Improved => {
			println!(
				"{} Total estimated CU improved by {} ({:+.1}%)",
				"✔".green(),
				-totals.delta_cu,
				totals.delta_percent
			);
		}
		pina_profile::ComparisonStatus::Regression => {
			println!(
				"{} Total estimated CU increased by {} ({:+.1}%), below the threshold \
				 ({threshold_text})",
				"⚠".yellow(),
				totals.delta_cu,
				totals.delta_percent
			);
		}
		pina_profile::ComparisonStatus::ThresholdRegression => {
			println!(
				"{} Total estimated CU regression {} ({:+.1}%) reaches the threshold \
				 ({threshold_text})",
				"✘".red().bold(),
				totals.delta_cu,
				totals.delta_percent
			);
		}
	}
}

fn run_codama_generate(options: &pina_cli::CodamaGenerateOptions) {
	let generated_examples = match pina_cli::generate_codama(options) {
		Ok(examples) => examples,
		Err(err) => {
			eprintln!("{} {}", "Error".red().bold(), err);
			std::process::exit(1);
		}
	};

	println!(
		"{} Generated Codama IDLs and Rust/CPI/JavaScript/Dart clients for {} example(s): {}",
		"✔".green(),
		generated_examples.len(),
		generated_examples.join(", "),
	);
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::io::Cursor;

	use ed25519_dalek::SigningKey;
	use sha2::Digest;
	use sha2::Sha256;
	use tempfile::TempDir;

	use super::display_features;
	use super::escaped_path;
	use super::escaped_text;
	use super::prepare_and_confirm_record;
	use super::publication_record_error;

	#[test]
	fn escapes_control_characters_in_confirmation_paths() {
		let rendered = escaped_path(std::path::Path::new("record\n\u{1b}[31m.json"));

		assert_eq!(rendered, "\"record\\n\\u{1b}[31m.json\"");
		assert!(!rendered.contains('\n'));
		assert!(!rendered.contains('\u{1b}'));
		assert_eq!(display_features(&[]), "(none)");
		assert_eq!(
			escaped_text("path with spaces\n\u{1b}"),
			"path with spaces\\n\\u{1b}"
		);
		assert_eq!(
			display_features(&["logs".to_owned(), "trace".to_owned()]),
			"logs,trace"
		);
		assert_eq!(
			publication_record_error("ledger became unreadable"),
			"Deployment succeeded, but migration publication could not be recorded: ledger became \
			 unreadable"
		);
	}

	fn record_options(temp: &TempDir, confirmed: bool) -> pina_cli::verification::RecordOptions {
		let artifact_bytes = vec![7_u8; 128];
		let hash = Sha256::digest(&artifact_bytes)
			.iter()
			.map(|byte| format!("{byte:02x}"))
			.collect::<String>();
		let record_dir = temp.path().join("record with spaces");
		fs::create_dir_all(&record_dir).unwrap();
		let record = record_dir.join(format!("fixture-{hash}.json"));
		let artifact = record.with_extension("so");
		let json = serde_json::json!({
			"schemaVersion": 1,
			"packageName": "fixture",
			"libraryName": "fixture",
			"executableHash": hash,
			"solanaVerifyVersion": "0.5.1",
			"build": {
				"mountPath": "mount with spaces\n\u{1b}[33m",
				"workspacePath": "workspace\n\u{1b}[34m",
				"programPath": "programs/fixture",
				"libraryName": "fixture",
				"features": ["bpf-entrypoint"],
				"defaultFeatures": true,
				"cargoLockSha256": "a".repeat(64),
			},
			"source": {
				"repository": "https://github.com/pina-rs/pina",
				"revision": "0123456789abcdef0123456789abcdef01234567",
				"dirty": false,
			},
			"diagnostics": [],
		});
		fs::write(&artifact, artifact_bytes).unwrap();
		fs::write(&record, serde_json::to_vec(&json).unwrap()).unwrap();

		let authority = temp.path().join("keypair with spaces.json");
		let signing_key = SigningKey::from_bytes(&[3_u8; 32]);
		let public = signing_key.verifying_key().to_bytes();
		let bytes = [signing_key.to_bytes().as_slice(), public.as_slice()].concat();
		fs::write(&authority, serde_json::to_vec(&bytes).unwrap()).unwrap();
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;

			fs::set_permissions(&authority, fs::Permissions::from_mode(0o600)).unwrap();
		}

		pina_cli::verification::RecordOptions {
			program_id: "11111111111111111111111111111111".to_owned(),
			cluster: "devnet".parse().unwrap(),
			build_record: record,
			authority: Some(authority),
			export_authority: None,
			export_output: None,
			export_encoding: pina_cli::verification::ExportEncoding::Base58,
			confirmed,
			mainnet_acknowledged: false,
		}
	}

	#[cfg(any(target_os = "linux", target_os = "macos"))]
	#[test]
	fn confirmation_prepares_exactly_the_plan_that_is_reviewed() {
		let temp = TempDir::new().unwrap();
		let options = record_options(&temp, false);
		let mut diagnostics = Vec::new();
		let error = prepare_and_confirm_record(
			&options,
			false,
			&mut Cursor::new(b"record\n"),
			&mut diagnostics,
		)
		.err()
		.unwrap();
		assert!(matches!(
			error,
			pina_cli::verification::VerifyError::ConfirmationRequired
		));
		assert!(diagnostics.is_empty());

		let error =
			prepare_and_confirm_record(&options, true, &mut Cursor::new(b"no\n"), &mut diagnostics)
				.err()
				.unwrap();
		assert!(matches!(
			error,
			pina_cli::verification::VerifyError::ConfirmationRequired
		));

		diagnostics.clear();
		let plan = prepare_and_confirm_record(
			&options,
			true,
			&mut Cursor::new(b"record\n"),
			&mut diagnostics,
		)
		.unwrap();
		let rendered = String::from_utf8_lossy(&diagnostics);
		assert!(rendered.contains("record with spaces"));
		assert!(rendered.contains("fixture-"));
		assert!(rendered.contains("keypair with spaces.json"));
		assert!(rendered.contains("Mount        mount with spaces\\n\\u{1b}[33m"));
		assert!(rendered.contains("Workspace    workspace\\n\\u{1b}[34m"));
		assert!(rendered.contains("Features     bpf-entrypoint"));
		assert!(rendered.contains("Defaults     enabled"));
		assert_eq!(plan.review().program_id, options.program_id);
		drop(plan);

		let mut json: serde_json::Value =
			serde_json::from_slice(&fs::read(&options.build_record).unwrap()).unwrap();
		json["build"]["defaultFeatures"] = serde_json::Value::Bool(false);
		fs::write(&options.build_record, serde_json::to_vec(&json).unwrap()).unwrap();
		diagnostics.clear();
		prepare_and_confirm_record(
			&options,
			true,
			&mut Cursor::new(b"record\n"),
			&mut diagnostics,
		)
		.unwrap();
		assert!(
			String::from_utf8(diagnostics)
				.unwrap()
				.contains("Defaults     disabled")
		);

		let confirmed = record_options(&temp, true);
		prepare_and_confirm_record(
			&confirmed,
			false,
			&mut Cursor::new(Vec::<u8>::new()),
			&mut Vec::new(),
		)
		.unwrap();
	}

	#[cfg(not(any(target_os = "linux", target_os = "macos")))]
	#[test]
	fn confirmation_fails_closed_on_unsupported_hosts() {
		let temp = TempDir::new().unwrap();
		let options = record_options(&temp, false);
		let error = prepare_and_confirm_record(
			&options,
			true,
			&mut Cursor::new(b"record\n"),
			&mut Vec::new(),
		)
		.err()
		.unwrap();

		assert!(matches!(
			error,
			pina_cli::verification::VerifyError::UnsupportedRecordHost { .. }
		));
	}
}

/// Print the human-readable form of a migrations inspect report.
pub fn print_inspect_report(report: &pina_cli::migrations::inspect::InspectReport) {
	if !report.exists {
		println!(
			"{} Account {} does not exist on chain (rpc: {}).",
			"⚠".yellow().bold(),
			report.address,
			report.rpc_url
		);
		return;
	}
	println!(
		"Account {} ({} bytes on chain, rpc: {})",
		report.address,
		report
			.data_len
			.map_or_else(|| "?".to_owned(), |len| len.to_string()),
		report.rpc_url
	);
	let Some(contract) = &report.contract else {
		println!(
			"{} No migration manifest contract matches this account's discriminator.",
			"⚠".yellow().bold()
		);
		return;
	};
	println!(
		"Contract {contract} ({})",
		report.rust_name.as_deref().unwrap_or("?")
	);
	match report.state {
		pina_cli::migrations::inspect::InspectState::Current => {
			println!(
				"{} Stored v{} matches the manifest's current version.",
				"✔".green(),
				report.stored_version.unwrap_or_default()
			);
		}
		pina_cli::migrations::inspect::InspectState::Future => {
			println!(
				"{} Stored v{} is newer than this checkout knows (current v{}); upgrade the \
				 client.",
				"✖".red().bold(),
				report.stored_version.unwrap_or_default(),
				report.current_version.unwrap_or_default()
			)
		}
		pina_cli::migrations::inspect::InspectState::Stale => {
			println!(
				"{} Stored v{} is stale; manifest current is v{} ({} pending hop(s)):",
				"⚠".yellow().bold(),
				report.stored_version.unwrap_or_default(),
				report.current_version.unwrap_or_default(),
				report.hops.len()
			);
			for hop in &report.hops {
				println!(
					"  v{} -> v{}: {} -> {} bytes, roughly {} lamports of rent",
					hop.from, hop.to, hop.byte_size_from, hop.byte_size_to, hop.rent_delta_lamports
				);
			}
			println!(
				"Route the reserved Migrate instruction (or rerun the touching transaction) to \
				 bring this account current."
			);
		}
		pina_cli::migrations::inspect::InspectState::UnknownContract => {
			println!(
				"{} The account data does not carry a decodable migration envelope.",
				"⚠".yellow().bold()
			);
		}
		pina_cli::migrations::inspect::InspectState::Empty => {}
	}
}

#[cfg(test)]
mod inspect_command_tests {
	use super::*;

	#[test]
	fn print_inspect_report_renders_every_state() {
		let address = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";
		let report = pina_cli::migrations::inspect::InspectReport {
			address: address.to_owned(),
			rpc_url: "http://rpc".to_owned(),
			exists: false,
			data_len: None,
			contract: None,
			rust_name: None,
			stored_version: None,
			current_version: None,
			state: pina_cli::migrations::inspect::InspectState::Empty,
			hops: Vec::new(),
		};
		print_inspect_report(&report);

		let future = pina_cli::migrations::inspect::InspectReport {
			exists: true,
			data_len: Some(13),
			contract: Some("account:1:01".to_owned()),
			rust_name: Some("State".to_owned()),
			stored_version: Some(9),
			current_version: Some(2),
			state: pina_cli::migrations::inspect::InspectState::Future,
			hops: Vec::new(),
			..report
		};
		print_inspect_report(&future);

		let stale = pina_cli::migrations::inspect::InspectReport {
			state: pina_cli::migrations::inspect::InspectState::Stale,
			stored_version: Some(0),
			hops: vec![pina_cli::migrations::inspect::InspectHop {
				from: 0,
				to: 1,
				byte_size_from: 10,
				byte_size_to: 11,
				rent_delta_lamports: 6_960,
			}],
			..future
		};
		print_inspect_report(&stale);

		let current = pina_cli::migrations::inspect::InspectReport {
			state: pina_cli::migrations::inspect::InspectState::Current,
			stored_version: Some(2),
			..stale
		};
		print_inspect_report(&current);

		let unknown = pina_cli::migrations::inspect::InspectReport {
			state: pina_cli::migrations::inspect::InspectState::UnknownContract,
			contract: None,
			rust_name: None,
			stored_version: None,
			current_version: None,
			hops: Vec::new(),
			..current
		};
		print_inspect_report(&unknown);
	}
}
