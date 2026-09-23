//! Deterministic project and toolchain diagnostics for humans and agents.

use std::fmt::Write;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use serde::Serialize;

use crate::keys::inspect_program_id;
use crate::keys::read_keypair_program_id;
use crate::lint_driver::DriverOrigin;
use crate::project::ClientLanguage;
use crate::project::Project;

/// Versioned JSON schema emitted by `pina doctor --json`.
pub const DOCTOR_SCHEMA_VERSION: u8 = 1;

/// Overall diagnostic severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorStatus {
	/// Required inputs are available and no recommendations remain.
	Ok,
	/// The core toolchain works, but optional capabilities are missing.
	Warning,
	/// A required tool or valid Pina program project is missing.
	Error,
}

/// Status of one stable diagnostic check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
	/// The capability is ready.
	Pass,
	/// The capability is optional or has an actionable recommendation.
	Warn,
	/// A baseline requirement failed.
	Fail,
}

/// One agent-addressable diagnostic result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
	/// Stable identifier suitable for automation rules.
	pub id: String,
	/// Typed severity for this check.
	pub status: CheckStatus,
	/// Concise human-readable result without ANSI styling.
	pub message: String,
}

/// A discovered external tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDiagnostic {
	/// Executable name searched through `PATH`.
	pub name: &'static str,
	/// Whether the CLI requires the tool for baseline Rust development.
	pub required: bool,
	/// Whether the executable ran successfully.
	pub available: bool,
	/// First line returned by the version command.
	pub version: Option<String>,
}

/// Project paths and identity diagnosed by `pina doctor`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDiagnostic {
	/// Absolute package root.
	pub root: PathBuf,
	/// Cargo package name.
	pub package_name: String,
	/// Rust library entrypoint.
	pub source: PathBuf,
	/// Program ID found in source, when readable.
	pub program_id: Option<String>,
	/// Conventional SBF artifact path.
	pub artifact: PathBuf,
	/// Whether the SBF artifact exists.
	pub artifact_exists: bool,
	/// Conventional local program keypair path.
	pub keypair: PathBuf,
	/// Whether the keypair exists.
	pub keypair_exists: bool,
	/// Whether the keypair identity matches the source declaration.
	pub keypair_matches_source: Option<bool>,
	/// Client ecosystems selected by project configuration.
	pub clients: Vec<ClientLanguage>,
}

/// The lint-driver state reported by `pina doctor`.
///
/// Every field answers one question a user cannot otherwise answer: which
/// toolchain is active, which toolchain a driver must match, where the CLI
/// looked, and what to do when nothing was found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LintDriverDiagnostic {
	/// Release, host, and commit hash of the active compiler.
	pub active_toolchain: Option<String>,

	/// The nightly release Pina's shipped lints are developed against.
	pub expected_toolchain: String,

	/// The driver path `pina lint` would use, absent when nothing resolved.
	pub resolved_driver: Option<PathBuf>,

	/// How that driver would be obtained, absent when nothing resolved.
	pub resolved_origin: Option<String>,

	/// The toolchain the resolved driver was built for, absent when unknown
	/// or when no driver resolved.
	pub resolved_toolchain: Option<String>,

	/// Per-user cache directory searched for a negotiated driver.
	pub cache_directory: Option<PathBuf>,

	/// Driver path bundled next to the CLI.
	pub bundled_driver: Option<PathBuf>,

	/// Whether the bundled driver exists.
	pub bundled_driver_exists: bool,

	/// The one-line remedy when no driver is available.
	pub remedy: Option<String>,
}

/// Stable diagnostic report emitted by `pina doctor`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
	/// Report schema version for agent compatibility.
	pub schema_version: u8,
	/// Pina CLI package version.
	pub cli_version: &'static str,
	/// Aggregate severity.
	pub status: DoctorStatus,
	/// Project information, absent when discovery fails.
	pub project: Option<ProjectDiagnostic>,
	/// Lint-driver resolution state.
	pub lint_driver: LintDriverDiagnostic,
	/// External tool availability in stable display order.
	pub tools: Vec<ToolDiagnostic>,
	/// Typed checks with stable IDs for agent decisions.
	pub checks: Vec<DoctorCheck>,
	/// Actionable findings in stable display order.
	pub findings: Vec<String>,
}

impl DoctorReport {
	/// Whether this report should produce a successful process exit.
	pub fn is_usable(&self) -> bool {
		self.status != DoctorStatus::Error
	}

	/// Render stable, color-free terminal output.
	pub fn render_text(&self) -> String {
		let mut output = String::new();
		let _ = writeln!(output, "Pina doctor");
		let _ = writeln!(output, "CLI: {}", escape_controls(self.cli_version));

		if let Some(project) = &self.project {
			let root = project.root.to_string_lossy();
			let artifact = project.artifact.to_string_lossy();
			let keypair = project.keypair.to_string_lossy();
			let _ = writeln!(output, "Project: {}", escape_controls(&root));
			let _ = writeln!(
				output,
				"Package: {}",
				escape_controls(&project.package_name)
			);
			let program_id = project.program_id.as_deref().unwrap_or("unavailable");
			let _ = writeln!(output, "Program ID: {}", escape_controls(program_id));
			let _ = writeln!(
				output,
				"Artifact: {} ({})",
				escape_controls(&artifact),
				presence(project.artifact_exists)
			);
			let _ = writeln!(
				output,
				"Keypair: {} ({})",
				escape_controls(&keypair),
				presence(project.keypair_exists)
			);
		} else {
			let _ = writeln!(output, "Project: unavailable");
		}

		let lint_driver = &self.lint_driver;
		let _ = writeln!(output, "Lint driver:");
		let active = lint_driver
			.active_toolchain
			.as_deref()
			.unwrap_or("unavailable");
		let _ = writeln!(output, "  active toolchain: {}", escape_controls(active));
		let _ = writeln!(
			output,
			"  shipped-lint toolchain: {}",
			escape_controls(&lint_driver.expected_toolchain)
		);
		match (&lint_driver.resolved_driver, &lint_driver.resolved_origin) {
			(Some(path), Some(origin)) => {
				let path = path.to_string_lossy();
				let _ = writeln!(
					output,
					"  driver: {} ({})",
					escape_controls(&path),
					escape_controls(origin)
				);
			}
			_ => {
				let _ = writeln!(output, "  driver: unavailable");
			}
		}
		if let Some(toolchain) = &lint_driver.resolved_toolchain {
			let _ = writeln!(
				output,
				"  resolved toolchain: {}",
				escape_controls(toolchain)
			);
		}
		if let Some(cache) = &lint_driver.cache_directory {
			let cache = cache.to_string_lossy();
			let _ = writeln!(output, "  cache: {}", escape_controls(&cache));
		}
		if let Some(bundled) = &lint_driver.bundled_driver {
			let bundled = bundled.to_string_lossy();
			let _ = writeln!(
				output,
				"  bundled: {} ({})",
				escape_controls(&bundled),
				presence(lint_driver.bundled_driver_exists)
			);
		}
		if let Some(remedy) = &lint_driver.remedy {
			let _ = writeln!(output, "  remedy: {}", escape_controls(remedy));
		}

		let _ = writeln!(output, "Tools:");

		for tool in &self.tools {
			let requirement = if tool.required {
				"required"
			} else {
				"optional"
			};
			let version = tool.version.as_deref().unwrap_or("missing");
			let _ = writeln!(
				output,
				"  {} ({requirement}): {}",
				escape_controls(tool.name),
				escape_controls(version)
			);
		}

		let _ = writeln!(output, "Checks:");

		for check in &self.checks {
			let status = match check.status {
				CheckStatus::Pass => "pass",
				CheckStatus::Warn => "warn",
				CheckStatus::Fail => "fail",
			};
			let _ = writeln!(
				output,
				"  [{status}] {}: {}",
				escape_controls(&check.id),
				escape_controls(&check.message)
			);
		}

		if !self.findings.is_empty() {
			let _ = writeln!(output, "Findings:");

			for finding in &self.findings {
				let _ = writeln!(output, "  - {}", escape_controls(finding));
			}
		}

		let status = match self.status {
			DoctorStatus::Ok => "ok",
			DoctorStatus::Warning => "warning",
			DoctorStatus::Error => "error",
		};
		let _ = writeln!(output, "Status: {status}");

		output
	}
}

