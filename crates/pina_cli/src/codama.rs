use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;

use atomic_write_file::AtomicWriteFile;
use pina_cli_renderer::RenderConfig as CliRenderConfig;
use pina_cli_renderer::RenderMode as CliRenderMode;
use pina_cli_renderer::render_idl_file as render_cli_idl_file;
use pina_codama_renderer::RenderConfig;
use pina_codama_renderer::RenderMode as RustRenderMode;
use pina_codama_renderer::render_idl_file;
use pina_cpi_renderer::RenderConfig as CpiRenderConfig;
use pina_cpi_renderer::RenderMode as CpiRenderMode;
use pina_cpi_renderer::ScaffoldDependency as CpiScaffoldDependency;
use pina_cpi_renderer::render_idl_file as render_cpi_idl_file;

use crate::dart_client::harden_generated_dart_clients;
use crate::dart_client::validate_dart_client_idls;
use crate::dart_client::write_dart_package_barrels;
use crate::error::CodamaError;
use crate::generate_idl;
use crate::js_client::harden_generated_clients;
use crate::project::ClientLanguage;
use crate::project::GenerationMode;
use crate::project::Project;

/// The `codama` core package installed for renderers that need only the
/// runtime. Pinned so an offline or sandboxed `npx`/`pnpm dlx` run resolves
/// the same tree as everyone else.
const CODAMA_PACKAGE: &str = "codama@1.11.0";
/// The `@codama/renderers-js` line whose scaffolded clients target
/// `@solana/kit` 8.
const RENDERERS_JS_PACKAGE: &str = "@codama/renderers-js@2.5.0";
/// The `codama-renderers-dart` line matching [`CODAMA_PACKAGE`].
const RENDERERS_DART_PACKAGE: &str = "codama-renderers-dart@0.5.6";
/// The minimum `@solana/kit` major the generated TypeScript sources compile
/// against, matching the manifests [`RENDERERS_JS_PACKAGE`] scaffolds.
const MINIMUM_GENERATED_KIT_MAJOR: u16 = 8;

