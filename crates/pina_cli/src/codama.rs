use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

use pina_codama_renderer::RenderConfig;
use pina_codama_renderer::render_idl_file;

use crate::error::CodamaError;
use crate::generate_idl;

const JS_RENDER_SCRIPT: &str = r#"
import { renderVisitor as renderJsVisitor } from "@codama/renderers-js";
import { createFromJson } from "codama";
import { readFileSync } from "node:fs";
import { basename, join } from "node:path";

const [outputRoot, ...idlPaths] = process.argv.slice(1);

if (!outputRoot) {
	throw new Error("missing output root argument");
}

for (const idlPath of idlPaths.sort()) {
	const name = basename(idlPath, ".json");
	const json = readFileSync(idlPath, "utf8");
	const codama = createFromJson(json);
	await codama.accept(
		renderJsVisitor(join(outputRoot, name), {
			formatCode: false,
			deleteFolderBeforeRendering: true,
		}),
	);
}
"#;

#[derive(Debug, Clone)]
pub struct CodamaGenerateOptions {
	pub examples_dir: PathBuf,
	pub idls_dir: PathBuf,
	pub rust_out: PathBuf,
	pub js_out: PathBuf,
	pub examples: Vec<String>,
	pub npx: String,
}

pub fn generate_codama(options: &CodamaGenerateOptions) -> Result<Vec<String>, CodamaError> {
	let examples = collect_examples(options)?;

	std::fs::create_dir_all(&options.idls_dir).map_err(|source| {
		CodamaError::CreateDir {
			path: options.idls_dir.clone(),
			source,
		}
	})?;
	std::fs::create_dir_all(&options.rust_out).map_err(|source| {
		CodamaError::CreateDir {
			path: options.rust_out.clone(),
			source,
		}
	})?;
	std::fs::create_dir_all(&options.js_out).map_err(|source| {
		CodamaError::CreateDir {
			path: options.js_out.clone(),
			source,
		}
	})?;

	let mut idl_paths = Vec::with_capacity(examples.len());
	for example in &examples {
		let program_path = options.examples_dir.join(example);
		let idl = generate_idl(&program_path, None).map_err(|source| {
			CodamaError::GenerateIdl {
				example: example.clone(),
				path: program_path,
				source,
			}
		})?;
		let idl_json = serde_json::to_string_pretty(&idl).map_err(|source| {
			CodamaError::SerializeIdl {
				example: example.clone(),
				source,
			}
		})?;

		let idl_path = options.idls_dir.join(format!("{example}.json"));
		std::fs::write(&idl_path, idl_json).map_err(|source| {
			CodamaError::WriteIdl {
				path: idl_path.clone(),
				source,
			}
		})?;
		idl_paths.push(idl_path);
	}

	let render_config = RenderConfig::default();
	for (example, idl_path) in examples.iter().zip(idl_paths.iter()) {
		let crate_dir = options.rust_out.join(example);
		render_idl_file(idl_path, &crate_dir, &render_config).map_err(|source| {
			CodamaError::RenderRust {
				path: crate_dir,
				source,
			}
		})?;
	}

	run_js_generation(options, &idl_paths)?;

	Ok(examples)
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

	let mut selected = options
		.examples
		.iter()
		.cloned()
		.collect::<BTreeSet<_>>()
		.into_iter()
		.collect::<Vec<_>>();
	selected.sort();
	Ok(selected)
}

fn run_js_generation(
	options: &CodamaGenerateOptions,
	idl_paths: &[PathBuf],
) -> Result<(), CodamaError> {
	let output = if options.npx == "node" {
		run_js_generation_with_node(options, idl_paths)?
	} else {
		match run_js_generation_with_npx(options, idl_paths) {
			Ok(output) => output,
			Err(source)
				if source.kind() == std::io::ErrorKind::NotFound && options.npx == "npx" =>
			{
				run_js_generation_with_pnpm(options, idl_paths)?
			}
			Err(source) => {
				return Err(CodamaError::RunCommand {
					cmd: options.npx.clone(),
					source,
				});
			}
		}
	};

	if output.status.success() {
		return Ok(());
	}

	let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
	let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
	let details = if !stderr.is_empty() {
		format!(": {stderr}")
	} else if !stdout.is_empty() {
		format!(": {stdout}")
	} else {
		String::new()
	};

	Err(CodamaError::CommandFailed {
		cmd: if options.npx == "npx" {
			"npx (or fallback pnpm dlx)".to_string()
		} else {
			options.npx.clone()
		},
		status: output.status.code().unwrap_or(-1),
		details,
	})
}

fn run_js_generation_with_npx(
	options: &CodamaGenerateOptions,
	idl_paths: &[PathBuf],
) -> std::io::Result<Output> {
	let mut command = Command::new(&options.npx);
	command
		.arg("-y")
		.arg("-p")
		.arg("codama@1.5.1")
		.arg("-p")
		.arg("@codama/renderers-js@2.0.2")
		.arg("node")
		.arg("--input-type=module")
		.arg("-e")
		.arg(JS_RENDER_SCRIPT)
		.arg(&options.js_out);

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	command.output()
}

fn run_js_generation_with_pnpm(
	options: &CodamaGenerateOptions,
	idl_paths: &[PathBuf],
) -> Result<Output, CodamaError> {
	let mut command = Command::new("pnpm");
	command
		.arg("dlx")
		.arg("--package")
		.arg("codama@1.5.1")
		.arg("--package")
		.arg("@codama/renderers-js@2.0.2")
		.arg("node")
		.arg("--input-type=module")
		.arg("-e")
		.arg(JS_RENDER_SCRIPT)
		.arg(&options.js_out);

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	command.output().map_err(|source| {
		CodamaError::RunCommand {
			cmd: "pnpm".to_string(),
			source,
		}
	})
}

fn run_js_generation_with_node(
	options: &CodamaGenerateOptions,
	idl_paths: &[PathBuf],
) -> Result<Output, CodamaError> {
	let mut command = Command::new("node");
	command
		.arg("--input-type=module")
		.arg("-e")
		.arg(JS_RENDER_SCRIPT)
		.arg(&options.js_out);

	for idl_path in idl_paths {
		command.arg(idl_path);
	}

	command.output().map_err(|source| {
		CodamaError::RunCommand {
			cmd: "node".to_string(),
			source,
		}
	})
}
