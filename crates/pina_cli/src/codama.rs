use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;

use atomic_write_file::AtomicWriteFile;
use pina_codama_renderer::RenderConfig;
use pina_codama_renderer::render_idl_file;

use crate::dart_client::validate_dart_client_idls;
use crate::dart_client::write_dart_package_barrels;
use crate::error::CodamaError;
use crate::generate_idl;
use crate::js_client::harden_generated_clients;
use crate::project::ClientLanguage;
use crate::project::Project;

const CLIENT_RENDER_SCRIPT: &str = r#"
import { createFromJson, visit } from "codama";
import { readFileSync } from "node:fs";
import { basename, join } from "node:path";

const [renderer, outputRoot, ...idlPaths] = process.argv.slice(1);

if (!renderer || !outputRoot) {
	throw new Error("missing renderer or output root argument");
}

for (const idlPath of idlPaths.sort()) {
	const name = basename(idlPath, ".json");
	const json = readFileSync(idlPath, "utf8");
	const codama = createFromJson(json);

	if (renderer === "typescript") {
		const { renderVisitor } = await import("@codama/renderers-js");
		await codama.accept(renderVisitor(join(outputRoot, name), {
			formatCode: false,
			deleteFolderBeforeRendering: true,
		}));
		continue;
	}

	if (renderer === "dart") {
		const { renderVisitor } = await import("codama-renderers-dart");
		visit(codama.getRoot(), renderVisitor(join(outputRoot, "lib", "src", "generated", name), {
			formatCode: false,
			deleteFolderBeforeRendering: true,
		}));
		continue;
	}

	throw new Error(`unknown renderer: ${renderer}`);
}
"#;

#[derive(Debug, Clone)]
pub struct CodamaGenerateOptions {
	pub examples_dir: PathBuf,
	pub idls_dir: PathBuf,
	pub rust_out: PathBuf,
	pub js_out: PathBuf,
	pub dart_out: PathBuf,
	pub examples: Vec<String>,
	pub npx: String,
}

/// Options for project-aware client generation.
#[derive(Debug, Clone)]
pub struct ProjectGenerateOptions {
	pub project_dir: PathBuf,
	pub clients: Vec<ClientLanguage>,
	pub output: Option<PathBuf>,
	pub npx: String,
}

/// Outputs produced by project-aware client generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGenerateOutput {
	pub package_name: String,
	pub idl: PathBuf,
	pub clients_dir: PathBuf,
	pub clients: Vec<ClientLanguage>,
}

#[derive(Debug)]
struct GenerationPlan {
	programs: Vec<(String, PathBuf)>,
	idls_dir: PathBuf,
	rust_out: PathBuf,
	typescript_out: PathBuf,
	dart_out: PathBuf,
	clients: BTreeSet<ClientLanguage>,
	npx: String,
}

pub fn generate_codama(options: &CodamaGenerateOptions) -> Result<Vec<String>, CodamaError> {
	let examples = collect_examples(options)?;
	let programs = examples
		.iter()
		.map(|example| (example.clone(), options.examples_dir.join(example)))
		.collect();
	let plan = GenerationPlan {
		programs,
		idls_dir: options.idls_dir.clone(),
		rust_out: options.rust_out.clone(),
		typescript_out: options.js_out.clone(),
		dart_out: options.dart_out.clone(),
		clients: [
			ClientLanguage::Rust,
			ClientLanguage::Typescript,
			ClientLanguage::Dart,
		]
		.into_iter()
		.collect(),
		npx: options.npx.clone(),
	};

	generate_plan(&plan)?;

	Ok(examples)
}