const CLIENT_RENDER_SCRIPT: &str = r#"
import { createFromJson, visit } from "codama";
import {
	copyFileSync,
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
import { basename, dirname, join, relative, sep } from "node:path";

const [renderer, outputRoot, clientRoot, requestedMode, scaffoldValue, ...idlPaths] = process.argv.slice(2);

if (!renderer || !outputRoot || !clientRoot || !requestedMode || !scaffoldValue) {
	throw new Error("missing renderer, output root, client root, mode, or scaffold argument");
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

		if (renderer === "cli-ts") {
			const { renderVisitor } = await import("@pina-rs/codama-renderer-cli");
			const appDir = join(stagingRoot, name);
			const clientDir = join(clientRoot, name);
			const clientImportPath = `../../../../js/${name}/src/generated/index`;
			await codama.accept(renderVisitor(appDir, { language: "typescript", clientImportPath }));
			continue;
		}

		if (renderer === "cli-dart") {
			const { renderVisitor } = await import("@pina-rs/codama-renderer-cli");
			const clientPackage = dartPackageName(clientRoot);
			await codama.accept(
				renderVisitor(stagingRoot, {
					language: "dart",
					packageName: dartPackageName(outputRoot) === "pina_codama_clients"
						? "pina_cli_apps"
						: dartPackageName(outputRoot),
					clientBarrel: `package:${clientPackage}/${name}.dart`,
				}),
			);
			continue;
		}

		throw new Error(`unknown renderer: ${renderer}`);
	}

	if (renderer === "typescript") {
		for (const name of names) {
			publishTypescript(name);
		}
	} else if (renderer === "cli-ts") {
		for (const name of names) {
			publishDirectory(join(stagingRoot, name), join(outputRoot, name));
		}
	} else if (renderer === "cli-dart") {
		publishDartCli();
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

function publishDartCli() {
	for (const name of names) {
		publishDirectory(
			join(stagingRoot, "lib", "src", name),
			join(outputRoot, "lib", "src", name),
		);
		// The Dart CLI package is shared by every program, so publishing the
		// whole staged bin directory would drop the entrypoints of programs
		// that are not part of this invocation.
		publishFile(
			join(stagingRoot, "bin", `${name}.dart`),
			join(outputRoot, "bin", `${name}.dart`),
		);
	}
	publishFile(
		join(stagingRoot, "lib", "src", "endpoint_guard.dart"),
		join(outputRoot, "lib", "src", "endpoint_guard.dart"),
	);
	publishFile(
		join(stagingRoot, "test", "endpoint_guard_test.dart"),
		join(outputRoot, "test", "endpoint_guard_test.dart"),
	);

	const manifest = join(outputRoot, "pubspec.yaml");
	if (scaffold && !existsSync(manifest)) {
		mkdirSync(outputRoot, { recursive: true });
		writeFileSync(manifest, dartCliManifest(), "utf8");
	}
}

function publishFile(staged, destination) {
	mkdirSync(dirname(destination), { recursive: true });
	copyFileSync(staged, destination);
}

function dartCliManifest() {
	const packageName = names.length === 1 ? `${names[0]}_cli` : "pina_cli_apps";
	const clientPackage = dartPackageName(clientRoot);
	const clientPath = relative(outputRoot, clientRoot).split(sep).join("/");
	const executables = names.map((name) => `  ${name.replace(/_/g, "-")}: ${name}`).join("\n");
	return `name: ${packageName}
description: CLI applications generated by Pina from Codama IDLs.
publish_to: none

environment:
  sdk: ">=3.10.0 <4.0.0"

executables:
${executables}

dependencies:
  args: ^2.7.0
  ${clientPackage}:
    path: ${clientPath}
  solana_kit_accounts: ">=0.10.0 <1.0.0"
  solana_kit_address: ">=0.10.0 <1.0.0"
  solana_kit_keys: ">=0.10.0 <1.0.0"
  solana_kit_rpc: ">=0.10.0 <1.0.0"
  solana_kit_transaction_messages: ">=0.10.0 <1.0.0"
  solana_kit_transactions: ">=0.10.0 <1.0.0"

dev_dependencies:
  test: ^1.25.0
`;
}

function dartPackageName(root) {
	const manifest = join(root, "pubspec.yaml");
	if (!existsSync(manifest)) {
		return "pina_codama_clients";
	}
	const match = readFileSync(manifest, "utf8").match(/^name:\s*(\S+)/m);
	return match ? match[1] : "pina_codama_clients";
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
  solana_kit_accounts: ">=0.10.0 <1.0.0"
  solana_kit_addresses: ">=0.10.0 <1.0.0"
  solana_kit_codecs_core: ">=0.10.0 <1.0.0"
  solana_kit_codecs_data_structures: ">=0.10.0 <1.0.0"
  solana_kit_codecs_numbers: ">=0.10.0 <1.0.0"
  solana_kit_codecs_strings: ">=0.10.0 <1.0.0"
  solana_kit_errors: ">=0.10.0 <1.0.0"
  solana_kit_instructions: ">=0.10.0 <1.0.0"
  solana_kit_rpc_types: ">=0.10.0 <1.0.0"
`;
}
"#;

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
	/// Maps a program name to the package name its generated CPI crate
	/// should use. Unset entries fall back to the snake-cased IDL name.
	cpi_package_names: BTreeMap<String, String>,
	idls_dir: PathBuf,
	rust_out: PathBuf,
	cpi_out: PathBuf,
	typescript_out: PathBuf,
	dart_out: PathBuf,
	cli_rust_out: PathBuf,
	cli_ts_out: PathBuf,
	cli_dart_out: PathBuf,
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
	let clients = expand_cli_clients(clients.into_iter().collect::<BTreeSet<_>>());
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
		cpi_package_names: BTreeMap::from([(
			project.library_name.clone(),
			project.package_name.clone(),
		)]),
		idls_dir: project.idl_dir.clone(),
		rust_out: client_target(ClientLanguage::Rust),
		cpi_out: client_target(ClientLanguage::Cpi),
		typescript_out: client_target(ClientLanguage::Typescript),
		dart_out: client_target(ClientLanguage::Dart),
		cli_rust_out: client_target(ClientLanguage::CliRust),
		cli_ts_out: client_target(ClientLanguage::CliTs),
		cli_dart_out: client_target(ClientLanguage::CliDart),
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

/// Add each selected CLI's base client and warn when several CLIs are picked.
/// Relative path from one directory to another, using only `..` and the
/// target's remaining components. Both inputs must be absolute or both
/// relative; the caller passes generation outputs, which share a root.
fn relative_dependency_path(from_dir: &Path, to_dir: &Path) -> String {
	let mut from = from_dir.components().peekable();
	let mut to = to_dir.components().peekable();
	while let (Some(left), Some(right)) = (from.peek(), to.peek()) {
		if left == right {
			from.next();
			to.next();
		} else {
			break;
		}
	}
	let mut parts: Vec<String> = Vec::new();
	for _ in from {
		parts.push("..".to_string());
	}
	for part in to {
		parts.push(part.as_os_str().to_string_lossy().into_owned());
	}
	if parts.is_empty() {
		parts.push(".".to_string());
	}
	parts.join("/")
}

fn expand_cli_clients(mut clients: BTreeSet<ClientLanguage>) -> BTreeSet<ClientLanguage> {
	let selected = [
		ClientLanguage::CliRust,
		ClientLanguage::CliTs,
		ClientLanguage::CliDart,
	]
	.into_iter()
	.filter(|language| clients.contains(language))
	.collect::<Vec<_>>();

	if selected.len() > 1 {
		let names = selected
			.iter()
			.map(|language| language.as_str())
			.collect::<Vec<_>>()
			.join(", ");
		eprintln!("warning: projects normally need one CLI; generating {names} anyway");
	}

	for language in selected {
		if let Some(base) = language.base_client() {
			clients.insert(base);
		}
	}
	clients
}

/// Read one program's checked-in event histories, naming the example on failure.
fn read_event_histories(
	example: &str,
	program_path: &Path,
) -> Result<crate::client_events::EventClientHistoryIndex, CodamaError> {
	crate::client_events::read_histories(program_path).map_err(|message| {
		CodamaError::EventHistories {
			example: example.to_owned(),
			path: program_path.to_path_buf(),
			message,
		}
	})
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
	let mut event_histories = Vec::with_capacity(plan.programs.len());
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
		event_histories.push(read_event_histories(example, program_path)?);
	}

	if plan.clients.contains(&ClientLanguage::Rust) {
		let settings = plan.generation[&ClientLanguage::Rust];

		for (index, (example, idl_path)) in examples.iter().zip(idl_paths.iter()).enumerate() {
			let crate_dir = plan.rust_out.join(example);
			validate_render_target(&crate_dir)?;
			let render_config = RenderConfig {
				mode: rust_render_mode(settings.mode),
				scaffold: settings.scaffold,
				event_histories: event_histories
					.get(index)
					.map(crate::client_events::EventClientHistoryIndex::renderer_histories)
					.unwrap_or_default(),
				..RenderConfig::default()
			};
			render_rust_client(idl_path, &crate_dir, &render_config)?;
		}
	}

	if plan.clients.contains(&ClientLanguage::Cpi) {
		let settings = plan.generation[&ClientLanguage::Cpi];

		for (example, idl_path) in examples.iter().zip(idl_paths.iter()) {
			let crate_dir = plan.cpi_out.join(example);
			validate_render_target(&crate_dir)?;
			let render_config = CpiRenderConfig {
				mode: cpi_render_mode(settings.mode),
				scaffold: settings.scaffold,
				package_name: plan.cpi_package_names.get(example).cloned(),
				// Generated example crates are workspace members, so they
				// inherit `pina` from the workspace like every other member.
				// Pinning a registry version here would add a second `pina`
				// to the graph and break `cargo kani -p pina`.
				scaffold_dependency: CpiScaffoldDependency::Workspace,
				..CpiRenderConfig::default()
			};
			render_cpi_client(idl_path, &crate_dir, &render_config)?;
		}
	}

	if plan.clients.contains(&ClientLanguage::Typescript) {
		let settings = plan.generation[&ClientLanguage::Typescript];

		for example in &examples {
			validate_generation_target(&plan.typescript_out.join(example), settings)?;
			verify_typescript_scaffold_kit_major(&plan.typescript_out.join(example), settings)?;
		}

		run_client_generation(plan, ClientLanguage::Typescript, &idl_paths)?;
		harden_generated_clients(
			&plan.typescript_out,
			&examples,
			&idl_paths,
			&event_histories,
		)?;
	}

	if plan.clients.contains(&ClientLanguage::Dart) {
		let settings = plan.generation[&ClientLanguage::Dart];
		validate_generation_target(&plan.dart_out, settings)?;
		verify_dart_scaffold_kit_range(&plan.dart_out.join("pubspec.yaml"), settings)?;

		for example in &examples {
			validate_render_target(&plan.dart_out.join("lib/src/generated").join(example))?;
		}

		validate_dart_client_idls(&plan.dart_out, &examples, &idl_paths)?;
		run_client_generation(plan, ClientLanguage::Dart, &idl_paths)?;
		harden_generated_dart_clients(&plan.dart_out, &examples, &idl_paths, &event_histories)?;
		write_dart_package_barrels(&plan.dart_out, &examples)?;
	}

	if plan.clients.contains(&ClientLanguage::CliRust) {
		let settings = plan.generation[&ClientLanguage::CliRust];

		for (example, idl_path) in examples.iter().zip(idl_paths.iter()) {
			let crate_dir = plan.cli_rust_out.join(example);
			validate_generation_target(&crate_dir, settings)?;
			let client_dir = plan.rust_out.join(example);
			let client_path = relative_dependency_path(&crate_dir, &client_dir);
			let client_package = rust_client_package(&client_dir, example);
			let render_config = CliRenderConfig {
				mode: cli_render_mode(settings.mode),
				scaffold: settings.scaffold,
				client_package,
				client_path,
			};
			render_cli_idl_file(idl_path, &crate_dir, &render_config).map_err(|source| {
				CodamaError::RenderCli {
					path: crate_dir.clone(),
					source,
				}
			})?;
		}
	}

	if plan.clients.contains(&ClientLanguage::CliTs) {
		let settings = plan.generation[&ClientLanguage::CliTs];

		for example in &examples {
			validate_generation_target(&plan.cli_ts_out.join(example), settings)?;
		}

		run_client_generation(plan, ClientLanguage::CliTs, &idl_paths)?;
	}

	if plan.clients.contains(&ClientLanguage::CliDart) {
		let settings = plan.generation[&ClientLanguage::CliDart];
		validate_generation_target(&plan.cli_dart_out, settings)?;
		verify_dart_scaffold_kit_range(&plan.cli_dart_out.join("pubspec.yaml"), settings)?;
		run_client_generation(plan, ClientLanguage::CliDart, &idl_paths)?;
	}

	Ok(idl_paths)
}

/// Read the package name of a generated Rust client crate, falling back to
/// the kebab convention used by fresh scaffolds.
fn rust_client_package(client_dir: &Path, example: &str) -> String {
	let manifest = client_dir.join("Cargo.toml");
	if let Ok(contents) = std::fs::read_to_string(&manifest) {
		for line in contents.lines() {
			if let Some(name) = line.strip_prefix("name = ") {
				return name.trim_matches('"').to_string();
			}
		}
	}
	format!("{}-client", example.replace('_', "-"))
}

const fn cli_render_mode(mode: GenerationMode) -> CliRenderMode {
	match mode {
		GenerationMode::Auto => CliRenderMode::Auto,
		GenerationMode::Create => CliRenderMode::Create,
		GenerationMode::Update => CliRenderMode::Update,
		GenerationMode::Overwrite => CliRenderMode::Overwrite,
	}
}

/// Refuse to regenerate a TypeScript client whose scaffold still pins an
/// older `@solana/kit` major than the generated sources need.
///
/// Scaffolded manifests are intentionally never rewritten by generation, so
/// a manifest created by an older renderer keeps its original pin forever.
/// Rendering new sources against it leaves the client uncompilable with no
/// diagnostic naming the cause; failing here turns that silent drift into an
/// actionable error. See [`MINIMUM_GENERATED_KIT_MAJOR`].
fn verify_typescript_scaffold_kit_major(
	path: &Path,
	settings: GenerationSettings,
) -> Result<(), CodamaError> {
	if settings.mode == GenerationMode::Overwrite {
		// `overwrite` removes the scaffold and re-creates it with the
		// current renderer's ranges, so there is nothing to defend.
		return Ok(());
	}

	let manifest_path = path.join("package.json");
	let Ok(manifest) = std::fs::read_to_string(&manifest_path) else {
		// A missing or unreadable manifest is either a fresh target or a
		// custom `--no-scaffold` layout; neither has a pin to defend.
		return Ok(());
	};

	let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&manifest) else {
		return Ok(());
	};

	let range = ["peerDependencies", "dependencies"]
		.iter()
		.find_map(|section| {
			parsed
				.get(*section)
				.and_then(|section| section.get("@solana/kit"))
				.and_then(serde_json::Value::as_str)
		});

	let Some(range) = range else {
		return Ok(());
	};

	if kit_range_major(range).is_some_and(|major| major < MINIMUM_GENERATED_KIT_MAJOR) {
		return Err(CodamaError::StaleKitScaffold {
			path: manifest_path,
			ecosystem: "TypeScript",
			range: format!("`@solana/kit` {range}"),
			minimum: MINIMUM_GENERATED_KIT_MAJOR.to_string(),
		});
	}

	Ok(())
}

/// Read the leading major out of a plain semver range (`^7.0.0`, `>=8`,
/// `8.x`); ranges that do not name a major (`*`, `workspace:*`) return
/// `None` and are never policed.
fn kit_range_major(range: &str) -> Option<u16> {
	let range = range
		.trim()
		.trim_start_matches(['^', '~', '>', '=', 'v', ' ']);
	let digits: &str = &range[..range.chars().take_while(char::is_ascii_digit).count()];
	digits.parse().ok()
}

/// Refuse to regenerate a Dart client whose scaffold still pins the Solana
/// Kit Dart packages below [`MINIMUM_GENERATED_KIT_DART`].
///
/// Like the TypeScript guard, this defends the once-created, never-rewritten
/// `pubspec.yaml`: a manifest from an older release keeps its original
/// ranges forever, so regeneration would pair fresh sources with
/// dependencies that cannot resolve them. The Dart floor is a range rather
/// than a major because the Solana Kit Dart packages are pre-1.0.
fn verify_dart_scaffold_kit_range(
	pubspec_path: &Path,
	settings: GenerationSettings,
) -> Result<(), CodamaError> {
	if settings.mode == GenerationMode::Overwrite {
		return Ok(());
	}

	let Ok(pubspec) = std::fs::read_to_string(pubspec_path) else {
		// A missing or unreadable manifest is either a fresh target or a
		// custom `--no-scaffold` layout; neither has a pin to defend.
		return Ok(());
	};

	for line in pubspec.lines() {
		let Some((name, range)) = line.split_once(':') else {
			continue;
		};
		if !name.trim().starts_with("solana_kit_") {
			continue;
		}
		let range = range.trim().trim_matches(['"', '\'']);
		if let Some((major, minor)) = dart_range_floor(range)
			&& (major, minor) < MINIMUM_GENERATED_KIT_DART
		{
			return Err(CodamaError::StaleKitScaffold {
				path: pubspec_path.to_path_buf(),
				ecosystem: "Dart",
				range: format!("{name}: {range}"),
				minimum: format!(
					"{}.{}",
					MINIMUM_GENERATED_KIT_DART.0, MINIMUM_GENERATED_KIT_DART.1
				),
			});
		}
	}

	Ok(())
}

/// Read the floor version out of a pubspec dependency range
/// (`^0.8.0`, `>=0.10.0 <1.0.0`, `0.9.0`); ranges that do not name a
/// version (`any`, `*`) return `None` and are never policed.
fn dart_range_floor(range: &str) -> Option<(u16, u16)> {
	let range = range.trim();
	let range = range
		.strip_prefix(">=")
		.unwrap_or(range)
		.trim_start_matches(['^', '~', '=', 'v', ' ']);
	let mut parts = range.split('.');
	let major = parts
		.next()?
		.chars()
		.take_while(char::is_ascii_digit)
		.collect::<String>()
		.parse()
		.ok()?;
	let minor = parts
		.next()?
		.chars()
		.take_while(char::is_ascii_digit)
		.collect::<String>()
		.parse()
		.ok()?;
	Some((major, minor))
}

/// The minimum Solana Kit Dart minor the generated Dart sources compile
/// against, matching the ranges the `dartManifest` scaffold writes.
const MINIMUM_GENERATED_KIT_DART: (u16, u16) = (0, 10);

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
	let has_link = crate::path_security::has_user_controlled_link_like_component(&absolute)
		.map_err(|source| create_dir_error(path, source))?;

	if absolute.parent().is_none() {
		return Err(CodamaError::UnsafeOutput {
			path: absolute,
			reason: "filesystem roots cannot be generation targets".to_owned(),
		});
	}

	if has_link {
		return Err(CodamaError::UnsafeOutput {
			path: absolute,
			reason: "generation output paths cannot traverse symbolic links".to_owned(),
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
	#[cfg(windows)]
	{
		use std::os::windows::fs::MetadataExt;

		metadata.file_type().is_symlink()
			|| has_windows_reparse_attribute(metadata.file_attributes())
	}

	#[cfg(not(windows))]
	metadata.file_type().is_symlink()
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

	if plan.clients.contains(&ClientLanguage::CliRust) {
		paths.push(plan.cli_rust_out.as_path());
	}

	if plan.clients.contains(&ClientLanguage::CliTs) {
		paths.push(plan.cli_ts_out.as_path());
	}

	if plan.clients.contains(&ClientLanguage::CliDart) {
		paths.push(plan.cli_dart_out.as_path());
	}

	paths
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

	command.arg("-y").arg("-p").arg(CODAMA_PACKAGE);
	add_npx_renderer_package(&mut command, renderer);
	command
		.arg("node")
		.arg("--input-type=module")
		.arg("-")
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer))
		.arg(client_root(plan, renderer));
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
	command.arg("dlx").arg("--package").arg(CODAMA_PACKAGE);
	add_pnpm_renderer_package(&mut command, renderer);
	command
		.arg("node")
		.arg("--input-type=module")
		.arg("-")
		.arg(renderer.as_str())
		.arg(renderer_output(plan, renderer))
		.arg(client_root(plan, renderer));
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
		.arg(renderer_output(plan, renderer))
		.arg(client_root(plan, renderer));
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

/// The generated-client root a CLI renderer resolves its imports against.
fn client_root(plan: &GenerationPlan, renderer: ClientLanguage) -> &Path {
	match renderer {
		ClientLanguage::CliTs | ClientLanguage::Typescript => &plan.typescript_out,
		ClientLanguage::CliDart | ClientLanguage::Dart => &plan.dart_out,
		ClientLanguage::Cpi | ClientLanguage::Rust | ClientLanguage::CliRust => &plan.cpi_out,
	}
}

fn renderer_output(plan: &GenerationPlan, renderer: ClientLanguage) -> &Path {
	match renderer {
		ClientLanguage::Cpi => &plan.cpi_out,
		ClientLanguage::Typescript => &plan.typescript_out,
		ClientLanguage::Dart => &plan.dart_out,
		ClientLanguage::Rust => &plan.rust_out,
		ClientLanguage::CliTs => &plan.cli_ts_out,
		ClientLanguage::CliDart => &plan.cli_dart_out,
		ClientLanguage::CliRust => &plan.cli_rust_out,
	}
}

fn add_npx_renderer_package(command: &mut Command, renderer: ClientLanguage) {
	command.arg("-p");

	match renderer {
		ClientLanguage::Cpi | ClientLanguage::Rust | ClientLanguage::CliRust => {
			command.arg(CODAMA_PACKAGE)
		}
		ClientLanguage::Typescript => command.arg(RENDERERS_JS_PACKAGE),
		ClientLanguage::Dart => command.arg(RENDERERS_DART_PACKAGE),
		ClientLanguage::CliTs | ClientLanguage::CliDart => command.arg(renderer_cli_package()),
	};
}

fn add_pnpm_renderer_package(command: &mut Command, renderer: ClientLanguage) {
	command.arg("--package");

	match renderer {
		ClientLanguage::Cpi | ClientLanguage::Rust | ClientLanguage::CliRust => {
			command.arg(CODAMA_PACKAGE)
		}
		ClientLanguage::Typescript => command.arg(RENDERERS_JS_PACKAGE),
		ClientLanguage::Dart => command.arg(RENDERERS_DART_PACKAGE),
		ClientLanguage::CliTs | ClientLanguage::CliDart => command.arg(renderer_cli_package()),
	};
}

/// `@pina-rs/codama-renderer-cli` is released in lockstep with this crate, so
/// the install spec has to track the crate version instead of a fixed pin.
fn renderer_cli_package() -> String {
	format!("@pina-rs/codama-renderer-cli@{}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet;
	use std::io::Cursor;
	use std::process::Command;

	use super::*;

	#[test]
	fn relative_dependency_path_walks_out_of_the_cli_tree() {
		// The CLI crate sits three levels below the clients root; the
		// dependency on the sibling rust client must climb exactly that far.
		let path = relative_dependency_path(
			&Path::new("codama/clients/cli/rust/example"),
			&Path::new("codama/clients/rust/example"),
		);
		assert_eq!(path, "../../../rust/example");
	}

	#[test]
	fn relative_dependency_path_siblings_and_identity() {
		// Only "out" is shared, so all three remaining components climb.
		let path = relative_dependency_path(
			&Path::new("out/cli/rust/example"),
			&Path::new("out/rust/example"),
		);
		assert_eq!(path, "../../../rust/example");

		// A from-dir that already prefixes the target only descends.
		let nested = relative_dependency_path(
			&Path::new("codama/clients"),
			&Path::new("codama/clients/rust/example"),
		);
		assert_eq!(nested, "rust/example");

		// Shared roots with no remainder fall back to an explicit ".".
		assert_eq!(
			relative_dependency_path(&Path::new("a/b"), &Path::new("a/b")),
			"."
		);
	}

	fn output(status: ExitStatus, stdout: &[u8], stderr: &[u8]) -> BoundedOutput {
		BoundedOutput {
			status,
			stdout: stdout.to_vec(),
			stderr: stderr.to_vec(),
		}
	}

	fn default_generation_settings() -> BTreeMap<ClientLanguage, GenerationSettings> {
		[
			ClientLanguage::Cpi,
			ClientLanguage::Rust,
			ClientLanguage::Typescript,
			ClientLanguage::Dart,
			ClientLanguage::CliRust,
			ClientLanguage::CliTs,
			ClientLanguage::CliDart,
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

	fn empty_plan(npx: impl Into<String>) -> GenerationPlan {
		GenerationPlan {
			programs: Vec::new(),
			override_idl_names: false,
			cpi_package_names: BTreeMap::new(),
			idls_dir: PathBuf::from("idl"),
			rust_out: PathBuf::from("rust"),
			cpi_out: PathBuf::from("cpi"),
			typescript_out: PathBuf::from("typescript"),
			dart_out: PathBuf::from("dart"),
			cli_rust_out: PathBuf::from("cli-rust"),
			cli_ts_out: PathBuf::from("cli-ts"),
			cli_dart_out: PathBuf::from("cli-dart"),
			clients: BTreeSet::new(),
			generation: default_generation_settings(),
			npx: npx.into(),
		}
	}

	fn write_kit_manifest(dir: &Path, kit_range: &str) {
		std::fs::create_dir_all(dir).unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		std::fs::write(
			dir.join("package.json"),
			format!(r#"{{"peerDependencies": {{"@solana/kit": "{kit_range}"}}}}"#),
		)
		.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
	}

	#[test]
	fn stale_kit_scaffold_fails_with_the_manifest_range_named() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let client = temp.path().join("counter_program");
		write_kit_manifest(&client, "^7.0.0");

		let error = verify_typescript_scaffold_kit_major(
			&client,
			GenerationSettings {
				mode: GenerationMode::Auto,
				scaffold: true,
			},
		)
		.expect_err("a Kit 7 scaffold must block regeneration");

		assert!(
			matches!(
				error,
				CodamaError::StaleKitScaffold {
					ref range,
					ecosystem: "TypeScript",
					ref minimum,
					..
				} if range == "`@solana/kit` ^7.0.0" && minimum == "8"
			),
			"unexpected error: {error}"
		);
		assert!(
			error
				.to_string()
				.contains("never rewrites an existing scaffold"),
			"the error must explain why the manifest is not updated: {error}"
		);
	}

	#[test]
	fn fresh_and_current_kit_scaffolds_pass_the_guard() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let settings = GenerationSettings {
			mode: GenerationMode::Auto,
			scaffold: true,
		};

		// A missing manifest is a fresh scaffold or a `--no-scaffold` target.
		verify_typescript_scaffold_kit_major(&temp.path().join("missing"), settings)
			.unwrap_or_else(|error| panic!("missing manifest must pass: {error}"));

		let current = temp.path().join("current");
		write_kit_manifest(&current, "^8.3.0");
		verify_typescript_scaffold_kit_major(&current, settings)
			.unwrap_or_else(|error| panic!("current manifest must pass: {error}"));

		// A user-raised pin is respected, never rewritten or rejected.
		let raised = temp.path().join("raised");
		write_kit_manifest(&raised, "^9.0.0");
		verify_typescript_scaffold_kit_major(&raised, settings)
			.unwrap_or_else(|error| panic!("raised manifest must pass: {error}"));

		// A dependency-section pin is checked too.
		let dependency = temp.path().join("dependency");
		std::fs::create_dir_all(&dependency)
			.unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		std::fs::write(
			dependency.join("package.json"),
			r#"{"dependencies": {"@solana/kit": "^7.2.0"}}"#,
		)
		.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
		assert!(matches!(
			verify_typescript_scaffold_kit_major(&dependency, settings),
			Err(CodamaError::StaleKitScaffold { .. })
		));
	}

	#[test]
	fn unreadable_and_rangeless_kit_pins_are_not_policed() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let settings = GenerationSettings {
			mode: GenerationMode::Auto,
			scaffold: true,
		};

		// Directory names must stay legal on every platform CI tests; `*`
		// is a valid macOS filename but not a Windows one.
		for (name, range) in [("workspace_star", "workspace:*"), ("star", "*")] {
			let client = temp.path().join(name);
			write_kit_manifest(&client, range);
			verify_typescript_scaffold_kit_major(&client, settings)
				.unwrap_or_else(|error| panic!("range {range} must pass: {error}"));
		}

		// A scaffold that does not pin `@solana/kit` at all has nothing to
		// police, even though the manifest itself parses.
		let kit_less = temp.path().join("kit_less");
		std::fs::create_dir_all(&kit_less)
			.unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		std::fs::write(kit_less.join("package.json"), br#"{"name": "js-client"}"#)
			.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
		verify_typescript_scaffold_kit_major(&kit_less, settings)
			.unwrap_or_else(|error| panic!("kit-less manifest must pass: {error}"));

		let malformed = temp.path().join("malformed");
		std::fs::create_dir_all(&malformed)
			.unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		std::fs::write(malformed.join("package.json"), b"not json")
			.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
		verify_typescript_scaffold_kit_major(&malformed, settings)
			.unwrap_or_else(|error| panic!("malformed manifest must pass: {error}"));
	}

	#[test]
	fn overwrite_mode_skips_the_kit_scaffold_guard() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let client = temp.path().join("counter_program");
		write_kit_manifest(&client, "^6.10.0");

		verify_typescript_scaffold_kit_major(
			&client,
			GenerationSettings {
				mode: GenerationMode::Overwrite,
				scaffold: true,
			},
		)
		.unwrap_or_else(|error| panic!("overwrite must skip the guard: {error}"));
	}

	#[test]
	fn kit_range_major_reads_the_leading_major_only() {
		assert_eq!(kit_range_major("^7.0.0"), Some(7));
		assert_eq!(kit_range_major(" ^8 "), Some(8));
		assert_eq!(kit_range_major(">=8"), Some(8));
		assert_eq!(kit_range_major("~6.10.0"), Some(6));
		assert_eq!(kit_range_major("8.x"), Some(8));
		assert_eq!(kit_range_major("workspace:*"), None);
		assert_eq!(kit_range_major("*"), None);
		assert_eq!(kit_range_major("latest"), None);
	}

	fn write_dart_pubspec(dir: &Path, kit_line: &str) {
		std::fs::create_dir_all(dir).unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		std::fs::write(
			dir.join("pubspec.yaml"),
			format!("name: example_client\ndependencies:\n  meta: ^1.16.0\n{kit_line}\n"),
		)
		.unwrap_or_else(|error| panic!("failed to write pubspec: {error}"));
	}

	#[test]
	fn stale_dart_kit_scaffold_fails_with_the_pubspec_range_named() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let client = temp.path().join("dart");
		write_dart_pubspec(&client, "  solana_kit_accounts: ^0.8.0");

		let error = verify_dart_scaffold_kit_range(
			&client.join("pubspec.yaml"),
			GenerationSettings {
				mode: GenerationMode::Auto,
				scaffold: true,
			},
		)
		.expect_err("a Kit Dart 0.8 scaffold must block regeneration");

		assert!(
			matches!(
				error,
				CodamaError::StaleKitScaffold {
					ecosystem: "Dart",
					ref range,
					ref minimum,
					..
				} if range == "  solana_kit_accounts: ^0.8.0" && minimum == "0.10"
			),
			"unexpected error: {error}"
		);
		assert!(
			error.to_string().contains("Dart"),
			"the error must name the ecosystem: {error}"
		);
	}

	#[test]
	fn current_dart_kit_ranges_pass_the_guard() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let settings = GenerationSettings {
			mode: GenerationMode::Auto,
			scaffold: true,
		};

		// A missing pubspec is a fresh target or a `--no-scaffold` layout.
		verify_dart_scaffold_kit_range(&temp.path().join("missing/pubspec.yaml"), settings)
			.unwrap_or_else(|error| panic!("missing pubspec must pass: {error}"));

		let client = temp.path().join("dart");
		write_dart_pubspec(&client, "  solana_kit_accounts: \">=0.10.0 <1.0.0\"");
		verify_dart_scaffold_kit_range(&client.join("pubspec.yaml"), settings)
			.unwrap_or_else(|error| panic!("current pubspec must pass: {error}"));

		// A user-raised floor is respected, never rewritten or rejected.
		let raised = temp.path().join("raised_dart");
		write_dart_pubspec(&raised, "  solana_kit_accounts: \">=0.11.0 <1.0.0\"");
		verify_dart_scaffold_kit_range(&raised.join("pubspec.yaml"), settings)
			.unwrap_or_else(|error| panic!("raised pubspec must pass: {error}"));
	}

	#[test]
	fn rangeless_and_overwrite_dart_scaffolds_are_not_policed() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let settings = GenerationSettings {
			mode: GenerationMode::Auto,
			scaffold: true,
		};

		for (name, kit_line) in [
			("any", "  solana_kit_accounts: any"),
			("star", "  solana_kit_accounts: ^1.0.0"),
		] {
			let client = temp.path().join(name);
			write_dart_pubspec(&client, kit_line);
			verify_dart_scaffold_kit_range(&client.join("pubspec.yaml"), settings)
				.unwrap_or_else(|error| panic!("{kit_line} must pass: {error}"));
		}

		let client = temp.path().join("no_kit");
		write_dart_pubspec(&client, "  args: ^2.7.0");
		verify_dart_scaffold_kit_range(&client.join("pubspec.yaml"), settings)
			.unwrap_or_else(|error| panic!("kit-less pubspec must pass: {error}"));

		let overwrite = GenerationSettings {
			mode: GenerationMode::Overwrite,
			scaffold: true,
		};
		let client = temp.path().join("overwrite");
		write_dart_pubspec(&client, "  solana_kit_accounts: ^0.8.0");
		verify_dart_scaffold_kit_range(&client.join("pubspec.yaml"), overwrite)
			.unwrap_or_else(|error| panic!("overwrite must skip the guard: {error}"));
	}

	#[test]
	fn dart_range_floor_reads_the_floor_version_only() {
		assert_eq!(dart_range_floor("^0.8.0"), Some((0, 8)));
		assert_eq!(dart_range_floor(">=0.10.0 <1.0.0"), Some((0, 10)));
		assert_eq!(dart_range_floor(" 0.9.0 "), Some((0, 9)));
		assert_eq!(dart_range_floor("any"), None);
		assert_eq!(dart_range_floor("*"), None);
	}

	#[test]
	fn quoted_dart_ranges_are_stripped_before_parsing() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let client = temp.path().join("dart");
		write_dart_pubspec(&client, "  solana_kit_accounts: '^0.8.0'");

		assert!(matches!(
			verify_dart_scaffold_kit_range(
				&client.join("pubspec.yaml"),
				GenerationSettings {
					mode: GenerationMode::Auto,
					scaffold: true,
				},
			),
			Err(CodamaError::StaleKitScaffold { .. })
		));
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
			cpi_package_names: BTreeMap::new(),
			idls_dir: PathBuf::from("idl"),
			rust_out: PathBuf::from("rust"),
			cpi_out: PathBuf::from("cpi"),
			typescript_out: PathBuf::from("typescript"),
			dart_out: PathBuf::from("dart"),
			cli_rust_out: PathBuf::from("cli-rust"),
			cli_ts_out: PathBuf::from("cli-ts"),
			cli_dart_out: PathBuf::from("cli-dart"),
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

	/// The exact packages are pinned here so a toolchain bump that changes
	/// which `@solana/kit` major fresh scaffolds receive cannot land
	/// silently.
	#[test]
	fn pinned_codama_packages_target_the_kit_8_line() {
		assert_eq!(CODAMA_PACKAGE, "codama@1.11.0");
		assert_eq!(RENDERERS_JS_PACKAGE, "@codama/renderers-js@2.5.0");
		assert_eq!(RENDERERS_DART_PACKAGE, "codama-renderers-dart@0.5.6");
	}

	#[test]
	fn renderer_command_helpers_cover_each_language() {
		let renderer_cli = renderer_cli_package();
		for (language, package) in [
			(ClientLanguage::Cpi, CODAMA_PACKAGE),
			(ClientLanguage::Rust, CODAMA_PACKAGE),
			(ClientLanguage::CliRust, CODAMA_PACKAGE),
			(ClientLanguage::Typescript, RENDERERS_JS_PACKAGE),
			(ClientLanguage::Dart, RENDERERS_DART_PACKAGE),
			(ClientLanguage::CliTs, renderer_cli.as_str()),
			(ClientLanguage::CliDart, renderer_cli.as_str()),
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
	fn renderer_cli_package_tracks_the_published_npm_version() {
		let manifest: serde_json::Value = serde_json::from_str(include_str!(
			"../../../packages/codama-renderer-cli/package.json"
		))
		.expect("renderer package manifest is valid JSON");
		let published = manifest["version"]
			.as_str()
			.expect("renderer package manifest declares a version");

		assert_eq!(
			renderer_cli_package(),
			format!("@pina-rs/codama-renderer-cli@{published}")
		);
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

	#[test]
	fn event_history_failures_name_the_example() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let program = temp.path().join("example");
		let migrations = program.join("migrations");
		std::fs::create_dir_all(&migrations)
			.unwrap_or_else(|error| panic!("failed to create migrations dir: {error}"));
		std::fs::write(migrations.join("manifest.json"), b"not json")
			.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));

		let error =
			read_event_histories("example", &program).expect_err("an invalid manifest must fail");
		assert!(matches!(error, CodamaError::EventHistories { .. }));
		let message = error.to_string();
		assert!(message.contains("example"), "unexpected message: {message}");
		assert!(
			message.contains("invalid migration manifest JSON"),
			"unexpected message: {message}"
		);

		let clean =
			Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello_solana_program");
		let histories = read_event_histories("hello_solana_program", &clean)
			.unwrap_or_else(|error| panic!("missing manifest should be empty: {error}"));
		assert!(histories.renderer_histories().is_empty());
	}

	#[cfg(unix)]
	#[test]
	fn generation_plan_reports_javascript_hardening_failures() {
		use std::os::unix::fs::PermissionsExt;

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let temp_root = std::fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize temp dir: {error}"));
		// A fake `node` that succeeds without rendering anything, so the plan
		// reaches client hardening with no generated tree to walk.
		let node = temp_root.join("node");
		std::fs::write(&node, "#!/bin/sh\nexit 0\n")
			.unwrap_or_else(|error| panic!("failed to write fake node: {error}"));
		std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755))
			.unwrap_or_else(|error| panic!("failed to make fake node executable: {error}"));

		let program =
			Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello_solana_program");
		let mut plan = empty_plan(node.to_string_lossy().into_owned());
		plan.idls_dir = temp_root.join("idls");
		plan.typescript_out = temp_root.join("typescript");
		plan.programs = vec![("hello_solana_program".to_owned(), program)];
		plan.clients.insert(ClientLanguage::Typescript);

		let error = generate_plan(&plan)
			.expect_err("missing generated client trees must fail JavaScript hardening");
		assert!(
			matches!(error, CodamaError::HardenJavaScript { .. }),
			"unexpected error: {error}"
		);
	}

	#[test]
	fn generation_plan_renders_cli_rust_clients_from_fresh_idls() {
		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let temp_root = std::fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("failed to canonicalize temp dir: {error}"));
		let program =
			Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello_solana_program");
		let mut plan = empty_plan("npx");
		plan.idls_dir = temp_root.join("idls");
		plan.rust_out = temp_root.join("rust");
		plan.cli_rust_out = temp_root.join("cli-rust");
		plan.programs = vec![("hello_solana_program".to_owned(), program)];
		plan.clients.insert(ClientLanguage::CliRust);

		let idl_paths =
			generate_plan(&plan).unwrap_or_else(|error| panic!("generation failed: {error}"));
		assert_eq!(
			idl_paths,
			vec![plan.idls_dir.join("hello_solana_program.json")]
		);

		// The scaffolded manifest pins the relative rust-client path, proving
		// the CliRust loop rendered with the conventional `../../rust` target.
		let manifest = std::fs::read_to_string(
			plan.cli_rust_out
				.join("hello_solana_program")
				.join("Cargo.toml"),
		)
		.unwrap_or_else(|error| panic!("scaffolded manifest: {error}"));
		assert!(
			manifest.contains(
				"hello-solana-program-client = { path = \"../../rust/hello_solana_program\" }",
			),
			"unexpected manifest:\n{manifest}"
		);
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
		let program =
			Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello_solana_program");
		let mut plan = empty_plan("npx");
		plan.idls_dir = temp_root.join("idls");
		plan.programs = vec![("hello_solana_program".to_owned(), program)];
		std::fs::create_dir_all(plan.idls_dir.join("hello_solana_program.json"))
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
	fn render_target_rejects_a_symlinked_prefix_and_target() {
		use std::os::unix::fs::symlink;

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let temp_root =
			std::fs::canonicalize(temp.path()).expect("temporary directory should resolve");
		let real = temp_root.join("real");
		let link = temp_root.join("link");
		std::fs::create_dir_all(&real)
			.unwrap_or_else(|error| panic!("failed to create real dir: {error}"));
		symlink(&real, &link).unwrap_or_else(|error| panic!("failed to create symlink: {error}"));

		let prefix_error = validate_render_target(&link.join("generated"))
			.expect_err("symlinked prefixes should be rejected");
		let error = validate_render_target(&link).expect_err("symlink target should be rejected");

		assert!(prefix_error.to_string().contains("symbolic link"));
		assert!(error.to_string().contains("symbolic link"));
	}

	#[cfg(unix)]
	#[test]
	fn render_target_rejects_a_symlink_inside_the_output_tree() {
		use std::os::unix::fs::symlink;

		let temp =
			tempfile::TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let temp_root =
			std::fs::canonicalize(temp.path()).expect("temporary directory should resolve");
		let output = temp_root.join("clients");
		let external = temp_root.join("external");
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
