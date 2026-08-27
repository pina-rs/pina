//! Execution and presentation for the \`pina idl\` command family.

use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use owo_colors::OwoColorize;

use crate::cli::ExportArg;
use crate::cli::IdlCommands;
use crate::cli::IdlExportEncodingArg;
use crate::cli::IdlGenerateArgs;
use crate::run_idl;

pub(crate) fn run_idl_command(command: Option<IdlCommands>, generate: &IdlGenerateArgs) {
	match command {
		None => run_idl_generate(generate),
		Some(IdlCommands::Generate(args)) => run_idl_generate(&args),
		Some(IdlCommands::Fetch {
			cluster,
			program_id,
			project,
			output,
			json,
			npx,
		}) => {
			run_idl_fetch(
				cluster,
				program_id.as_deref(),
				&project,
				output.as_deref(),
				json,
				npx,
			);
		}
		Some(IdlCommands::Diff {
			cluster,
			program_id,
			project,
			file,
			json,
			npx,
		}) => {
			run_idl_diff(
				cluster,
				program_id.as_deref(),
				&project,
				file.as_deref(),
				json,
				npx,
			);
		}
		Some(IdlCommands::Publish {
			cluster,
			program_id,
			project,
			file,
			authority,
			payer,
			export,
			output,
			export_encoding,
			yes,
			json,
			priority_fee,
			npx,
		}) => {
			run_idl_publish(IdlPublishCommand {
				cluster,
				program_id,
				project,
				file,
				authority,
				payer,
				export,
				output,
				export_encoding,
				yes,
				json,
				priority_fee,
				npx,
			});
		}
	}
}

fn run_idl_generate(args: &IdlGenerateArgs) {
	run_idl(
		args.path.as_path(),
		args.output.as_deref(),
		args.name.as_deref(),
		!args.compact,
	);
}

fn run_idl_fetch(
	cluster: String,
	program_id: Option<&str>,
	project: &Path,
	output: Option<&Path>,
	json: bool,
	npx: String,
) {
	let local = if program_id.is_none() {
		Some(or_exit(pina_cli::idl_metadata::load_local_idl(
			project, None,
		)))
	} else {
		None
	};
	let program_id = or_exit(pina_cli::idl_metadata::resolve_program_id(
		program_id,
		local.as_ref(),
	));
	let client = pina_cli::idl_metadata::ClientOptions { npx, cluster };
	let value = or_exit(pina_cli::idl_metadata::fetch_idl(&client, &program_id));
	let pretty = or_exit(serde_json::to_vec_pretty(&value));

	if let Some(output) = output {
		or_exit(pina_cli::idl_metadata::write_atomic(output, &pretty));
		println!(
			"{} Wrote canonical IDL to {}",
			"✔".green(),
			safe_path_display(output)
		);
		return;
	}

	if json {
		println!(
			"{}",
			serde_json::json!({
				"schemaVersion": 1,
				"programId": program_id,
				"idl": value,
			})
		);
		return;
	}

	println!("{}", String::from_utf8_lossy(&pretty));
}

fn run_idl_diff(
	cluster: String,
	program_id: Option<&str>,
	project: &Path,
	file: Option<&Path>,
	json: bool,
	npx: String,
) {
	let local = or_exit(pina_cli::idl_metadata::load_local_idl(project, file));
	let program_id = or_exit(pina_cli::idl_metadata::resolve_program_id(
		program_id,
		Some(&local),
	));
	let client = pina_cli::idl_metadata::ClientOptions { npx, cluster };
	let on_chain = or_exit(pina_cli::idl_metadata::fetch_idl(&client, &program_id));
	let difference = pina_cli::idl_metadata::compare_idls(&local, on_chain);

	if json {
		println!("{}", or_exit(serde_json::to_string_pretty(&difference)));
	} else if difference.equal {
		println!(
			"{} Local and canonical on-chain IDLs are equal",
			"✔".green()
		);
	} else {
		println!("{} Local and canonical on-chain IDLs differ", "✘".red());
	}

	if !difference.equal {
		std::process::exit(2);
	}
}