/// Generate selected clients for the project discovered from `project_dir`.
///
/// An empty `clients` override uses the languages configured in `Pina.toml`.
/// The output override is resolved from the process working directory, matching
/// normal command-line path behavior.
///
/// # Errors
///
/// Returns an error when project discovery, IDL generation, or a selected
/// renderer fails.
pub fn generate_project_clients(
	options: &ProjectGenerateOptions,
) -> Result<ProjectGenerateOutput, CodamaError> {
	let project = Project::discover(&options.project_dir).map_err(CodamaError::Project)?;
	let clients = if options.clients.is_empty() {
		project.clients.clone()
	} else {
		options.clients.clone()
	};
	let clients = clients.into_iter().collect::<BTreeSet<_>>();
	let clients_dir = options
		.output
		.clone()
		.unwrap_or_else(|| project.clients_dir.clone());
	validate_render_target(&clients_dir)?;
	let plan = GenerationPlan {
		programs: vec![(project.library_name.clone(), project.program_dir.clone())],
		idls_dir: project.idl_dir.clone(),
		rust_out: clients_dir.join("rust"),
		typescript_out: clients_dir.join("typescript"),
		dart_out: clients_dir.join("dart"),
		clients: clients.clone(),
		npx: options.npx.clone(),
	};

	let idl_paths = generate_plan(&plan)?;
	let idl = idl_paths
		.into_iter()
		.next()
		.ok_or_else(|| CodamaError::NoPrograms)?;

	Ok(ProjectGenerateOutput {
		package_name: project.package_name,
		idl,
		clients_dir,
		clients: clients.into_iter().collect(),
	})
}

fn generate_plan(plan: &GenerationPlan) -> Result<Vec<PathBuf>, CodamaError> {
	let examples = plan
		.programs
		.iter()
		.map(|(name, _)| name.clone())
		.collect::<Vec<_>>();

	std::fs::create_dir_all(&plan.idls_dir).map_err(|source| {
		CodamaError::CreateDir {
			path: plan.idls_dir.clone(),
			source,
		}
	})?;

	for path in selected_output_dirs(plan) {
		validate_render_target(path)?;
		std::fs::create_dir_all(path).map_err(|source| {
			CodamaError::CreateDir {
				path: path.to_path_buf(),
				source,
			}
		})?;
	}

	let mut idl_paths = Vec::with_capacity(plan.programs.len());
	for (example, program_path) in &plan.programs {
		let idl = generate_idl(program_path, None).map_err(|source| {
			CodamaError::GenerateIdl {
				example: example.clone(),
				path: program_path.clone(),
				source,
			}
		})?;
		let idl_json = serde_json::to_string_pretty(&idl).map_err(|source| {
			CodamaError::SerializeIdl {
				example: example.clone(),
				source,
			}
		})?;

		let idl_path = plan.idls_dir.join(format!("{example}.json"));
		write_idl_atomic(&idl_path, idl_json.as_bytes()).map_err(|source| {
			CodamaError::WriteIdl {
				path: idl_path.clone(),
				source,
			}
		})?;
		idl_paths.push(idl_path);
	}

	if plan.clients.contains(&ClientLanguage::Rust) {
		let render_config = RenderConfig::default();

		for (example, idl_path) in examples.iter().zip(idl_paths.iter()) {
			let crate_dir = plan.rust_out.join(example);
			validate_render_target(&crate_dir)?;
			render_idl_file(idl_path, &crate_dir, &render_config).map_err(|source| {
				CodamaError::RenderRust {
					path: crate_dir.clone(),
					source,
				}
			})?;
		}
	}

	if plan.clients.contains(&ClientLanguage::Typescript) {
		for example in &examples {
			validate_render_target(&plan.typescript_out.join(example))?;
		}

		run_client_generation(plan, ClientLanguage::Typescript, &idl_paths)?;
		harden_generated_clients(&plan.typescript_out, &examples)?;
	}

	if plan.clients.contains(&ClientLanguage::Dart) {
		for example in &examples {
			validate_render_target(&plan.dart_out.join("lib/src/generated").join(example))?;
		}

		validate_dart_client_idls(&plan.dart_out, &examples, &idl_paths)?;
		run_client_generation(plan, ClientLanguage::Dart, &idl_paths)?;
		write_dart_package_barrels(&plan.dart_out, &examples)?;
	}

	Ok(idl_paths)
}