/// Diagnose the current Pina project and relevant ecosystem tools.
pub fn diagnose(start: &Path) -> DoctorReport {
	let mut findings = Vec::new();
	let mut checks = Vec::new();
	let project = diagnose_project(start, &mut findings, &mut checks);

	// The lint driver is resolved against the discovered project, because the
	// project's own rust-toolchain.toml selects the compiler a driver must
	// match. Without a project there is no toolchain to negotiate against, so
	// the diagnostic reports the state without claiming a resolution.
	let lint_root = project
		.as_ref()
		.map_or_else(|| start.to_path_buf(), |project| project.root.clone());
	let lint_driver = diagnose_lint_driver(&lint_root, &mut findings, &mut checks);

	let needs_node = project.as_ref().is_some_and(|project| {
		project
			.clients
			.iter()
			.any(|client| matches!(client, ClientLanguage::Typescript | ClientLanguage::Dart))
	});
	let tools = tool_specs(needs_node)
		.iter()
		.map(|spec| diagnose_tool(*spec))
		.collect::<Vec<_>>();

	for tool in &tools {
		checks.push(DoctorCheck {
			id: format!("tool.{}", tool.name),
			status: if tool.available {
				CheckStatus::Pass
			} else if tool.required {
				CheckStatus::Fail
			} else {
				CheckStatus::Warn
			},
			message: tool_message(tool),
		});
	}

	if needs_node {
		let renderer_available = tools
			.iter()
			.any(|tool| matches!(tool.name, "npx" | "pnpm") && tool.available);
		checks.push(DoctorCheck {
			id: "client.renderer".to_owned(),
			status: if renderer_available {
				CheckStatus::Pass
			} else {
				CheckStatus::Fail
			},
			message: if renderer_available {
				"an npx or pnpm renderer is available".to_owned()
			} else {
				"configured TypeScript or Dart clients require npx or pnpm".to_owned()
			},
		});
	}

	let has_failures = checks.iter().any(|check| check.status == CheckStatus::Fail);
	let missing_required_tools = tools.iter().any(|tool| tool.required && !tool.available);
	let missing_optional_tools = tools
		.iter()
		.any(|tool| !tool.required && !matches!(tool.name, "npx" | "pnpm") && !tool.available);

	if missing_required_tools {
		findings.push(
			"install the missing Rust/SBF prerequisites and ensure they are on PATH".to_owned(),
		);
	}

	if missing_optional_tools {
		findings.push(
			"optional Solana, Surfpool, and client tools unlock deployment and integration \
			 workflows"
				.to_owned(),
		);
	}

	let status = if project.is_none() || has_failures {
		DoctorStatus::Error
	} else if !findings.is_empty() {
		DoctorStatus::Warning
	} else {
		DoctorStatus::Ok
	};

	DoctorReport {
		schema_version: DOCTOR_SCHEMA_VERSION,
		cli_version: env!("CARGO_PKG_VERSION"),
		status,
		project,
		lint_driver,
		tools,
		checks,
		findings,
	}
}

/// Diagnose the lint-driver resolution state for `project_root`.
///
/// The check answers the question a failing `pina lint` leaves open: which
/// toolchain is active, whether a driver exists for it, and what to run when
/// one does not. It deliberately does not download anything — a diagnostic
/// that mutates a cache is not a diagnostic — so a missing driver is reported
/// as a warning with the exact command that fixes it.
fn diagnose_lint_driver(
	project_root: &Path,
	findings: &mut Vec<String>,
	checks: &mut Vec<DoctorCheck>,
) -> LintDriverDiagnostic {
	use crate::lint_driver::PINA_LINT_DRIVER_PATH;

	let expected_toolchain = crate::lint_driver::LINT_DRIVER_TOOLCHAIN.to_owned();
	let cache_directory =
		crate::lint_toolchain::cache_root().map(|root| root.join(env!("CARGO_PKG_VERSION")));
	let bundled_driver = std::env::current_exe()
		.ok()
		.map(|executable| executable.with_file_name(crate::lint_driver::driver_binary_name()));
	let bundled_driver_exists = bundled_driver
		.as_ref()
		.is_some_and(|path| crate::lint_driver::is_executable(path));
	let active_toolchain = crate::lint_toolchain::identify(project_root)
		.ok()
		.map(|identity| identity.to_string());

	// Preparing the driver without the source-build option only inspects
	// cache, bundle, and download. A download would mutate the cache, so only
	// the non-mutating half is reported here.
	let resolved = resolve_without_download(project_root);
	let resolved_toolchain = resolved_toolchain_for(
		resolved.as_ref().map(|(_, origin)| *origin),
		active_toolchain.as_deref(),
	);

	let (resolved_driver, resolved_origin, remedy) = if let Some((path, origin)) = resolved {
		(Some(path), Some(origin.as_str().to_owned()), None)
	} else {
		// An override that does not name an executable is the most confusing
		// state, because the user believes they already configured a driver.
		let remedy = if std::env::var_os(PINA_LINT_DRIVER_PATH).is_some() {
			format!(
				"`{PINA_LINT_DRIVER_PATH}` is set but does not point at an executable driver; \
				 unset it to negotiate a driver, or point it at one you built."
			)
		} else {
			"Run `pina lint --build-driver` to build the driver for the active toolchain, or `pina \
			 lint` to download the driver published for this CLI release."
				.to_owned()
		};
		findings.push(remedy.clone());
		(None, None, Some(remedy))
	};

	let status = if resolved_driver.is_some() {
		CheckStatus::Pass
	} else {
		// A missing driver blocks one command, not the whole project; the
		// project itself may build and deploy with no lints at all.
		CheckStatus::Warn
	};
	let message = resolved_driver.as_ref().map_or_else(
		|| {
			format!(
				"no driver resolved for the active toolchain; the expected release is \
				 {expected_toolchain}"
			)
		},
		|path| format!("resolved {}", path.to_string_lossy()),
	);
	checks.push(DoctorCheck {
		id: "lint.driver".to_owned(),
		status,
		message,
	});

	LintDriverDiagnostic {
		active_toolchain,
		expected_toolchain,
		resolved_driver,
		resolved_origin,
		resolved_toolchain,
		cache_directory,
		bundled_driver,
		bundled_driver_exists,
		remedy,
	}
}