struct IdlPublishCommand {
	cluster: String,
	program_id: Option<String>,
	project: PathBuf,
	file: Option<PathBuf>,
	authority: Option<PathBuf>,
	payer: Option<PathBuf>,
	export: Option<ExportArg>,
	output: Option<PathBuf>,
	export_encoding: IdlExportEncodingArg,
	yes: bool,
	json: bool,
	priority_fee: u64,
	npx: String,
}

fn run_idl_publish(command: IdlPublishCommand) {
	// Validate before any confirmation or output can reflect a user-provided RPC URL.
	or_exit(pina_cli::idl_metadata::rpc_url(&command.cluster));
	let cluster_display = pina_cli::idl_metadata::cluster_display(&command.cluster);
	let idl_label = command.file.as_ref().map_or_else(
		|| format!("generated from {}", safe_path_display(&command.project)),
		|path| safe_path_display(path),
	);
	let local = or_exit(pina_cli::idl_metadata::load_local_idl(
		&command.project,
		command.file.as_deref(),
	));
	let program_id = or_exit(pina_cli::idl_metadata::resolve_program_id(
		command.program_id.as_deref(),
		Some(&local),
	));
	let export_authority = command.export.as_ref().and_then(ExportArg::authority);
	let exporting = command.export.is_some();

	if export_authority.is_some() && command.payer.is_some() {
		exit_with_error(
			"--payer cannot be used with an exported authority because the official planner uses \
			 the noop authority as payer",
		);
	}

	if export_authority.is_some() && command.authority.is_some() {
		exit_with_error(
			"--authority cannot be used with --export <ADDRESS> because the exported authority is \
			 a noop signer",
		);
	}

	if !exporting && command.authority.is_none() {
		exit_with_error("--authority is required when publication is not exported");
	}

	if exporting && export_authority.is_none() && command.authority.is_none() {
		exit_with_error("--authority is required for --export without an authority address");
	}

	let confirmation = (!exporting && !command.yes)
		.then(|| confirm_publication(&cluster_display, &program_id, &idl_label))
		.transpose();
	or_exit(confirmation);

	let client = pina_cli::idl_metadata::ClientOptions {
		npx: command.npx,
		cluster: command.cluster.clone(),
	};
	let export_encoding = match command.export_encoding {
		IdlExportEncodingArg::Base58 => "base58",
		IdlExportEncodingArg::Base64 => "base64",
	};
	let options = pina_cli::idl_metadata::PublishOptions {
		local: &local,
		authority: command.authority.as_deref(),
		payer: command.payer.as_deref(),
		priority_fee: command.priority_fee,
		export: exporting,
		export_authority,
		export_encoding,
	};
	let exported = or_exit(pina_cli::idl_metadata::publish_idl(&client, &options));

	if let Some(exported) = exported {
		if let Some(output) = command.output {
			or_exit(pina_cli::idl_metadata::write_atomic(
				&output,
				exported.as_bytes(),
			));
			eprintln!(
				"{} Wrote every exported transaction to {}",
				"✔".green(),
				safe_path_display(&output)
			);
		} else {
			print!("{exported}");
		}

		return;
	}

	if command.json {
		println!(
			"{}",
			serde_json::json!({
				"schemaVersion": 1,
				"status": "published",
				"programId": program_id,
				"seed": "idl",
				"cluster": cluster_display,
			})
		);
	} else {
		println!(
			"{} Published the canonical IDL for {}",
			"✔".green(),
			program_id
		);
	}
}

fn confirm_publication(cluster: &str, program_id: &str, idl: &str) -> Result<(), &'static str> {
	use std::io::IsTerminal;

	let stdin = std::io::stdin();
	let mut input = stdin.lock();
	let mut output = std::io::stderr().lock();
	let confirmation = or_exit(prompt_publication(
		stdin.is_terminal(),
		&mut input,
		&mut output,
		cluster,
		program_id,
		idl,
	));

	confirmation_error(&confirmation).map_or(Ok(()), Err)
}