fn validate_render_target(path: &Path) -> Result<(), CodamaError> {
	let absolute = std::path::absolute(path).map_err(|source| {
		CodamaError::CreateDir {
			path: path.to_path_buf(),
			source,
		}
	})?;

	if absolute.parent().is_none() {
		return Err(CodamaError::UnsafeOutput {
			path: absolute,
			reason: "filesystem roots cannot be generation targets".to_owned(),
		});
	}

	for ancestor in absolute.ancestors() {
		if let Ok(metadata) = std::fs::symlink_metadata(ancestor)
			&& metadata.file_type().is_symlink()
		{
			return Err(CodamaError::UnsafeOutput {
				path: path.to_path_buf(),
				reason: format!(
					"generation targets cannot traverse symbolic link {}",
					ancestor.display()
				),
			});
		}
	}

	Ok(())
}

fn write_idl_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
	use std::io::Write;

	let mut file = AtomicWriteFile::open(path)?;
	file.write_all(contents)?;
	file.commit()
}

fn selected_output_dirs(plan: &GenerationPlan) -> Vec<&Path> {
	let mut paths = Vec::with_capacity(plan.clients.len());

	if plan.clients.contains(&ClientLanguage::Rust) {
		paths.push(plan.rust_out.as_path());
	}

	if plan.clients.contains(&ClientLanguage::Typescript) {
		paths.push(plan.typescript_out.as_path());
	}

	if plan.clients.contains(&ClientLanguage::Dart) {
		paths.push(plan.dart_out.as_path());
	}

	paths
}

fn collect_examples(options: &CodamaGenerateOptions) -> Result<Vec<String>, CodamaError> {
	let mut available = std::fs::read_dir(&options.examples_dir)
		.map_err(|source| {
			CodamaError::ReadExamples {
				path: options.examples_dir.clone(),
				source,
			}
		})?
		.filter_map(Result::ok)
		.filter(|entry| entry.path().is_dir())
		.filter_map(|entry| entry.file_name().into_string().ok())
		.collect::<Vec<_>>();

	available.sort();

	if available.is_empty() {
		return Err(CodamaError::NoExamples {
			path: options.examples_dir.clone(),
		});
	}

	if options.examples.is_empty() {
		return Ok(available);
	}

	let available_set = available.iter().cloned().collect::<BTreeSet<_>>();
	for requested in &options.examples {
		if !available_set.contains(requested) {
			return Err(CodamaError::UnknownExample {
				example: requested.clone(),
				available: available.join(", "),
			});
		}
	}

	let mut selected: Vec<_> = options
		.examples
		.iter()
		.cloned()
		.collect::<BTreeSet<_>>()
		.into_iter()
		.collect();
	selected.sort();

	Ok(selected)
}

fn run_client_generation(
	plan: &GenerationPlan,
	renderer: ClientLanguage,
	idl_paths: &[PathBuf],
) -> Result<(), CodamaError> {
	let output = if plan.npx == "node" {
		run_client_generation_with_node(plan, renderer, idl_paths)?
	} else {
		match run_client_generation_with_npx(plan, renderer, idl_paths) {
			Ok(output) => output,
			Err(source) if source.kind() == std::io::ErrorKind::NotFound && plan.npx == "npx" => {
				run_client_generation_with_pnpm(plan, renderer, idl_paths)?
			}
			Err(source) => {
				return Err(CodamaError::RunCommand {
					cmd: plan.npx.clone(),
					source,
				});
			}
		}
	};

	if output.success() {
		return Ok(());
	}

	let cmd = if plan.npx == "npx" {
		"npx (or fallback pnpm dlx)".to_string()
	} else {
		plan.npx.clone()
	};

	Err(command_failed(cmd, output))
}

fn command_failed(cmd: String, status: ExitStatus) -> CodamaError {
	CodamaError::CommandFailed {
		cmd,
		status: status.to_string(),
	}
}

