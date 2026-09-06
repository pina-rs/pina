use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;

use atomic_write_file::AtomicWriteFile;
use pina_codama_renderer::RenderConfig;
use pina_codama_renderer::RenderMode as RustRenderMode;
use pina_codama_renderer::render_idl_file;
use pina_cpi_renderer::RenderConfig as CpiRenderConfig;
use pina_cpi_renderer::RenderMode as CpiRenderMode;
use pina_cpi_renderer::render_idl_file as render_cpi_idl_file;

use crate::dart_client::validate_dart_client_idls;
use crate::dart_client::write_dart_package_barrels;
use crate::error::CodamaError;
use crate::generate_idl;
use crate::js_client::harden_generated_clients;
use crate::project::ClientLanguage;
use crate::project::GenerationMode;
use crate::project::Project;

const CLIENT_RENDER_SCRIPT: &str = r#"
import { createFromJson, visit } from "codama";
import {
	cpSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	renameSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";

const [renderer, outputRoot, requestedMode, scaffoldValue, ...idlPaths] = process.argv.slice(2);

if (!renderer || !outputRoot || !requestedMode || !scaffoldValue) {
	throw new Error("missing renderer, output root, mode, or scaffold argument");
}

const scaffold = scaffoldValue === "true";
const names = idlPaths.map((path) => basename(path, ".json")).sort();
const stagingRoot = mkdtempSync(join(tmpdir(), "pina-clients-"));

try {
	for (const idlPath of idlPaths.sort()) {
		const name = basename(idlPath, ".json");
		const json = readFileSync(idlPath, "utf8");
		const codama = createFromJson(json);

		if (renderer === "typescript") {
			const { renderVisitor } = await import("@codama/renderers-js");
			await codama.accept(renderVisitor(join(stagingRoot, name), {
				formatCode: false,
				deleteFolderBeforeRendering: true,
			}));
			continue;
		}

		if (renderer === "dart") {
			const { renderVisitor } = await import("codama-renderers-dart");
			visit(codama.getRoot(), renderVisitor(join(stagingRoot, "lib", "src", "generated", name), {
				formatCode: false,
				deleteFolderBeforeRendering: true,
			}));
			continue;
		}

		throw new Error(`unknown renderer: ${renderer}`);
	}

	if (renderer === "typescript") {
		for (const name of names) {
			publishTypescript(name);
		}
	} else {
		publishDart();
	}
} finally {
	rmSync(stagingRoot, { force: true, recursive: true });
}

function publishTypescript(name) {
	const destination = join(outputRoot, name);
	const staged = join(stagingRoot, name);
	const mode = resolveMode(destination);

	if (mode === "overwrite") {
		rmSync(destination, { force: true, recursive: true });
	}

	publishDirectory(
		join(staged, "src", "generated"),
		join(destination, "src", "generated"),
	);

	const manifest = join(destination, "package.json");
	if (scaffold && !existsSync(manifest)) {
		mkdirSync(destination, { recursive: true });
		cpSync(join(staged, "package.json"), manifest);
	}
}

function publishDart() {
	const mode = resolveMode(outputRoot);

	if (mode === "overwrite") {
		rmSync(outputRoot, { force: true, recursive: true });
	}

	for (const name of names) {
		publishDirectory(
			join(stagingRoot, "lib", "src", "generated", name),
			join(outputRoot, "lib", "src", "generated", name),
		);
	}

	const manifest = join(outputRoot, "pubspec.yaml");
	if (scaffold && !existsSync(manifest)) {
		mkdirSync(outputRoot, { recursive: true });
		writeFileSync(manifest, dartManifest(), "utf8");
	}
}

function publishDirectory(staged, destination) {
	const parent = dirname(destination);
	const next = `${destination}.pina-next`;
	mkdirSync(parent, { recursive: true });
	rmSync(next, { force: true, recursive: true });
	cpSync(staged, next, { recursive: true });
	rmSync(destination, { force: true, recursive: true });
	renameSync(next, destination);
}

function resolveMode(destination) {
	const empty = !existsSync(destination) || readdirSync(destination).length === 0;

	if (requestedMode === "auto") {
		return empty ? "create" : "update";
	}

	if (requestedMode === "create" && !empty) {
		throw new Error(`cannot create generated client at nonempty destination: ${destination}`);
	}

	if (requestedMode === "update" && empty) {
		throw new Error(`cannot update missing or empty generated client: ${destination}`);
	}

	return requestedMode;
}

function dartManifest() {
	const packageName = names.length === 1
		? `${names[0]}_client`
		: "pina_clients";
	return `name: ${packageName}
description: Generated Dart and Flutter clients for Pina programs.
publish_to: none

environment:
  sdk: ">=3.10.0 <4.0.0"

dependencies:
  meta: ^1.16.0
  solana_kit_accounts: ^0.8.0
  solana_kit_addresses: ^0.8.0
  solana_kit_codecs_core: ^0.8.0
  solana_kit_codecs_data_structures: ^0.8.0
  solana_kit_codecs_numbers: ^0.8.0
  solana_kit_codecs_strings: ^0.8.0
  solana_kit_errors: ^0.8.0
  solana_kit_instructions: ^0.8.0
  solana_kit_rpc_types: ^0.8.0
`;
}
"#;

#[derive(Debug, Clone)]
pub struct CodamaGenerateOptions {
	pub examples_dir: PathBuf,
	pub idls_dir: PathBuf,
	pub rust_out: PathBuf,
	pub cpi_out: PathBuf,
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
	pub mode: Option<GenerationMode>,
	pub scaffold: Option<bool>,
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
	override_idl_names: bool,
	idls_dir: PathBuf,
	rust_out: PathBuf,
	cpi_out: PathBuf,
	typescript_out: PathBuf,
	dart_out: PathBuf,
	clients: BTreeSet<ClientLanguage>,
	generation: BTreeMap<ClientLanguage, GenerationSettings>,
	npx: String,
}

#[derive(Debug, Clone, Copy)]
struct GenerationSettings {
	mode: GenerationMode,
	scaffold: bool,
}

struct BoundedOutput {
	status: ExitStatus,
	stdout: Vec<u8>,
	stderr: Vec<u8>,
}

pub fn generate_codama(options: &CodamaGenerateOptions) -> Result<Vec<String>, CodamaError> {
	let examples = collect_examples(options)?;
	let programs = examples
		.iter()
		.map(|example| (example.clone(), options.examples_dir.join(example)))
		.collect();
	let plan = GenerationPlan {
		programs,
		override_idl_names: false,
		idls_dir: options.idls_dir.clone(),
		rust_out: options.rust_out.clone(),
		cpi_out: options.cpi_out.clone(),
		typescript_out: options.js_out.clone(),
		dart_out: options.dart_out.clone(),
		clients: [
			ClientLanguage::Cpi,
			ClientLanguage::Rust,
			ClientLanguage::Typescript,
			ClientLanguage::Dart,
		]
		.into_iter()
		.collect(),
		generation: default_generation_settings(),
		npx: options.npx.clone(),
	};

	generate_plan(&plan)?;

	Ok(examples)
}

/// Generate selected clients for the project discovered from `project_dir`.
///
/// An empty `clients` override uses the languages configured in `pina.toml`.
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
	let client_target = |language: ClientLanguage| {
		let configured = &project.client_generation[&language];

		clients_dir.join(&configured.output)
	};
	let generation = project
		.client_generation
		.iter()
		.map(|(language, configured)| {
			(
				*language,
				GenerationSettings {
					mode: options.mode.unwrap_or(configured.mode),
					scaffold: options.scaffold.unwrap_or(configured.scaffold),
				},
			)
		})
		.collect();
	let plan = GenerationPlan {
		programs: vec![(project.library_name.clone(), project.program_dir.clone())],
		override_idl_names: true,
		idls_dir: project.idl_dir.clone(),
		rust_out: client_target(ClientLanguage::Rust),
		cpi_out: client_target(ClientLanguage::Cpi),
		typescript_out: client_target(ClientLanguage::Typescript),
		dart_out: client_target(ClientLanguage::Dart),
		clients: clients.clone(),
		generation,
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

fn default_generation_settings() -> BTreeMap<ClientLanguage, GenerationSettings> {
	[
		ClientLanguage::Cpi,
		ClientLanguage::Rust,
		ClientLanguage::Typescript,
		ClientLanguage::Dart,
	]
	.into_iter()
	.map(|language| {
		(
			language,
			GenerationSettings {
				mode: GenerationMode::Auto,
				scaffold: true,
			},
		)
	})
	.collect()
}

fn generate_plan(plan: &GenerationPlan) -> Result<Vec<PathBuf>, CodamaError> {
	let examples = plan
		.programs
		.iter()
		.map(|(name, _)| name.clone())
		.collect::<Vec<_>>();

	create_output_dir(&plan.idls_dir)?;

	for path in selected_output_dirs(plan) {
		validate_render_target(path)?;
	}

	let mut idl_paths = Vec::with_capacity(plan.programs.len());
	for (example, program_path) in &plan.programs {
		let name_override = plan.override_idl_names.then_some(example.as_str());
		let idl = generate_idl(program_path, name_override).map_err(|source| {
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
		let settings = plan.generation[&ClientLanguage::Rust];
		let render_config = RenderConfig {
			mode: rust_render_mode(settings.mode),
			scaffold: settings.scaffold,
			..RenderConfig::default()
		};

		for (example, idl_path) in examples.iter().zip(idl_paths.iter()) {
			let crate_dir = plan.rust_out.join(example);
			validate_render_target(&crate_dir)?;
			render_rust_client(idl_path, &crate_dir, &render_config)?;
		}
	}

	if plan.clients.contains(&ClientLanguage::Cpi) {
		let settings = plan.generation[&ClientLanguage::Cpi];
		let render_config = CpiRenderConfig {
			mode: cpi_render_mode(settings.mode),
			scaffold: settings.scaffold,
			..CpiRenderConfig::default()
		};

		for (example, idl_path) in examples.iter().zip(idl_paths.iter()) {
			let crate_dir = plan.cpi_out.join(example);
			validate_render_target(&crate_dir)?;
			render_cpi_client(idl_path, &crate_dir, &render_config)?;
		}
	}

	if plan.clients.contains(&ClientLanguage::Typescript) {
		let settings = plan.generation[&ClientLanguage::Typescript];

		for example in &examples {
			validate_generation_target(&plan.typescript_out.join(example), settings)?;
		}

		run_client_generation(plan, ClientLanguage::Typescript, &idl_paths)?;
		harden_generated_clients(&plan.typescript_out, &examples)?;
	}

	if plan.clients.contains(&ClientLanguage::Dart) {
		let settings = plan.generation[&ClientLanguage::Dart];
		validate_generation_target(&plan.dart_out, settings)?;

		for example in &examples {
			validate_render_target(&plan.dart_out.join("lib/src/generated").join(example))?;
		}

		validate_dart_client_idls(&plan.dart_out, &examples, &idl_paths)?;
		run_client_generation(plan, ClientLanguage::Dart, &idl_paths)?;
		write_dart_package_barrels(&plan.dart_out, &examples)?;
	}

	Ok(idl_paths)
}

fn validate_generation_target(
	path: &Path,
	settings: GenerationSettings,
) -> Result<(), CodamaError> {
	validate_render_target(path)?;
	let empty = destination_is_empty(path)?;

	if settings.mode == GenerationMode::Create && !empty {
		return Err(CodamaError::InvalidGenerationState {
			path: path.to_path_buf(),
			mode: "create",
			reason: "the destination is not empty",
		});
	}

	if settings.mode == GenerationMode::Update && empty {
		return Err(CodamaError::InvalidGenerationState {
			path: path.to_path_buf(),
			mode: "update",
			reason: "the destination is empty or does not exist",
		});
	}

	if settings.mode != GenerationMode::Overwrite {
		return Ok(());
	}

	let absolute = if path.exists() {
		std::fs::canonicalize(path).map_err(|source| create_dir_error(path, source))?
	} else {
		std::path::absolute(path).map_err(|source| create_dir_error(path, source))?
	};
	let current_dir =
		std::env::current_dir().map_err(|source| create_dir_error(Path::new("."), source))?;
	let current = std::fs::canonicalize(&current_dir)
		.map_err(|source| create_dir_error(&current_dir, source))?;

	if current.starts_with(&absolute) || absolute.join(".git").exists() {
		return Err(CodamaError::UnsafeOutput {
			path: absolute,
			reason: "refusing to overwrite a working tree".to_owned(),
		});
	}

	Ok(())
}

fn destination_is_empty(path: &Path) -> Result<bool, CodamaError> {
	let metadata = match std::fs::symlink_metadata(path) {
		Ok(metadata) => metadata,
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(true),
		Err(source) => return Err(create_dir_error(path, source)),
	};

	if !metadata.is_dir() {
		return Ok(false);
	}

	std::fs::read_dir(path)
		.map_err(|source| create_dir_error(path, source))?
		.next()
		.transpose()
		.map(|entry| entry.is_none())
		.map_err(|source| create_dir_error(path, source))
}

const fn rust_render_mode(mode: GenerationMode) -> RustRenderMode {
	match mode {
		GenerationMode::Auto => RustRenderMode::Auto,
		GenerationMode::Create => RustRenderMode::Create,
		GenerationMode::Update => RustRenderMode::Update,
		GenerationMode::Overwrite => RustRenderMode::Overwrite,
	}
}

const fn cpi_render_mode(mode: GenerationMode) -> CpiRenderMode {
	match mode {
		GenerationMode::Auto => CpiRenderMode::Auto,
		GenerationMode::Create => CpiRenderMode::Create,
		GenerationMode::Update => CpiRenderMode::Update,
		GenerationMode::Overwrite => CpiRenderMode::Overwrite,
	}
}

fn create_output_dir(path: &Path) -> Result<(), CodamaError> {
	std::fs::create_dir_all(path).map_err(|source| {
		CodamaError::CreateDir {
			path: path.to_path_buf(),
			source,
		}
	})
}

fn render_rust_client(
	idl_path: &Path,
	crate_dir: &Path,
	config: &RenderConfig,
) -> Result<(), CodamaError> {
	render_idl_file(idl_path, crate_dir, config).map_err(|source| {
		CodamaError::RenderRust {
			path: crate_dir.to_path_buf(),
			source,
		}
	})
}

fn render_cpi_client(
	idl_path: &Path,
	crate_dir: &Path,
	config: &CpiRenderConfig,
) -> Result<(), CodamaError> {
	render_cpi_idl_file(idl_path, crate_dir, config)
		.map_err(|source| cpi_render_error(crate_dir, source))
}

fn cpi_render_error(path: &Path, source: pina_cpi_renderer::RenderError) -> CodamaError {
	CodamaError::RenderCpi {
		path: path.to_path_buf(),
		source,
	}
}

pub(crate) fn validate_render_target(path: &Path) -> Result<(), CodamaError> {
	let absolute = std::path::absolute(path).map_err(|source| create_dir_error(path, source))?;

	if absolute.parent().is_none() {
		return Err(CodamaError::UnsafeOutput {
			path: absolute,
			reason: "filesystem roots cannot be generation targets".to_owned(),
		});
	}

	validate_existing_render_tree(path, &absolute)
}

fn validate_existing_render_tree(root: &Path, path: &Path) -> Result<(), CodamaError> {
	let metadata = match std::fs::symlink_metadata(path) {
		Ok(metadata) => metadata,
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(source) => return Err(create_dir_error(path, source)),
	};

	if is_link_like(&metadata) {
		return Err(unsafe_output_link(root, path));
	}

	if !metadata.is_dir() {
		return Err(CodamaError::UnsafeOutput {
			path: root.to_path_buf(),
			reason: "generation output targets must be directories".to_owned(),
		});
	}

	for entry in walkdir::WalkDir::new(path).follow_links(false).min_depth(1) {
		let entry =
			entry.map_err(|source| create_dir_error(path, std::io::Error::other(source)))?;
		let metadata = std::fs::symlink_metadata(entry.path())
			.map_err(|source| create_dir_error(entry.path(), source))?;
		if is_link_like(&metadata) {
			return Err(unsafe_output_link(root, entry.path()));
		}
	}

	Ok(())
}

fn unsafe_output_link(root: &Path, link: &Path) -> CodamaError {
	CodamaError::UnsafeOutput {
		path: root.to_path_buf(),
		reason: format!(
			"generation output trees cannot contain symbolic link {}",
			link.display()
		),
	}
}

fn create_dir_error(path: &Path, source: std::io::Error) -> CodamaError {
	CodamaError::CreateDir {
		path: path.to_path_buf(),
		source,
	}
}

fn is_link_like(metadata: &std::fs::Metadata) -> bool {
	if metadata.file_type().is_symlink() {
		return true;
	}

	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		return has_windows_reparse_attribute(metadata.file_attributes());
	}

	#[cfg(not(windows))]
	false
}

#[cfg(windows)]
const fn has_windows_reparse_attribute(attributes: u32) -> bool {
	const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

	attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
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

	if plan.clients.contains(&ClientLanguage::Cpi) {
		paths.push(plan.cpi_out.as_path());
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
	if idl_paths.is_empty() {
		return Err(CodamaError::NoPrograms);
	}

	let output = if Path::new(&plan.npx).file_stem() == Some(OsStr::new("node")) {
		run_client_generation_with_node(plan, renderer, idl_paths, OsStr::new(&plan.npx))?
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

	if output.status.success() {
		return Ok(());
	}

	let cmd = if plan.npx == "npx" {
		"npx (or fallback pnpm dlx)".to_string()
	} else {
		plan.npx.clone()
	};

	Err(command_error(cmd, &output))
}

fn command_error(cmd: String, output: &BoundedOutput) -> CodamaError {
	let stderr = diagnostic_text(&output.stderr);
	let stdout = diagnostic_text(&output.stdout);
	let details = if !stderr.is_empty() {
		format!(": {stderr}")
	} else if !stdout.is_empty() {
		format!(": {stdout}")
	} else if output.status.code().is_none() {
		format!(": {}", output.status)
	} else {
		String::new()
	};

	CodamaError::CommandFailed {
		cmd,
		status: output.status.code().unwrap_or(-1),
		details,
	}
}

fn diagnostic_text(bytes: &[u8]) -> String {
	String::from_utf8_lossy(bytes)
		.trim()
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

fn run_client_generation_with_npx(
	plan: &GenerationPlan,
	renderer: ClientLanguage,
	idl_paths: &[PathBuf],
) -> std::io::Result<BoundedOutput> {
	let mut command = Command::new(&plan.npx);

	command.arg("-y").arg("-p").arg("codama@1.10.1");
	add_npx_renderer_package(&mut command, renderer);
	command
		.arg("node")
		.arg("--input-type=module")
		.arg("-")
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer));
	add_generation_arguments(&mut command, plan.generation[&renderer]);

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	run_bounded(&mut command, Some(CLIENT_RENDER_SCRIPT.as_bytes()))
}

fn run_client_generation_with_pnpm(
	plan: &GenerationPlan,
	renderer: ClientLanguage,
	idl_paths: &[PathBuf],
) -> Result<BoundedOutput, CodamaError> {
	let mut command = Command::new("pnpm");
	command.arg("dlx").arg("--package").arg("codama@1.10.1");
	add_pnpm_renderer_package(&mut command, renderer);
	command
		.arg("node")
		.arg("--input-type=module")
		.arg("-")
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer));
	add_generation_arguments(&mut command, plan.generation[&renderer]);

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	run_bounded(&mut command, Some(CLIENT_RENDER_SCRIPT.as_bytes())).map_err(|source| {
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
	node: &OsStr,
) -> Result<BoundedOutput, CodamaError> {
	let mut command = Command::new(node);
	command
		.arg("--input-type=module")
		.arg("-")
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer));
	add_generation_arguments(&mut command, plan.generation[&renderer]);

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	run_bounded(&mut command, Some(CLIENT_RENDER_SCRIPT.as_bytes())).map_err(|source| {
		CodamaError::RunCommand {
			cmd: "node".to_string(),
			source,
		}
	})
}

fn add_generation_arguments(command: &mut Command, settings: GenerationSettings) {
	command
		.arg(settings.mode.as_str())
		.arg(if settings.scaffold { "true" } else { "false" });
}

fn run_bounded(command: &mut Command, input: Option<&[u8]>) -> std::io::Result<BoundedOutput> {
	use std::io::Write;

	let mut child = command
		.stdin(if input.is_some() {
			Stdio::piped()
		} else {
			Stdio::null()
		})
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()?;
	let stdout = child
		.stdout
		.take()
		.ok_or_else(|| std::io::Error::other("renderer stdout pipe was not created"))?;
	let stderr = child
		.stderr
		.take()
		.ok_or_else(|| std::io::Error::other("renderer stderr pipe was not created"))?;

	let stdin = if input.is_some() {
		Some(
			child
				.stdin
				.take()
				.ok_or_else(|| std::io::Error::other("renderer stdin pipe was not created"))?,
		)
	} else {
		None
	};

	std::thread::scope(|scope| {
		let stdin = stdin.zip(input).map(|(mut stdin, input)| {
			scope.spawn(move || allow_closed_stdin(stdin.write_all(input)))
		});
		let stdout = scope.spawn(|| read_tail(stdout));
		let stderr = scope.spawn(|| read_tail(stderr));
		let status = child.wait()?;
		if let Some(stdin) = stdin {
			join_writer(stdin)?;
		}
		let stdout = join_reader(stdout)?;
		let stderr = join_reader(stderr)?;

		Ok(BoundedOutput {
			status,
			stdout,
			stderr,
		})
	})
}

fn allow_closed_stdin(result: std::io::Result<()>) -> std::io::Result<()> {
	match result {
		Ok(()) => Ok(()),
		Err(source) if source.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
		Err(source) => Err(source),
	}
}

fn join_writer(
	handle: std::thread::ScopedJoinHandle<'_, std::io::Result<()>>,
) -> std::io::Result<()> {
	handle
		.join()
		.map_err(|_| std::io::Error::other("renderer input writer panicked"))?
}

fn read_tail(reader: impl std::io::Read) -> std::io::Result<Vec<u8>> {
	const MAX_CAPTURE_BYTES: usize = 16 * 1024;

	let mut reader = std::io::BufReader::new(reader);
	let mut chunk = [0u8; 4096];
	let mut captured = Vec::with_capacity(MAX_CAPTURE_BYTES);

	loop {
		let read = std::io::Read::read(&mut reader, &mut chunk)?;

		if read == 0 {
			return Ok(captured);
		}

		captured.extend_from_slice(&chunk[..read]);

		if captured.len() > MAX_CAPTURE_BYTES {
			let overflow = captured.len() - MAX_CAPTURE_BYTES;
			captured.drain(..overflow);
		}
	}
}

fn join_reader(
	handle: std::thread::ScopedJoinHandle<'_, std::io::Result<Vec<u8>>>,
) -> std::io::Result<Vec<u8>> {
	handle
		.join()
		.map_err(|_| std::io::Error::other("renderer output reader panicked"))?
}

fn renderer_output(plan: &GenerationPlan, renderer: ClientLanguage) -> &Path {
	match renderer {
		ClientLanguage::Cpi => &plan.cpi_out,
		ClientLanguage::Typescript => &plan.typescript_out,
		ClientLanguage::Dart => &plan.dart_out,
		ClientLanguage::Rust => &plan.rust_out,
	}
}

fn add_npx_renderer_package(command: &mut Command, renderer: ClientLanguage) {
	command.arg("-p");

	match renderer {
		ClientLanguage::Cpi | ClientLanguage::Rust => command.arg("codama@1.10.1"),
		ClientLanguage::Typescript => command.arg("@codama/renderers-js@2.3.1"),
		ClientLanguage::Dart => command.arg("codama-renderers-dart@0.5.5"),
	};
}

fn add_pnpm_renderer_package(command: &mut Command, renderer: ClientLanguage) {
	command.arg("--package");

	match renderer {
		ClientLanguage::Cpi | ClientLanguage::Rust => command.arg("codama@1.10.1"),
		ClientLanguage::Typescript => command.arg("@codama/renderers-js@2.3.1"),
		ClientLanguage::Dart => command.arg("codama-renderers-dart@0.5.5"),
	};
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet;
	use std::io::Cursor;
	use std::process::Command;

	use super::*;

	fn output(status: ExitStatus, stdout: &[u8], stderr: &[u8]) -> BoundedOutput {
		BoundedOutput {
			status,
			stdout: stdout.to_vec(),
			stderr: stderr.to_vec(),
		}
	}

	fn empty_plan(npx: impl Into<String>) -> GenerationPlan {
		GenerationPlan {
			programs: Vec::new(),
			override_idl_names: false,
			idls_dir: PathBuf::from("idl"),
			rust_out: PathBuf::from("rust"),
			cpi_out: PathBuf::from("cpi"),
			typescript_out: PathBuf::from("typescript"),
			dart_out: PathBuf::from("dart"),
			clients: BTreeSet::new(),
			generation: default_generation_settings(),
			npx: npx.into(),
		}
	}

	#[cfg(unix)]
	fn exit_status(code: i32) -> ExitStatus {
		use std::os::unix::process::ExitStatusExt;

		ExitStatus::from_raw(code << 8)
	}

	#[cfg(windows)]
	fn exit_status(code: i32) -> ExitStatus {
		use std::os::windows::process::ExitStatusExt;

		ExitStatus::from_raw(code as u32)
	}

	#[test]
	fn command_failure_prefers_stderr_then_stdout() {
		let stderr = command_error(
			"renderer".to_owned(),
			&output(exit_status(2), b"stdout", b"stderr"),
		)
		.to_string();
		let stdout = command_error(
			"renderer".to_owned(),
			&output(exit_status(3), b"stdout", b""),
		)
		.to_string();
		let empty =
			command_error("renderer".to_owned(), &output(exit_status(4), b"", b"")).to_string();

		assert!(stderr.contains("stderr"));
		assert!(!stderr.contains("stdout"));
		assert!(stdout.contains("stdout"));
		assert!(empty.contains("status 4"));
	}

	#[test]
	fn command_failure_escapes_untrusted_terminal_controls() {
		let error = command_error(
			"renderer".to_owned(),
			&output(exit_status(2), b"", b"first\n\t\x1b[31msecond\xff\r\n"),
		)
		.to_string();

		assert!(error.contains("first\\n\\t\\u{1b}[31msecond�"));
		assert!(!error.chars().any(char::is_control));
	}

	#[test]
	fn bounded_reader_keeps_only_the_tail() {
		let bytes = vec![b'a'; 20 * 1024];
		let captured = read_tail(Cursor::new(bytes))
			.unwrap_or_else(|error| panic!("failed to capture output: {error}"));

		assert_eq!(captured.len(), 16 * 1024);
		assert!(captured.iter().all(|byte| *byte == b'a'));
	}

	#[test]
	fn bounded_runner_captures_stdout_and_stderr() {
		#[cfg(windows)]
		let mut command = {
			let mut command = Command::new("cmd");
			command.args(["/C", "echo stdout & echo stderr 1>&2 & exit /B 7"]);
			command
		};
		#[cfg(not(windows))]
		let mut command = {
			let mut command = Command::new("sh");
			command.args(["-c", "printf stdout; printf stderr >&2; exit 7"]);
			command
		};
		let captured = run_bounded(&mut command, None)
			.unwrap_or_else(|error| panic!("failed to run command: {error}"));

		assert_eq!(captured.status.code(), Some(7));
		assert_eq!(captured.stdout.trim_ascii_end(), b"stdout");
		assert!(String::from_utf8_lossy(&captured.stderr).contains("stderr"));
	}

	#[cfg(unix)]
	#[test]
	fn bounded_runner_preserves_failure_when_the_child_closes_stdin() {
		let mut command = Command::new("sh");
		command.args(["-c", "printf 'renderer failed' >&2; exit 9"]);
		let input = vec![b'x'; 1024 * 1024];
		let captured = run_bounded(&mut command, Some(&input))
			.unwrap_or_else(|error| panic!("failed to run command: {error}"));

		assert_eq!(captured.status.code(), Some(9));
		assert!(String::from_utf8_lossy(&captured.stderr).contains("renderer failed"));
	}

	#[test]
	fn stdin_writer_only_ignores_a_closed_child_pipe() {
		assert!(allow_closed_stdin(Ok(())).is_ok());
		assert!(
			allow_closed_stdin(Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))).is_ok()
		);

		let error = allow_closed_stdin(Err(std::io::Error::from(
			std::io::ErrorKind::PermissionDenied,
		)))
		.expect_err("unrelated stdin errors must be preserved");
		assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
	}

	#[test]
	fn selected_output_directories_follow_requested_clients() {
		let plan = GenerationPlan {
			programs: Vec::new(),
			override_idl_names: false,
			idls_dir: PathBuf::from("idl"),
			rust_out: PathBuf::from("rust"),
			cpi_out: PathBuf::from("cpi"),
			typescript_out: PathBuf::from("typescript"),
			dart_out: PathBuf::from("dart"),
			clients: [
				ClientLanguage::Cpi,
				ClientLanguage::Rust,
				ClientLanguage::Typescript,
				ClientLanguage::Dart,
			]
			.into_iter()
			.collect(),
			generation: default_generation_settings(),
			npx: "npx".to_owned(),
		};

		assert_eq!(
			selected_output_dirs(&plan),
			vec![
				Path::new("rust"),
				Path::new("cpi"),
				Path::new("typescript"),
				Path::new("dart")
			]
		);
		assert_eq!(
			renderer_output(&plan, ClientLanguage::Cpi),
			Path::new("cpi")
		);
		assert_eq!(
			renderer_output(&plan, ClientLanguage::Rust),
			Path::new("rust")
		);
	}

	#[cfg(unix)]
	#[test]
	fn client_runner_reports_spawn_and_renderer_failures() {
		use std::os::unix::fs::PermissionsExt;

		let missing = empty_plan("definitely-missing-pina-renderer-command");
		let idls = [PathBuf::from("program.json")];
		assert!(matches!(
			run_client_generation(&missing, ClientLanguage::Typescript, &idls),
			Err(CodamaError::RunCommand { .. })
		));

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let script = temp.path().join("node");
		std::fs::write(&script, "#!/bin/sh\nprintf 'renderer failed' >&2\nexit 9\n")
			.unwrap_or_else(|error| panic!("failed to write renderer: {error}"));
		std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
			.unwrap_or_else(|error| panic!("failed to make renderer executable: {error}"));
		let failing = empty_plan(script.to_string_lossy());
		let error = run_client_generation(&failing, ClientLanguage::Dart, &idls)
			.expect_err("renderer failure should be reported");

		assert!(error.to_string().contains("renderer failed"));
		let node_output = run_client_generation_with_node(
			&failing,
			ClientLanguage::Typescript,
			&idls,
			script.as_os_str(),
		)
		.unwrap_or_else(|error| panic!("failed to run fake node: {error}"));
		assert_eq!(node_output.status.code(), Some(9));
		assert!(matches!(
			run_client_generation_with_node(
				&failing,
				ClientLanguage::Typescript,
				&idls,
				OsStr::new("definitely-missing-node"),
			),
			Err(CodamaError::RunCommand { .. })
		));
	}

	#[test]
	fn renderer_command_helpers_cover_each_language() {
		for (language, package) in [
			(ClientLanguage::Cpi, "codama@1.10.1"),
			(ClientLanguage::Rust, "codama@1.10.1"),
			(ClientLanguage::Typescript, "@codama/renderers-js@2.3.1"),
			(ClientLanguage::Dart, "codama-renderers-dart@0.5.5"),
		] {
			let mut npx = Command::new("npx");
			add_npx_renderer_package(&mut npx, language);
			assert_eq!(
				npx.get_args().collect::<Vec<_>>(),
				[OsStr::new("-p"), OsStr::new(package)],
			);

			let mut pnpm = Command::new("pnpm");
			add_pnpm_renderer_package(&mut pnpm, language);
			assert_eq!(
				pnpm.get_args().collect::<Vec<_>>(),
				[OsStr::new("--package"), OsStr::new(package)],
			);
		}
	}

	#[test]
	fn renderer_rejects_an_empty_idl_set_without_spawning() {
		assert!(matches!(
			run_client_generation(
				&empty_plan("definitely-missing-pina-renderer-command"),
				ClientLanguage::Typescript,
				&[],
			),
			Err(CodamaError::NoPrograms)
		));
	}

	#[test]
	fn cpi_renderer_failures_preserve_the_output_path() {
		let output = Path::new("clients/cpi");
		let error = render_cpi_client(
			Path::new("missing-cpi-idl.json"),
			output,
			&CpiRenderConfig::default(),
		)
		.expect_err("missing IDL should fail");

		assert!(matches!(error, CodamaError::RenderCpi { path, .. } if path == output));
	}

	#[test]
	fn create_directory_error_preserves_the_target() {
		let error = create_dir_error(Path::new("clients"), std::io::Error::other("failure"));

		assert!(matches!(error, CodamaError::CreateDir { .. }));
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let file = temp.path().join("file");
		std::fs::write(&file, b"blocked")
			.unwrap_or_else(|error| panic!("failed to create blocking file: {error}"));
		assert!(matches!(
			create_output_dir(&file),
			Err(CodamaError::CreateDir { .. })
		));
	}

	#[test]
	fn generation_target_modes_enforce_destination_state_and_renderer_mapping() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let nonempty = temp.path().join("nonempty");
		std::fs::create_dir_all(&nonempty)
			.unwrap_or_else(|error| panic!("failed to create destination: {error}"));
		std::fs::write(nonempty.join("keep"), "keep")
			.unwrap_or_else(|error| panic!("failed to populate destination: {error}"));
		assert!(matches!(
			validate_generation_target(
				&nonempty,
				GenerationSettings {
					mode: GenerationMode::Create,
					scaffold: true,
				},
			),
			Err(CodamaError::InvalidGenerationState { .. })
		));

		let empty = temp.path().join("empty");
		std::fs::create_dir_all(&empty)
			.unwrap_or_else(|error| panic!("failed to create empty destination: {error}"));
		assert!(matches!(
			validate_generation_target(
				&empty,
				GenerationSettings {
					mode: GenerationMode::Update,
					scaffold: true,
				},
			),
			Err(CodamaError::InvalidGenerationState { .. })
		));

		let missing = temp.path().join("missing");
		validate_generation_target(
			&missing,
			GenerationSettings {
				mode: GenerationMode::Overwrite,
				scaffold: false,
			},
		)
		.unwrap_or_else(|error| panic!("missing overwrite target should be safe: {error}"));

		let git_tree = temp.path().join("git-tree");
		std::fs::create_dir_all(git_tree.join(".git"))
			.unwrap_or_else(|error| panic!("failed to create Git marker: {error}"));
		assert!(matches!(
			validate_generation_target(
				&git_tree,
				GenerationSettings {
					mode: GenerationMode::Overwrite,
					scaffold: true,
				},
			),
			Err(CodamaError::UnsafeOutput { .. })
		));

		let file = temp.path().join("file");
		std::fs::write(&file, "file")
			.unwrap_or_else(|error| panic!("failed to create file fixture: {error}"));
		assert!(
			!destination_is_empty(&file)
				.unwrap_or_else(|error| panic!("file state should be observable: {error}"))
		);

		for (mode, rust, cpi) in [
			(
				GenerationMode::Auto,
				RustRenderMode::Auto,
				CpiRenderMode::Auto,
			),
			(
				GenerationMode::Create,
				RustRenderMode::Create,
				CpiRenderMode::Create,
			),
			(
				GenerationMode::Update,
				RustRenderMode::Update,
				CpiRenderMode::Update,
			),
			(
				GenerationMode::Overwrite,
				RustRenderMode::Overwrite,
				CpiRenderMode::Overwrite,
			),
		] {
			assert_eq!(rust_render_mode(mode), rust);
			assert_eq!(cpi_render_mode(mode), cpi);
		}
	}

	#[cfg(unix)]
	#[test]
	fn destination_state_reports_unreadable_paths() {
		use std::os::unix::fs::PermissionsExt;

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let blocked = temp.path().join("blocked");
		std::fs::create_dir_all(&blocked)
			.unwrap_or_else(|error| panic!("failed to create blocked parent: {error}"));
		std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))
			.unwrap_or_else(|error| panic!("failed to block parent: {error}"));
		let result = destination_is_empty(&blocked.join("child"));
		std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700))
			.unwrap_or_else(|error| panic!("failed to restore parent: {error}"));

		assert!(matches!(result, Err(CodamaError::CreateDir { .. })));
	}

	#[test]
	fn generation_plan_reports_directory_and_idl_failures() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let temp_root = std::fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize temp dir: {error}"));
		let blocked_idls = temp_root.join("blocked-idls");
		std::fs::write(&blocked_idls, b"file")
			.unwrap_or_else(|error| panic!("failed to block IDL directory: {error}"));
		let mut plan = empty_plan("npx");
		plan.idls_dir = blocked_idls;
		let error = generate_plan(&plan).expect_err("blocked Rust output should fail");
		assert!(
			matches!(error, CodamaError::CreateDir { .. }),
			"unexpected error: {error}"
		);

		plan.idls_dir = temp_root.join("idls");
		plan.rust_out = temp_root.join("blocked-rust");
		plan.clients.insert(ClientLanguage::Rust);
		std::fs::write(&plan.rust_out, b"file")
			.unwrap_or_else(|error| panic!("failed to block Rust directory: {error}"));
		let error = generate_plan(&plan).expect_err("blocked Rust output should fail");
		assert!(
			matches!(error, CodamaError::UnsafeOutput { .. }),
			"unexpected error: {error}"
		);

		plan.clients.clear();
		plan.programs = vec![("missing".to_owned(), temp_root.join("missing-program"))];
		assert!(matches!(
			generate_plan(&plan),
			Err(CodamaError::GenerateIdl { .. })
		));
	}

	#[test]
	fn generation_plan_reports_idl_write_and_rust_render_failures() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let temp_root = std::fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize temp dir: {error}"));
		let program = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello_solana");
		let mut plan = empty_plan("npx");
		plan.idls_dir = temp_root.join("idls");
		plan.programs = vec![("hello_solana".to_owned(), program)];
		std::fs::create_dir_all(plan.idls_dir.join("hello_solana.json"))
			.unwrap_or_else(|error| panic!("failed to block IDL output: {error}"));
		assert!(matches!(
			generate_plan(&plan),
			Err(CodamaError::WriteIdl { .. })
		));

		std::fs::remove_dir_all(&plan.idls_dir)
			.unwrap_or_else(|error| panic!("failed to clear IDL output: {error}"));
		let invalid_idl = temp_root.join("invalid.json");
		std::fs::write(&invalid_idl, b"not json")
			.unwrap_or_else(|error| panic!("failed to write invalid IDL: {error}"));
		assert!(matches!(
			render_rust_client(
				&invalid_idl,
				&temp_root.join("rust"),
				&RenderConfig::default(),
			),
			Err(CodamaError::RenderRust { .. })
		));
	}

	#[cfg(unix)]
	#[test]
	fn command_failure_preserves_signal_status() {
		use std::os::unix::process::ExitStatusExt;

		let output = BoundedOutput {
			status: ExitStatus::from_raw(15),
			stdout: Vec::new(),
			stderr: Vec::new(),
		};
		let error = command_error("renderer".to_owned(), &output);
		let message = error.to_string();

		assert!(message.contains("signal"));
		assert!(message.contains("15"));
		assert!(message.contains("-1"));
	}

	#[cfg(unix)]
	#[test]
	fn render_target_allows_a_symlinked_prefix_but_rejects_the_target() {
		use std::os::unix::fs::symlink;

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let real = temp.path().join("real");
		let link = temp.path().join("link");
		std::fs::create_dir_all(&real)
			.unwrap_or_else(|error| panic!("failed to create real dir: {error}"));
		symlink(&real, &link).unwrap_or_else(|error| panic!("failed to create symlink: {error}"));

		validate_render_target(&link.join("generated"))
			.unwrap_or_else(|error| panic!("symlinked prefix should be allowed: {error}"));
		let error = validate_render_target(&link).expect_err("symlink target should be rejected");

		assert!(error.to_string().contains("symbolic link"));
	}

	#[cfg(unix)]
	#[test]
	fn render_target_rejects_a_symlink_inside_the_output_tree() {
		use std::os::unix::fs::symlink;

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let output = temp.path().join("clients");
		let external = temp.path().join("external");
		std::fs::create_dir_all(&output)
			.unwrap_or_else(|error| panic!("failed to create output dir: {error}"));
		std::fs::create_dir_all(&external)
			.unwrap_or_else(|error| panic!("failed to create external dir: {error}"));
		symlink(&external, output.join("escape"))
			.unwrap_or_else(|error| panic!("failed to create symlink: {error}"));

		let error = validate_render_target(&output)
			.expect_err("a link inside the deletion boundary should be rejected");
		assert!(error.to_string().contains("escape"));
	}

	#[test]
	fn render_target_rejects_filesystem_root() {
		#[cfg(windows)]
		let root = Path::new(r"C:\");
		#[cfg(not(windows))]
		let root = Path::new("/");
		let error = validate_render_target(root).expect_err("filesystem root should be rejected");

		assert!(error.to_string().contains("filesystem roots"));
	}

	#[test]
	fn render_target_reports_inspection_errors() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let invalid = temp.path().join("x".repeat(32 * 1024));
		assert!(matches!(
			validate_render_target(&invalid),
			Err(CodamaError::CreateDir { .. })
		));
	}

	#[cfg(windows)]
	#[test]
	fn windows_reparse_point_flag_is_detected() {
		assert!(has_windows_reparse_attribute(0x0400));
		assert!(has_windows_reparse_attribute(0x0410));
		assert!(!has_windows_reparse_attribute(0x0010));
	}
}