fn confirmation_error(confirmation: &Confirmation) -> Option<&'static str> {
	match confirmation {
		Confirmation::Approved => None,
		Confirmation::NonInteractive => {
			Some(
				"publication requires confirmation; rerun with --yes in non-interactive \
				 environments",
			)
		}
		Confirmation::Cancelled => Some("publication cancelled"),
	}
}

fn safe_path_display(path: &Path) -> String {
	path.to_string_lossy()
		.chars()
		.fold(String::new(), |mut output, character| {
			if character.is_control() {
				output.extend(character.escape_default());
			} else {
				output.push(character);
			}

			output
		})
}

#[derive(Debug, PartialEq, Eq)]
enum Confirmation {
	Approved,
	Cancelled,
	NonInteractive,
}

fn prompt_publication(
	is_terminal: bool,
	input: &mut impl BufRead,
	output: &mut impl Write,
	cluster: &str,
	program_id: &str,
	idl: &str,
) -> std::io::Result<Confirmation> {
	if !is_terminal {
		return Ok(Confirmation::NonInteractive);
	}

	writeln!(output, "Publish canonical IDL metadata?")?;
	writeln!(output, "  Cluster  {cluster}")?;
	writeln!(output, "  Program  {program_id}")?;
	writeln!(output, "  IDL      {idl}")?;
	write!(output, "Type `publish` to continue: ")?;
	output.flush()?;
	let mut response = String::new();
	input.read_line(&mut response)?;

	if response.trim() == "publish" {
		Ok(Confirmation::Approved)
	} else {
		Ok(Confirmation::Cancelled)
	}
}

fn or_exit<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
	match result {
		Ok(value) => value,
		Err(error) => exit_with_error(&error.to_string()),
	}
}

fn exit_with_error(message: &str) -> ! {
	eprintln!("{} {}", "Error".red().bold(), message);
	std::process::exit(1);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn publication_prompt_covers_approval_cancellation_and_non_interactive_use() {
		let mut output = Vec::new();
		let decision = prompt_publication(
			true,
			&mut "publish\n".as_bytes(),
			&mut output,
			"devnet",
			"program",
			"idl.json",
		)
		.unwrap_or_else(|error| panic!("prompt failed: {error}"));
		assert_eq!(decision, Confirmation::Approved);
		assert_eq!(confirmation_error(&decision), None);
		assert!(String::from_utf8_lossy(&output).contains("Type `publish`"));

		let cancelled = prompt_publication(
			true,
			&mut "no\n".as_bytes(),
			&mut Vec::new(),
			"devnet",
			"program",
			"idl.json",
		)
		.unwrap_or_else(|error| panic!("prompt failed: {error}"));
		assert_eq!(cancelled, Confirmation::Cancelled);
		assert_eq!(
			confirmation_error(&cancelled),
			Some("publication cancelled")
		);

		let non_interactive = prompt_publication(
			false,
			&mut "".as_bytes(),
			&mut Vec::new(),
			"devnet",
			"program",
			"idl.json",
		)
		.unwrap_or_else(|error| panic!("prompt failed: {error}"));
		assert_eq!(non_interactive, Confirmation::NonInteractive);
		assert!(
			confirmation_error(&non_interactive)
				.unwrap_or_default()
				.contains("requires confirmation")
		);
	}

	#[test]
	fn terminal_paths_escape_newlines_and_ansi_controls() {
		let displayed = safe_path_display(Path::new("idl\n\u{1b}[31m.json"));
		assert_eq!(displayed, "idl\\n\\u{1b}[31m.json");
		assert!(!displayed.contains('\n'));
		assert!(!displayed.contains('\u{1b}'));
	}
}