fn run_client_generation_with_npx(
	plan: &GenerationPlan,
	renderer: ClientLanguage,
	idl_paths: &[PathBuf],
) -> std::io::Result<ExitStatus> {
	let mut command = Command::new(&plan.npx);

	command.arg("-y").arg("-p").arg("codama@1.10.1");
	add_npx_renderer_package(&mut command, renderer);
	command
		.arg("node")
		.arg("--input-type=module")
		.arg("-e")
		.arg(CLIENT_RENDER_SCRIPT)
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer));

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	command.status()
}

fn run_client_generation_with_pnpm(
	plan: &GenerationPlan,
	renderer: ClientLanguage,
	idl_paths: &[PathBuf],
) -> Result<ExitStatus, CodamaError> {
	let mut command = Command::new("pnpm");
	command.arg("dlx").arg("--package").arg("codama@1.10.1");
	add_pnpm_renderer_package(&mut command, renderer);
	command
		.arg("node")
		.arg("--input-type=module")
		.arg("-e")
		.arg(CLIENT_RENDER_SCRIPT)
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer));

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	command.status().map_err(|source| {
		CodamaError::RunCommand {
			cmd: "pnpm".to_string(),
			source,
		}
	})
}

fn run_client_generation_with_node(
	plan: &GenerationPlan,
	renderer: ClientLanguage,
	idl_paths: &[PathBuf],
) -> Result<ExitStatus, CodamaError> {
	let mut command = Command::new("node");
	command
		.arg("--input-type=module")
		.arg("-e")
		.arg(CLIENT_RENDER_SCRIPT)
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer));

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	command.status().map_err(|source| {
		CodamaError::RunCommand {
			cmd: "node".to_string(),
			source,
		}
	})
}

fn renderer_output(plan: &GenerationPlan, renderer: ClientLanguage) -> &Path {
	match renderer {
		ClientLanguage::Typescript => &plan.typescript_out,
		ClientLanguage::Dart => &plan.dart_out,
		ClientLanguage::Rust => &plan.rust_out,
	}
}

fn add_npx_renderer_package(command: &mut Command, renderer: ClientLanguage) {
	command.arg("-p");

	match renderer {
		ClientLanguage::Typescript => command.arg("@codama/renderers-js@2.3.1"),
		ClientLanguage::Dart => command.arg("codama-renderers-dart@0.5.1"),
		ClientLanguage::Rust => command.arg("codama@1.10.1"),
	};
}

fn add_pnpm_renderer_package(command: &mut Command, renderer: ClientLanguage) {
	command.arg("--package");

	match renderer {
		ClientLanguage::Typescript => command.arg("@codama/renderers-js@2.3.1"),
		ClientLanguage::Dart => command.arg("codama-renderers-dart@0.5.1"),
		ClientLanguage::Rust => command.arg("codama@1.10.1"),
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[cfg(unix)]
	#[test]
	fn command_failure_preserves_signal_status() {
		use std::os::unix::process::ExitStatusExt;

		let error = command_failed("renderer".to_owned(), ExitStatus::from_raw(15));
		let message = error.to_string();

		assert!(message.contains("signal"));
		assert!(message.contains("15"));
		assert!(!message.contains("-1"));
	}

	#[cfg(unix)]
	#[test]
	fn render_target_rejects_symlink_ancestor() {
		use std::os::unix::fs::symlink;

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let real = temp.path().join("real");
		let link = temp.path().join("link");
		std::fs::create_dir_all(&real)
			.unwrap_or_else(|error| panic!("failed to create real dir: {error}"));
		symlink(&real, &link).unwrap_or_else(|error| panic!("failed to create symlink: {error}"));

		let error = validate_render_target(&link.join("generated"))
			.expect_err("symlink traversal should be rejected");

		assert!(error.to_string().contains("symbolic link"));
	}

	#[test]
	fn render_target_rejects_filesystem_root() {
		let root = if cfg!(windows) {
			Path::new(r"C:\")
		} else {
			Path::new("/")
		};
		let error = validate_render_target(root).expect_err("filesystem root should be rejected");

		assert!(error.to_string().contains("filesystem roots"));
	}
}