/// Resolve a driver without mutating the cache.
///
/// Mirrors [`crate::lint_driver::prepare_driver`] for the sources that already
/// exist on disk. A download is intentionally excluded: `pina doctor` reports
/// state rather than changing it, and the remedy line names the command that
/// performs the download.
fn resolve_without_download(project_root: &Path) -> Option<(PathBuf, DriverOrigin)> {
	crate::lint_driver::resolve_existing(project_root)
		.ok()
		.flatten()
		.map(|driver| (driver.path, driver.origin))
}

/// Report the toolchain the resolved driver was built for.
///
/// A driver only loads against the compiler revision it was built with, so
/// the honest answer depends on how the driver was obtained. A cached,
/// downloaded, or source-built driver was produced for the active toolchain
/// by construction, a bundled driver was built for the nightly Pina's
/// shipped lints target even though the load probe proved it compatible
/// with the active compiler, and an override names an arbitrary local build
/// whose toolchain the CLI cannot know. Nothing resolved means there is no
/// toolchain to report.
fn resolved_toolchain_for(
	origin: Option<DriverOrigin>,
	active_toolchain: Option<&str>,
) -> Option<String> {
	match origin {
		Some(DriverOrigin::Cache | DriverOrigin::Downloaded | DriverOrigin::BuiltFromSource) => {
			active_toolchain.map(str::to_owned)
		}
		Some(DriverOrigin::Bundled) => Some(crate::lint_driver::LINT_DRIVER_TOOLCHAIN.to_owned()),
		Some(DriverOrigin::Override) | None => None,
	}
}

#[derive(Clone, Copy)]
struct ToolSpec {
	name: &'static str,
	required: bool,
	version_args: &'static [&'static str],
	accept_nonzero: bool,
}

const BASE_TOOL_SPECS: &[ToolSpec] = &[
	ToolSpec {
		name: "cargo",
		required: true,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "rustc",
		required: true,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "cargo-build-sbf",
		required: true,
		version_args: &["--version"],
		accept_nonzero: true,
	},
	ToolSpec {
		name: "surfpool",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "solana",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
];

const CLIENT_TOOL_SPECS: &[ToolSpec] = &[
	ToolSpec {
		name: "node",
		required: true,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "npx",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
	ToolSpec {
		name: "pnpm",
		required: false,
		version_args: &["--version"],
		accept_nonzero: false,
	},
];

fn tool_specs(needs_node: bool) -> Vec<ToolSpec> {
	let mut specs = BASE_TOOL_SPECS.to_vec();
	if needs_node {
		specs.extend_from_slice(CLIENT_TOOL_SPECS);
	}
	specs
}

fn diagnose_project(
	start: &Path,
	findings: &mut Vec<String>,
	checks: &mut Vec<DoctorCheck>,
) -> Option<ProjectDiagnostic> {
	let project = match Project::discover(start) {
		Ok(project) => project,
		Err(error) => {
			findings.push(error.to_string());
			checks.push(DoctorCheck {
				id: "project.discovery".to_owned(),
				status: CheckStatus::Fail,
				message: error.to_string(),
			});

			return None;
		}
	};
	checks.push(DoctorCheck {
		id: "project.discovery".to_owned(),
		status: CheckStatus::Pass,
		message: format!("found {}", project.root.display()),
	});
	checks.push(size_profile_check(&project));
	let artifact = project.sbf_artifact();
	let keypair = project.keypair();
	let artifact_metadata = inspect_metadata(&artifact, fs::symlink_metadata(&artifact));
	let keypair_metadata = inspect_metadata(&keypair, fs::symlink_metadata(&keypair));
	let artifact_exists = artifact_metadata
		.as_ref()
		.is_ok_and(|value| value.as_ref().is_some_and(fs::Metadata::is_file));
	let keypair_exists = keypair_metadata.as_ref().is_ok_and(Option::is_some);
	append_inspection_error(&artifact_metadata, findings);
	append_inspection_error(&keypair_metadata, findings);
	let program_id = match inspect_program_id(&project.library_source) {
		Ok(declaration) => {
			checks.push(DoctorCheck {
				id: "project.program-id".to_owned(),
				status: CheckStatus::Pass,
				message: format!("declared as {}", declaration.program_id),
			});
			Some(declaration.program_id)
		}
		Err(error) => {
			findings.push(error.to_string());
			checks.push(DoctorCheck {
				id: "project.program-id".to_owned(),
				status: CheckStatus::Fail,
				message: error.to_string(),
			});
			None
		}
	};
	let (keypair_valid, keypair_matches_source) = if keypair_exists {
		match read_keypair_program_id(&keypair) {
			Ok(keypair_program_id) => {
				(
					true,
					program_id
						.as_ref()
						.map(|program_id| program_id == &keypair_program_id),
				)
			}
			Err(error) => {
				findings.push(error.to_string());

				(false, None)
			}
		}
	} else {
		(false, None)
	};
	checks.push(artifact_check(
		&artifact,
		&artifact_metadata,
		artifact_exists,
	));
	checks.push(keypair_check(
		&keypair,
		&keypair_metadata,
		keypair_exists,
		keypair_valid,
	));

	if let Some(matches) = keypair_matches_source {
		checks.push(DoctorCheck {
			id: "project.identity-match".to_owned(),
			status: if matches {
				CheckStatus::Pass
			} else {
				CheckStatus::Warn
			},
			message: if matches {
				"source and keypair program IDs match".to_owned()
			} else {
				"source and keypair program IDs differ".to_owned()
			},
		});
	}

	if artifact_metadata.is_ok() && !artifact_exists {
		findings.push(format!(
			"build the SBF artifact expected at {}",
			artifact.display()
		));
	}

	if keypair_metadata.is_ok() && !keypair_exists {
		findings.push(format!(
			"create a local program keypair at {}",
			keypair.display()
		));
	}

	if keypair_matches_source == Some(false) {
		findings.push("run `pina keys sync` after reviewing the selected keypair".to_owned());
	}

	Some(ProjectDiagnostic {
		root: project.root,
		package_name: project.package_name,
		source: project.library_source,
		program_id,
		artifact,
		artifact_exists,
		keypair,
		keypair_exists,
		keypair_matches_source,
		clients: project.clients,
	})
}

#[allow(clippy::unnecessary_debug_formatting)]
fn inspect_metadata(
	path: &Path,
	result: std::io::Result<fs::Metadata>,
) -> Result<Option<fs::Metadata>, String> {
	match result {
		Ok(metadata) => Ok(Some(metadata)),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(error) => Err(format!("failed to inspect {path:?}: {error}")),
	}
}

fn tool_message(tool: &ToolDiagnostic) -> String {
	match (tool.available, tool.version.as_ref()) {
		(true, Some(version)) => format!("available: {version}"),
		(true, None) => "available (version not reported)".to_owned(),
		(false, _) => "not available on PATH".to_owned(),
	}
}

fn append_inspection_error(
	metadata: &Result<Option<fs::Metadata>, String>,
	findings: &mut Vec<String>,
) {
	if let Err(error) = metadata {
		findings.push(error.clone());
	}
}

fn artifact_check(
	path: &Path,
	metadata: &Result<Option<fs::Metadata>, String>,
	exists: bool,
) -> DoctorCheck {
	DoctorCheck {
		id: "project.artifact".to_owned(),
		status: if metadata.is_err() {
			CheckStatus::Fail
		} else if exists {
			CheckStatus::Pass
		} else {
			CheckStatus::Warn
		},
		message: metadata.as_ref().map_or_else(Clone::clone, |_| {
			format!("{} at {}", presence(exists), path.display())
		}),
	}
}

fn keypair_check(
	path: &Path,
	metadata: &Result<Option<fs::Metadata>, String>,
	exists: bool,
	valid: bool,
) -> DoctorCheck {
	DoctorCheck {
		id: "project.keypair".to_owned(),
		status: if metadata.is_err() {
			CheckStatus::Fail
		} else if valid {
			CheckStatus::Pass
		} else if exists {
			CheckStatus::Fail
		} else {
			CheckStatus::Warn
		},
		message: if let Err(error) = metadata {
			error.clone()
		} else if valid {
			format!("valid at {}", path.display())
		} else if exists {
			format!("invalid at {}", path.display())
		} else {
			format!("missing at {}", path.display())
		},
	}
}

fn diagnose_tool(spec: ToolSpec) -> ToolDiagnostic {
	let output = capture_command(Command::new(spec.name).args(spec.version_args));
	let Ok(output) = output else {
		return ToolDiagnostic {
			name: spec.name,
			required: spec.required,
			available: false,
			version: None,
		};
	};

	if !output.status.success() && !spec.accept_nonzero {
		return ToolDiagnostic {
			name: spec.name,
			required: spec.required,
			available: false,
			version: None,
		};
	}

	let bytes = if output.stdout.is_empty() {
		&output.stderr
	} else {
		&output.stdout
	};
	let version = sanitized_first_line(bytes);

	ToolDiagnostic {
		name: spec.name,
		required: spec.required,
		available: true,
		version,
	}
}

const TOOL_OUTPUT_LIMIT: usize = 4 * 1024;
const VERSION_TEXT_LIMIT: usize = 512;
const TOOL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct CapturedOutput {
	status: ExitStatus,
	stdout: Vec<u8>,
	stderr: Vec<u8>,
}

fn capture_command(command: &mut Command) -> std::io::Result<CapturedOutput> {
	capture_command_with_timeout(command, TOOL_TIMEOUT)
}

fn capture_command_with_timeout(
	command: &mut Command,
	timeout: Duration,
) -> std::io::Result<CapturedOutput> {
	let deadline = Instant::now() + timeout;
	let mut child = command
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()?;
	let stdout = child
		.stdout
		.take()
		.ok_or_else(|| std::io::Error::other("diagnostic command did not provide stdout"))?;
	let stderr = child
		.stderr
		.take()
		.ok_or_else(|| std::io::Error::other("diagnostic command did not provide stderr"))?;
	let stdout_reader = spawn_reader(stdout);
	let stderr_reader = spawn_reader(stderr);
	let status = wait_with_timeout(
		&mut child,
		deadline.saturating_duration_since(Instant::now()),
	)?;
	let stdout = receive_reader(&stdout_reader, deadline)?;
	let stderr = receive_reader(&stderr_reader, deadline)?;

	Ok(CapturedOutput {
		status,
		stdout,
		stderr,
	})
}

trait ManagedChild {
	fn try_wait_managed(&mut self) -> std::io::Result<Option<ExitStatus>>;
	fn kill_managed(&mut self) -> std::io::Result<()>;
	fn wait_managed(&mut self) -> std::io::Result<ExitStatus>;
}

impl ManagedChild for std::process::Child {
	fn try_wait_managed(&mut self) -> std::io::Result<Option<ExitStatus>> {
		self.try_wait()
	}

	fn kill_managed(&mut self) -> std::io::Result<()> {
		self.kill()
	}

	fn wait_managed(&mut self) -> std::io::Result<ExitStatus> {
		self.wait()
	}
}

fn wait_with_timeout(
	child: &mut impl ManagedChild,
	timeout: Duration,
) -> std::io::Result<ExitStatus> {
	let started = Instant::now();

	loop {
		match child.try_wait_managed() {
			Ok(Some(status)) => return Ok(status),
			Ok(None) => {}
			Err(error) => return cleanup_after_error(child, error),
		}

		if started.elapsed() >= timeout {
			let timeout_error = std::io::Error::new(
				std::io::ErrorKind::TimedOut,
				"diagnostic command exceeded the five-second timeout",
			);
			return cleanup_after_error(child, timeout_error);
		}

		std::thread::sleep(Duration::from_millis(10));
	}
}

fn cleanup_after_error<T>(
	child: &mut impl ManagedChild,
	original: std::io::Error,
) -> std::io::Result<T> {
	if let Err(kill) = child.kill_managed() {
		return Err(std::io::Error::other(format!(
			"{original}; child cleanup failed (kill: {kill})"
		)));
	}

	match child.wait_managed() {
		Ok(_) => Err(original),
		Err(wait) => {
			Err(std::io::Error::other(format!(
				"{original}; child cleanup failed (wait: {wait})"
			)))
		}
	}
}

fn drain_bounded(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
	let mut captured = Vec::with_capacity(TOOL_OUTPUT_LIMIT);
	let mut chunk = [0u8; 1024];

	loop {
		let read = reader.read(&mut chunk)?;
		if read == 0 {
			return Ok(captured);
		}

		let remaining = TOOL_OUTPUT_LIMIT.saturating_sub(captured.len());
		captured.extend_from_slice(&chunk[..read.min(remaining)]);
	}
}

fn spawn_reader(
	reader: impl Read + Send + 'static,
) -> std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>> {
	let (sender, receiver) = std::sync::mpsc::sync_channel(1);
	std::thread::spawn(move || {
		let _ = sender.send(drain_bounded(reader));
	});
	receiver
}

fn receive_reader(
	receiver: &std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
	deadline: Instant,
) -> std::io::Result<Vec<u8>> {
	let remaining = deadline.saturating_duration_since(Instant::now());

	match receiver.recv_timeout(remaining) {
		Ok(result) => result,
		Err(error) => {
			let kind = match error {
				std::sync::mpsc::RecvTimeoutError::Timeout => std::io::ErrorKind::TimedOut,
				std::sync::mpsc::RecvTimeoutError::Disconnected => std::io::ErrorKind::Other,
			};
			Err(std::io::Error::new(
				kind,
				"diagnostic output remained open after the probe exited",
			))
		}
	}
}

fn sanitized_first_line(bytes: &[u8]) -> Option<String> {
	let text = String::from_utf8_lossy(bytes);
	let line = text
		.lines()
		.next()
		.map(str::trim)
		.filter(|line| !line.is_empty())?;
	Some(escape_controls(
		&line.chars().take(VERSION_TEXT_LIMIT).collect::<String>(),
	))
}

fn escape_controls(value: &str) -> String {
	let mut escaped = String::with_capacity(value.len());

	for character in value.chars() {
		if character.is_control() {
			escaped.extend(character.escape_default());
		} else {
			escaped.push(character);
		}
	}

	escaped
}

fn presence(exists: bool) -> &'static str {
	if exists { "found" } else { "missing" }
}

/// Report the deployed-size settings for this program.
///
/// The two ways a program silently pays for unused bytes are a second crate
/// type, which makes rustc refuse LTO, and a release profile with no LTO at
/// all. Both were measured in the wild: dropping `"lib"` from a real program
/// shrank it 28.5%, and another measured losing LTO at +40%.
fn size_profile_check(project: &Project) -> DoctorCheck {
	let crate_types = &project.library_crate_types;
	let profile = crate::build::declared_release_profile(project);

	if crate_types.iter().any(|crate_type| crate_type != "cdylib") {
		return DoctorCheck {
			id: "project.size-profile".to_owned(),
			status: CheckStatus::Warn,
			message: format!(
				"crate-type [{}] precludes link-time optimization; a `[\"cdylib\"]`-only target \
				 is typically 20-35% smaller. Move shared logic into a separate crate.",
				crate_types.join(", ")
			),
		};
	}

	if profile.lto != Some(true) {
		return DoctorCheck {
			id: "project.size-profile".to_owned(),
			status: CheckStatus::Warn,
			message: "`[profile.release] lto` is not enabled; fat LTO is the largest single \
			          deployed-size reduction available"
				.to_owned(),
		};
	}

	DoctorCheck {
		id: "project.size-profile".to_owned(),
		status: CheckStatus::Pass,
		message: "cdylib-only target with link-time optimization enabled".to_owned(),
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use ed25519_dalek::SigningKey;
	use solana_address::Address;
	use tempfile::TempDir;

	use super::*;

	/// A lint-driver diagnostic for tests that are not about the driver.
	///
	/// The report renders the section unconditionally, so the fixture carries
	/// a resolved driver; tests that assert the unresolved state build their
	/// own value.
	fn resolved_lint_driver() -> LintDriverDiagnostic {
		LintDriverDiagnostic {
			active_toolchain: Some(
				"1.95.0-nightly (aarch64-apple-darwin 7f99507f5, 2026-02-19)".to_owned(),
			),
			expected_toolchain: "nightly-2026-02-20".to_owned(),
			resolved_driver: Some(PathBuf::from("/cli/pina_lint_driver")),
			resolved_origin: Some("bundled".to_owned()),
			resolved_toolchain: Some("nightly-2026-02-20".to_owned()),
			cache_directory: Some(PathBuf::from("/cache/pina/lint-driver/0.18.0")),
			bundled_driver: Some(PathBuf::from("/cli/pina_lint_driver")),
			bundled_driver_exists: true,
			remedy: None,
		}
	}

	#[test]
	fn renders_stable_human_report() {
		let report = DoctorReport {
			schema_version: 1,
			cli_version: "1.2.3",
			status: DoctorStatus::Warning,
			lint_driver: resolved_lint_driver(),
			project: Some(ProjectDiagnostic {
				root: PathBuf::from("/project"),
				package_name: "counter".to_owned(),
				source: PathBuf::from("/project/src/lib.rs"),
				program_id: Some("11111111111111111111111111111111".to_owned()),
				artifact: PathBuf::from("/project/target/deploy/counter.so"),
				artifact_exists: false,
				keypair: PathBuf::from("/project/target/deploy/counter-keypair.json"),
				keypair_exists: true,
				keypair_matches_source: Some(true),
				clients: vec![ClientLanguage::Rust],
			}),
			tools: vec![
				ToolDiagnostic {
					name: "cargo",
					required: true,
					available: true,
					version: Some("cargo 1.89.0".to_owned()),
				},
				ToolDiagnostic {
					name: "surfpool",
					required: false,
					available: false,
					version: None,
				},
			],
			checks: vec![DoctorCheck {
				id: "project.artifact".to_owned(),
				status: CheckStatus::Warn,
				message: "missing".to_owned(),
			}],
			findings: vec!["build the program".to_owned()],
		};

		assert_eq!(
			report.render_text(),
			"Pina doctor\nCLI: 1.2.3\nProject: /project\nPackage: counter\nProgram ID: \
			 11111111111111111111111111111111\nArtifact: /project/target/deploy/counter.so \
			 (missing)\nKeypair: /project/target/deploy/counter-keypair.json (found)\nLint \
			 driver:\n  active toolchain: 1.95.0-nightly (aarch64-apple-darwin 7f99507f5, \
			 2026-02-19)\n  shipped-lint toolchain: nightly-2026-02-20\n  driver: \
			 /cli/pina_lint_driver (bundled)\n  resolved toolchain: nightly-2026-02-20\n  cache: \
			 /cache/pina/lint-driver/0.18.0\n  bundled: /cli/pina_lint_driver (found)\nTools:\n  \
			 cargo (required): cargo 1.89.0\n  surfpool (optional): missing\nChecks:\n  [warn] \
			 project.artifact: missing\nFindings:\n  - build the program\nStatus: warning\n"
		);
	}

	#[test]
	fn lint_driver_section_reports_an_unresolved_driver_with_its_remedy() {
		let mut report = DoctorReport {
			schema_version: 1,
			cli_version: "1.2.3",
			status: DoctorStatus::Warning,
			lint_driver: LintDriverDiagnostic {
				active_toolchain: Some("1.96.0-nightly (host 8a1061806, 2026-03-01)".to_owned()),
				expected_toolchain: "nightly-2026-02-20".to_owned(),
				resolved_driver: None,
				resolved_origin: None,
				resolved_toolchain: None,
				cache_directory: None,
				bundled_driver: Some(PathBuf::from("/cli/pina_lint_driver")),
				bundled_driver_exists: false,
				remedy: Some("Run `pina lint --build-driver`...".to_owned()),
			},
			project: None,
			tools: Vec::new(),
			checks: Vec::new(),
			findings: Vec::new(),
		};

		let text = report.render_text();
		assert!(text.contains("driver: unavailable"), "{text}");
		assert!(text.contains("active toolchain: 1.96.0-nightly"), "{text}");
		assert!(
			text.contains("shipped-lint toolchain: nightly-2026-02-20"),
			"{text}"
		);
		assert!(
			text.contains("bundled: /cli/pina_lint_driver (missing)"),
			"{text}"
		);
		assert!(
			text.contains("remedy: Run `pina lint --build-driver`"),
			"{text}"
		);
		assert!(
			!text.contains("cache:"),
			"an unavailable cache directory is omitted rather than shown empty: {text}"
		);
		assert!(
			!text.contains("resolved toolchain:"),
			"an unresolved driver has no toolchain to report: {text}"
		);

		// An unresolved driver is reported in JSON without inventing a path.
		report.status = DoctorStatus::Warning;
		let json = serde_json::to_value(&report).expect("serialize the report");
		assert!(json["lintDriver"]["resolvedDriver"].is_null());
		assert!(json["lintDriver"]["resolvedToolchain"].is_null());
		assert_eq!(json["lintDriver"]["bundledDriverExists"], false);
	}

	/// The regression behind issue #458: a driver negotiated for the active
	/// toolchain must not make the diagnostic look pinned to the shipped
	/// nightly. Cached, downloaded, and source-built drivers all exist for the
	/// active revision by construction, so that is the toolchain to report.
	#[test]
	fn a_negotiated_driver_reports_the_active_toolchain_not_the_shipped_pin() {
		const SHIPPED: &str = "nightly-2026-02-20";
		let september = "1.96.0-nightly (host 8a1061806, 2026-09-01)";

		for origin in [
			DriverOrigin::Cache,
			DriverOrigin::Downloaded,
			DriverOrigin::BuiltFromSource,
		] {
			let resolved = resolved_toolchain_for(Some(origin), Some(september));
			assert_eq!(
				resolved.as_deref(),
				Some(september),
				"{origin:?} must report the active toolchain"
			);
			assert_ne!(
				resolved.as_deref(),
				Some(SHIPPED),
				"a negotiated driver must not look pinned to the shipped nightly"
			);
		}

		// Without an identified active toolchain there is nothing honest to
		// report for a negotiated driver.
		assert_eq!(
			resolved_toolchain_for(Some(DriverOrigin::Cache), None),
			None
		);
	}

	/// A bundled driver was built for the shipped nightly even though the load
	/// probe proved it compatible with the active compiler — that difference is
	/// the honest answer. An override names an arbitrary local build, and no
	/// resolution at all leaves nothing to report.
	#[test]
	fn bundled_drivers_report_the_shipped_nightly_and_unknowns_stay_absent() {
		let active = "1.96.0-nightly (host 8a1061806, 2026-09-01)";

		assert_eq!(
			resolved_toolchain_for(Some(DriverOrigin::Bundled), Some(active)),
			Some(crate::lint_driver::LINT_DRIVER_TOOLCHAIN.to_owned())
		);
		assert_eq!(
			resolved_toolchain_for(Some(DriverOrigin::Bundled), None),
			Some(crate::lint_driver::LINT_DRIVER_TOOLCHAIN.to_owned())
		);
		assert_eq!(
			resolved_toolchain_for(Some(DriverOrigin::Override), Some(active)),
			None,
			"an override's toolchain is unknowable"
		);
		assert_eq!(
			resolved_toolchain_for(None, Some(active)),
			None,
			"nothing resolved, so there is no toolchain to report"
		);
	}

	#[test]
	fn human_report_escapes_controls_without_changing_json_values() {
		let report = DoctorReport {
			schema_version: 1,
			cli_version: "1.2.3",
			status: DoctorStatus::Warning,
			lint_driver: resolved_lint_driver(),
			project: Some(ProjectDiagnostic {
				root: PathBuf::from("/project\n\u{1b}"),
				package_name: "counter\rname".to_owned(),
				source: PathBuf::from("/source"),
				program_id: Some("program\tidentifier".to_owned()),
				artifact: PathBuf::from("/artifact\npath"),
				artifact_exists: false,
				keypair: PathBuf::from("/keypair\rpath"),
				keypair_exists: true,
				keypair_matches_source: None,
				clients: Vec::new(),
			}),
			tools: vec![ToolDiagnostic {
				name: "tool\nname",
				required: true,
				available: true,
				version: Some("version\u{1b}".to_owned()),
			}],
			checks: vec![DoctorCheck {
				id: "check\nid".to_owned(),
				status: CheckStatus::Warn,
				message: "message\rtext".to_owned(),
			}],
			findings: vec!["finding\ttext".to_owned()],
		};

		let text = report.render_text();
		let json = serde_json::to_value(&report)
			.unwrap_or_else(|error| panic!("serialization failed: {error}"));

		assert!(
			text.chars()
				.all(|character| character == '\n' || !character.is_control())
		);
		assert!(text.contains("/project\\n\\u{1b}"));
		assert!(text.contains("counter\\rname"));
		assert!(text.contains("check\\nid: message\\rtext"));
		assert!(text.contains("finding\\ttext"));
		assert_eq!(json["project"]["packageName"], "counter\rname");
		assert_eq!(json["checks"][0]["message"], "message\rtext");

		// The fixture resolves a bundled driver, so its own toolchain is the
		// shipped nightly, reported beside the unchanged expectedToolchain key.
		assert_eq!(
			json["lintDriver"]["expectedToolchain"],
			"nightly-2026-02-20"
		);
		assert_eq!(
			json["lintDriver"]["resolvedToolchain"],
			"nightly-2026-02-20"
		);
	}

	#[test]
	fn renders_error_and_ok_reports_without_a_project_or_findings() {
		let error = DoctorReport {
			schema_version: 1,
			cli_version: "1.2.3",
			status: DoctorStatus::Error,
			lint_driver: resolved_lint_driver(),
			project: None,
			tools: Vec::new(),
			checks: vec![DoctorCheck {
				id: "project.discovery".to_owned(),
				status: CheckStatus::Fail,
				message: "missing".to_owned(),
			}],
			findings: Vec::new(),
		};
		let ok = DoctorReport {
			status: DoctorStatus::Ok,
			..error.clone()
		};

		assert!(error.render_text().contains("Project: unavailable"));
		assert!(error.render_text().contains("Status: error"));
		assert!(ok.render_text().contains("Status: ok"));
	}

	/// Build a minimal discoverable project whose library target declares the
	/// given crate types and release profile lines.
	fn project_with_crate_types(crate_types: &str, release_lines: &str) -> TempDir {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = temp.path();
		let source_dir = root.join("src");
		fs::create_dir_all(&source_dir)
			.unwrap_or_else(|error| panic!("source create failed: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			format!(
				"[package]\nname = \"size-demo\"\n\n[lib]\ncrate-type = \
				 {crate_types}\n\n[profile.release]\n{release_lines}"
			),
		)
		.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
		fs::write(
			source_dir.join("lib.rs"),
			"declare_id!(\"11111111111111111111111111111111\");\n",
		)
		.unwrap_or_else(|error| panic!("source write failed: {error}"));
		temp
	}

	fn check<'a>(report: &'a DoctorReport, id: &str) -> Option<&'a DoctorCheck> {
		report.checks.iter().find(|check| check.id == id)
	}

	/// A second crate type silently disables fat LTO. Measured in the wild: a
	/// program that dropped `"lib"` shrank 28.5%, and another measured the
	/// reverse at +40%. The diagnostic has to fire on the crate-type alone,
	/// because `cargo-build-sbf --lto` also rejects the combination.
	#[test]
	fn warns_when_a_second_crate_type_disables_lto() {
		let temp =
			project_with_crate_types("[\"cdylib\", \"lib\"]", "opt-level = 3\nlto = \"fat\"\n");
		let report = diagnose(temp.path());

		let size_check = check(&report, "project.size-profile")
			.unwrap_or_else(|| panic!("size profile check must run: {:?}", report.checks));
		assert_eq!(
			size_check.status,
			CheckStatus::Warn,
			"a dual crate type must warn: {}",
			size_check.message
		);
		assert!(
			size_check.message.contains("cdylib")
				&& size_check.message.contains("link-time optimization")
				&& size_check.message.contains("35%"),
			"the message must name the cause, the remedy, and the measured cost: {}",
			size_check.message
		);
	}

	/// A cdylib-only target with the size settings passes.
	#[test]
	fn passes_for_a_cdylib_only_target_with_size_settings() {
		let temp = project_with_crate_types(
			"[\"cdylib\"]",
			"opt-level = 3\nlto = \"fat\"\ncodegen-units = 1\npanic = \"abort\"\n",
		);
		let report = diagnose(temp.path());

		let size_check = check(&report, "project.size-profile")
			.unwrap_or_else(|| panic!("size profile check must run: {:?}", report.checks));
		assert_eq!(
			size_check.status,
			CheckStatus::Pass,
			"a cdylib-only target with a size profile must pass: {}",
			size_check.message
		);
	}

	/// A cdylib-only target that never enabled LTO still warns, because the
	/// single biggest size win is unclaimed.
	#[test]
	fn warns_when_lto_is_not_enabled() {
		let temp = project_with_crate_types("[\"cdylib\"]", "opt-level = 3\n");
		let report = diagnose(temp.path());

		let size_check = check(&report, "project.size-profile")
			.unwrap_or_else(|| panic!("size profile check must run: {:?}", report.checks));
		assert_eq!(
			size_check.status,
			CheckStatus::Warn,
			"a missing lto setting must warn: {}",
			size_check.message
		);
	}

	#[test]
	fn diagnoses_matching_and_mismatching_project_identity() {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
		let root = fs::canonicalize(temp.path())
			.unwrap_or_else(|error| panic!("canonicalize failed: {error}"));
		let source_dir = root.join("src");
		let deploy_dir = root.join("target/deploy");
		fs::create_dir_all(&source_dir)
			.unwrap_or_else(|error| panic!("source create failed: {error}"));
		fs::create_dir_all(&deploy_dir)
			.unwrap_or_else(|error| panic!("deploy create failed: {error}"));
		fs::write(
			root.join("Cargo.toml"),
			"[package]\nname = \"doctor-demo\"\n",
		)
		.unwrap_or_else(|error| panic!("manifest write failed: {error}"));
		fs::write(deploy_dir.join("doctor_demo.so"), [])
			.unwrap_or_else(|error| panic!("artifact write failed: {error}"));
		let signing_key = SigningKey::from_bytes(&[12u8; 32]);
		let public_key = signing_key.verifying_key().to_bytes();
		let program_id = Address::from(public_key).to_string();
		let mut bytes = signing_key.to_bytes().to_vec();
		bytes.extend(public_key);
		fs::write(
			deploy_dir.join("doctor_demo-keypair.json"),
			serde_json::to_vec(&bytes)
				.unwrap_or_else(|error| panic!("keypair serialization failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("keypair write failed: {error}"));
		fs::write(
			source_dir.join("lib.rs"),
			format!("declare_id!(\"{program_id}\");\n"),
		)
		.unwrap_or_else(|error| panic!("source write failed: {error}"));

		let matching = diagnose(&root);
		assert_eq!(
			matching
				.project
				.as_ref()
				.and_then(|project| project.keypair_matches_source),
			Some(true)
		);

		fs::write(
			source_dir.join("lib.rs"),
			"declare_id!(\"11111111111111111111111111111111\");\n",
		)
		.unwrap_or_else(|error| panic!("source rewrite failed: {error}"));
		let mismatch = diagnose(&root);
		assert_eq!(
			mismatch
				.project
				.as_ref()
				.and_then(|project| project.keypair_matches_source),
			Some(false)
		);
		assert!(
			mismatch
				.findings
				.contains(&"run `pina keys sync` after reviewing the selected keypair".to_owned())
		);
	}

	#[test]
	fn tool_diagnostics_cover_launch_failure_nonzero_and_stderr_versions() {
		let missing = diagnose_tool(ToolSpec {
			name: "pina-definitely-missing-tool",
			required: true,
			version_args: &[],
			accept_nonzero: false,
		});
		let rejected = diagnose_tool(ToolSpec {
			name: "sh",
			required: false,
			version_args: &["--pina-invalid-option"],
			accept_nonzero: false,
		});
		let accepted = diagnose_tool(ToolSpec {
			name: "sh",
			required: false,
			version_args: &["--pina-invalid-option"],
			accept_nonzero: true,
		});

		assert!(!missing.available);
		assert!(!rejected.available);
		assert!(accepted.available);
		assert!(accepted.version.is_some());
	}

	#[cfg(unix)]
	#[test]
	fn command_capture_drains_both_streams_with_bounded_storage() {
		let output = capture_command(Command::new("sh").args([
			"-c",
			"i=0; while [ $i -lt 6000 ]; do printf x; printf y >&2; i=$((i+1)); done",
		]))
		.unwrap_or_else(|error| panic!("capture failed: {error}"));

		assert!(output.status.success());
		assert_eq!(output.stdout.len(), TOOL_OUTPUT_LIMIT);
		assert_eq!(output.stderr.len(), TOOL_OUTPUT_LIMIT);
	}

	#[test]
	fn command_capture_closes_child_stdin() {
		const NESTED: &str = "PINA_DOCTOR_STDIN_NESTED";

		let executable = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable discovery failed: {error}"));

		if std::env::var_os(NESTED).is_some() {
			let output = capture_command(
				Command::new(executable)
					.args(["--exact", "doctor::tests::stdin_probe_process"])
					.env("PINA_DOCTOR_STDIN_PROBE", "1"),
			)
			.unwrap_or_else(|error| panic!("capture failed: {error}"));
			assert!(output.status.success());

			return;
		}

		let mut nested = Command::new(executable)
			.args([
				"--exact",
				"doctor::tests::command_capture_closes_child_stdin",
			])
			.env(NESTED, "1")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("nested test launch failed: {error}"));
		let _held_open = nested
			.stdin
			.take()
			.unwrap_or_else(|| panic!("nested stdin pipe should be present"));

		let status = wait_for_test_child(&mut nested, 200)
			.unwrap_or_else(|| panic!("diagnostic child inherited operator input"));
		assert!(status.success());
	}

	#[test]
	fn command_capture_and_status_kill_hanging_probes() {
		let mut capture = hanging_command();
		let capture_error = capture_command_with_timeout(&mut capture, Duration::from_millis(50))
			.expect_err("capture must time out");
		assert_eq!(capture_error.kind(), std::io::ErrorKind::TimedOut);

		let mut status = hanging_command();
		let mut child = status
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("status probe launch failed: {error}"));
		let status_error = wait_with_timeout(&mut child, Duration::from_millis(50))
			.expect_err("status must time out");
		assert_eq!(status_error.kind(), std::io::ErrorKind::TimedOut);

		let mut preserved = FakeManagedChild::new(true, false, false);
		let error = wait_with_timeout(&mut preserved, Duration::ZERO)
			.expect_err("try-wait error must be preserved");
		assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
		let mut timed_out = FakeManagedChild::new(false, false, false);
		let error = wait_with_timeout(&mut timed_out, Duration::ZERO)
			.expect_err("running child must time out");
		assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
		assert!(timed_out.wait_called);

		for (kill_error, wait_error) in [(true, false), (false, true)] {
			let mut cleanup_failure = FakeManagedChild::new(true, kill_error, wait_error);
			let error = wait_with_timeout(&mut cleanup_failure, Duration::ZERO)
				.expect_err("cleanup failure must be reported");
			assert!(error.to_string().contains("child cleanup failed"));
			assert_eq!(cleanup_failure.wait_called, !kill_error);
		}
	}

	#[test]
	fn disconnected_output_reader_is_reported() {
		let (sender, receiver) = std::sync::mpsc::channel();
		drop(sender);

		let error = receive_reader(&receiver, Instant::now() + Duration::from_millis(10))
			.expect_err("disconnected reader must fail");
		assert_eq!(error.kind(), std::io::ErrorKind::Other);
	}

	#[test]
	fn command_capture_bounds_pipes_held_by_descendants() {
		const PROBE: &str = "PINA_DOCTOR_OUTPUT_PROBE";
		const HOLDER: &str = "PINA_DOCTOR_OUTPUT_HOLDER";
		let executable = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable discovery failed: {error}"));

		if std::env::var_os(HOLDER).is_some() {
			std::thread::sleep(Duration::from_millis(500));
			return;
		}

		if std::env::var_os(PROBE).is_some() {
			let mut holder = Command::new(executable)
				.args([
					"--exact",
					"doctor::tests::command_capture_bounds_pipes_held_by_descendants",
				])
				.env(HOLDER, "1")
				.stdin(Stdio::null())
				.spawn()
				.unwrap_or_else(|error| panic!("output holder launch failed: {error}"));
			std::thread::spawn(move || holder.wait());
			return;
		}

		let mut command = Command::new(executable);
		command
			.args([
				"--exact",
				"doctor::tests::command_capture_bounds_pipes_held_by_descendants",
			])
			.env(PROBE, "1");
		let started = Instant::now();
		let error = capture_command_with_timeout(&mut command, Duration::from_millis(100))
			.expect_err("inherited output pipe must hit the capture deadline");

		assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
		assert!(started.elapsed() < Duration::from_millis(400));
	}

	struct FakeManagedChild {
		failures: [bool; 3],
		wait_called: bool,
	}

	impl FakeManagedChild {
		fn new(try_error: bool, kill_error: bool, wait_error: bool) -> Self {
			Self {
				failures: [try_error, kill_error, wait_error],
				wait_called: false,
			}
		}
	}

	impl ManagedChild for FakeManagedChild {
		fn try_wait_managed(&mut self) -> std::io::Result<Option<ExitStatus>> {
			if self.failures[0] {
				return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
			}
			Ok(None)
		}

		fn kill_managed(&mut self) -> std::io::Result<()> {
			if self.failures[1] {
				return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
			}
			Ok(())
		}

		fn wait_managed(&mut self) -> std::io::Result<ExitStatus> {
			self.wait_called = true;
			if self.failures[2] {
				return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
			}
			Ok(success_status())
		}
	}

	#[cfg(unix)]
	fn success_status() -> ExitStatus {
		use std::os::unix::process::ExitStatusExt;
		ExitStatus::from_raw(0)
	}

	#[cfg(windows)]
	fn success_status() -> ExitStatus {
		use std::os::windows::process::ExitStatusExt;
		ExitStatus::from_raw(0)
	}

	#[cfg(unix)]
	fn hanging_command() -> Command {
		let mut command = Command::new("sleep");
		command.arg("60");
		command
	}

	#[cfg(windows)]
	fn hanging_command() -> Command {
		let mut command = Command::new("ping");
		command.args(["-n", "60", "127.0.0.1"]);
		command
	}

	#[test]
	fn stdin_probe_process() {
		if std::env::var_os("PINA_DOCTOR_STDIN_PROBE").is_none() {
			return;
		}

		let mut input = Vec::new();
		std::io::stdin()
			.read_to_end(&mut input)
			.unwrap_or_else(|error| panic!("stdin probe failed: {error}"));
		assert!(input.is_empty());
	}

	#[test]
	fn nested_test_wait_terminates_a_blocked_probe() {
		let executable = std::env::current_exe()
			.unwrap_or_else(|error| panic!("test executable discovery failed: {error}"));
		let mut child = Command::new(executable)
			.args(["--exact", "doctor::tests::stdin_probe_process"])
			.env("PINA_DOCTOR_STDIN_PROBE", "1")
			.stdin(Stdio::piped())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.unwrap_or_else(|error| panic!("probe launch failed: {error}"));
		let _held_open = child
			.stdin
			.take()
			.unwrap_or_else(|| panic!("probe stdin pipe should be present"));

		assert!(wait_for_test_child(&mut child, 1).is_none());
	}

	fn wait_for_test_child(child: &mut std::process::Child, attempts: usize) -> Option<ExitStatus> {
		for _ in 0..attempts {
			if let Some(status) = child
				.try_wait()
				.unwrap_or_else(|error| panic!("nested test wait failed: {error}"))
			{
				return Some(status);
			}

			std::thread::sleep(Duration::from_millis(10));
		}

		let _ = child.kill();
		let _ = child.wait();

		None
	}

	#[test]
	fn version_text_escapes_controls_and_replaces_invalid_utf8() {
		let version = sanitized_first_line(b"\x1b[31mtool\xff\t1.0\nignored")
			.unwrap_or_else(|| panic!("version should be present"));

		assert!(!version.contains('\x1b'));
		assert!(!version.contains('\t'));
		assert!(version.contains("\\u{1b}"));
		assert!(version.contains(char::REPLACEMENT_CHARACTER));
		assert!(version.contains("\\t"));
	}

	#[test]
	fn version_text_is_bounded_and_empty_input_stays_absent() {
		let oversized = vec![b'x'; VERSION_TEXT_LIMIT * 4];
		let version =
			sanitized_first_line(&oversized).unwrap_or_else(|| panic!("version should be present"));

		assert_eq!(version.len(), VERSION_TEXT_LIMIT);
		assert_eq!(sanitized_first_line(b"\n"), None);
	}

	#[test]
	fn metadata_inspection_and_versionless_tools_remain_unambiguous() {
		let path = Path::new("keypair\n\u{1b}[31m.json");
		assert!(
			inspect_metadata(
				path,
				Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
			)
			.unwrap_or_else(|error| panic!("missing metadata rejected: {error}"))
			.is_none()
		);
		let denied = inspect_metadata(
			path,
			Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
		)
		.expect_err("inspection error must be preserved");
		assert!(!denied.contains('\u{1b}'));
		assert!(!denied.contains('\n'));
		assert!(denied.contains("\\n"));

		assert_eq!(
			tool_message(&ToolDiagnostic {
				name: "cargo-build-sbf",
				required: true,
				available: true,
				version: None,
			}),
			"available (version not reported)"
		);

		let mut findings = Vec::new();
		let inspection_error = Err("inspection failed".to_owned());
		append_inspection_error(&inspection_error, &mut findings);
		append_inspection_error(&Ok(None), &mut findings);
		assert_eq!(findings, ["inspection failed"]);
		assert_eq!(
			artifact_check(path, &inspection_error, false).status,
			CheckStatus::Fail
		);
		assert_eq!(
			artifact_check(path, &Ok(None), false).status,
			CheckStatus::Warn
		);
		assert_eq!(
			artifact_check(path, &Ok(None), true).status,
			CheckStatus::Pass
		);
		assert_eq!(
			keypair_check(path, &inspection_error, false, false).status,
			CheckStatus::Fail
		);
		assert_eq!(
			keypair_check(path, &Ok(None), true, false).status,
			CheckStatus::Fail
		);
		assert_eq!(
			keypair_check(path, &Ok(None), false, false).status,
			CheckStatus::Warn
		);
		assert_eq!(
			keypair_check(path, &Ok(None), true, true).status,
			CheckStatus::Pass
		);
	}
}
