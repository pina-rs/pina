//! Isolation for the npm and Node children generation and IDL commands run.
//!
//! A renderer or official-client child resolves packages from its working
//! directory (a stdin module resolves bare imports from `cwd`) and from every
//! `node_modules/.bin` directory on `PATH`, and `npx` prefers a matching
//! local install over the pinned registry fetch. All three channels reach a
//! committed `node_modules` when the child runs inside an untrusted project,
//! which is arbitrary code execution with the developer's privileges.
//!
//! [`isolate_package_child`] closes the channels the repository controls: the
//! child runs from an empty temporary directory — so no project
//! `node_modules`, `.npmrc`, or package manifest is visible — and every
//! `PATH` entry inside the project workspace is dropped, so the render
//! script's `PATH` fallback cannot import a committed package either. The
//! `npx`/`pnpm dlx` cache directories those runners prepend to `PATH` live
//! outside the workspace, so the pinned packages still resolve.
//!
//! Running a renderer through a Node executable directly (`--npx <node>`)
//! intentionally skips isolation: that mode exists to resolve packages from
//! the project on purpose, and it is documented as an explicit operator
//! choice.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Configure `command` so it cannot resolve project-local packages.
///
/// Returns the isolated working directory, which must outlive the child.
///
/// # Errors
///
/// Returns an error when the temporary directory or the current directory
/// cannot be determined.
pub(crate) fn isolate_package_child(
	command: &mut Command,
	workspace_root: &Path,
) -> std::io::Result<TempDir> {
	let isolated = TempDir::new()?;

	if let Some(path) = std::env::var_os("PATH") {
		let current = std::env::current_dir()?;
		command.env("PATH", scrubbed_path(&path, workspace_root, &current)?);
	}

	// A pure package-resolution channel nothing in the pinned flows needs.
	command.env_remove("NODE_PATH");
	command.current_dir(isolated.path());

	Ok(isolated)
}

/// Return `path` without the entries that sit inside `workspace_root`.
///
/// Relative entries resolve against `current` before the comparison, because
/// a project-local `node_modules/.bin` is usually added as a relative entry.
/// Entries outside the workspace keep their original spelling and order, and
/// empty entries are dropped.
pub(crate) fn scrubbed_path(
	path: &OsStr,
	workspace_root: &Path,
	current: &Path,
) -> std::io::Result<OsString> {
	let kept = std::env::split_paths(path)
		.filter(|entry| {
			!entry.as_os_str().is_empty()
				&& !resolved_entry(entry, current).starts_with(workspace_root)
		})
		.collect::<Vec<_>>();

	std::env::join_paths(kept).map_err(std::io::Error::other)
}

fn resolved_entry(entry: &Path, current: &Path) -> PathBuf {
	if entry.is_absolute() {
		entry.to_path_buf()
	} else {
		current.join(entry)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn scrub(raw: &str, workspace_root: &Path, current: &Path) -> String {
		let scrubbed = scrubbed_path(OsStr::new(raw), workspace_root, current)
			.unwrap_or_else(|error| panic!("scrub failed: {error}"));

		scrubbed.to_string_lossy().into_owned()
	}

	#[test]
	fn entries_inside_the_workspace_are_dropped_and_others_kept_in_order() {
		let workspace = Path::new("/work/repo");
		let current = Path::new("/work/repo/program");
		let raw = "/usr/bin:/work/repo/node_modules/.bin:bin:/opt/tools/bin";

		assert_eq!(
			scrub(raw, workspace, current),
			"/usr/bin:/opt/tools/bin",
			"workspace entries must be dropped, including relative ones resolved against the \
			 current directory, and survivors must keep their order"
		);
	}

	#[test]
	fn empty_entries_do_not_become_separators() {
		let workspace = Path::new("/work/repo");
		let current = Path::new("/work/repo");

		assert_eq!(scrub("", workspace, current), "");
		assert_eq!(
			scrub("/usr/bin::/opt/bin", workspace, current),
			"/usr/bin:/opt/bin"
		);
	}

	#[cfg(unix)]
	#[test]
	fn non_utf8_entries_survive_scrubbing() {
		use std::os::unix::ffi::OsStringExt as _;

		let workspace = Path::new("/work/repo");
		let current = Path::new("/elsewhere");
		let raw = OsString::from_vec(vec![b'a', 0xFF, b'/', b'x']);

		let scrubbed = scrubbed_path(raw.as_os_str(), workspace, current)
			.unwrap_or_else(|error| panic!("scrub failed: {error}"));

		assert_eq!(scrubbed, raw);
	}
	#[test]
	fn isolated_child_runs_outside_the_project_without_workspace_path_entries() {
		let project = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let workspace = project.path().join("repo");
		std::fs::create_dir_all(workspace.join("node_modules/.bin"))
			.unwrap_or_else(|error| panic!("{error}"));

		let mut command = Command::new("printer");
		let isolated = isolate_package_child(&mut command, &workspace)
			.unwrap_or_else(|error| panic!("{error}"));
		let configured = command
			.get_current_dir()
			.expect("isolation sets a working directory");

		assert_ne!(
			configured,
			std::env::current_dir().expect("test has a working directory"),
			"the child must not run from the caller's directory"
		);
		assert!(
			configured.starts_with(isolated.path()),
			"the child must run from the isolated temporary directory"
		);
		assert!(
			command
				.get_envs()
				.any(|(key, value)| { key == "NODE_PATH" && value.is_none() }),
			"NODE_PATH must be removed from the child environment"
		);
	}

	#[test]
	fn untrusted_directories_are_not_writable_through_the_scrub() {
		// The scrub keeps every entry outside the workspace; this pins the
		// trust boundary itself — only repository-controlled locations are
		// removed, so legitimate toolchains keep working.
		let workspace = Path::new("/work/repo");
		let current = Path::new("/home/dev");

		assert_eq!(
			scrub("/home/dev/.bin", workspace, current),
			"/home/dev/.bin"
		);
	}

	/// A shadow package committed under the project's `node_modules`, run
	/// through the exact stdin-module resolution the render script uses.
	#[cfg(unix)]
	#[test]
	fn isolated_children_cannot_import_project_local_packages() {
		use std::io::Write as _;
		use std::process::Stdio as TestStdio;

		let project = tempfile::TempDir::new().unwrap_or_else(|error| panic!("{error}"));
		let shadow = project.path().join("node_modules/codama");
		std::fs::create_dir_all(&shadow).unwrap_or_else(|error| panic!("{error}"));
		std::fs::write(
			shadow.join("package.json"),
			r#"{"name":"codama","version":"99.0.0","main":"index.js"}"#,
		)
		.unwrap_or_else(|error| panic!("{error}"));
		std::fs::write(
			shadow.join("index.js"),
			r#"import { writeFileSync } from "node:fs";
writeFileSync(process.env.PINA_SHADOW_PROBE ?? "/dev/null", "pwned");
"#,
		)
		.unwrap_or_else(|error| panic!("{error}"));

		// The render script resolves its packages by importing them from a
		// stdin module, falling back to every node_modules/.bin directory on
		// PATH — mirror both channels and report what resolved.
		let script = r#"
import { existsSync, readFileSync } from "node:fs";
import { basename, dirname, delimiter, join } from "node:path";
import { pathToFileURL } from "node:url";

async function loadPackage(name) {
	try {
		return await import(name);
	} catch (error) {
		if (error?.code !== "ERR_MODULE_NOT_FOUND") {
			throw error;
		}
	}
	for (const binDir of (process.env.PATH ?? "").split(delimiter)) {
		if (basename(binDir) !== ".bin" || basename(dirname(binDir)) !== "node_modules") {
			continue;
		}
		const manifestPath = join(dirname(binDir), ...name.split("/"), "package.json");
		if (!existsSync(manifestPath)) {
			continue;
		}
		return await import(pathToFileURL(join(dirname(binDir), ...name.split("/"))).href);
	}
	throw new Error("unresolved");
}

try {
	await loadPackage("codama");
	console.log("RESOLVED");
} catch {
	console.log("UNRESOLVED");
}
"#;
		let probe = project.path().join("shadow-ran");

		let run = |isolate: bool| {
			let mut command = Command::new("node");
			command.args(["--input-type=module", "-"]);
			command.env("PINA_SHADOW_PROBE", &probe);
			command.env_remove("NODE_PATH");
			if isolate {
				// The guard must outlive the child, or the isolated
				// directory disappears before the spawn.
				let _guard = isolate_package_child(&mut command, project.path())
					.unwrap_or_else(|error| panic!("isolation failed: {error}"));
				command.stdin(TestStdio::piped());
				command.stdout(TestStdio::piped());
				command.stderr(TestStdio::piped());
				let mut child = command.spawn().expect("node is available in the dev shell");
				child
					.stdin
					.take()
					.expect("stdin pipe")
					.write_all(script.as_bytes())
					.expect("write script");
				let output = child.wait_with_output().expect("wait for node");

				assert!(
					output.status.success(),
					"node script failed: {}",
					String::from_utf8_lossy(&output.stderr)
				);

				return String::from_utf8_lossy(&output.stdout).trim().to_owned();
			}
			// The explicit project-package mode: the child keeps the
			// project directory, so local installs resolve on purpose.
			command.current_dir(project.path());
			command.stdin(TestStdio::piped());
			command.stdout(TestStdio::piped());
			command.stderr(TestStdio::piped());
			let mut child = command.spawn().expect("node is available in the dev shell");
			child
				.stdin
				.take()
				.expect("stdin pipe")
				.write_all(script.as_bytes())
				.expect("write script");
			let output = child.wait_with_output().expect("wait for node");

			assert!(
				output.status.success(),
				"node script failed: {}",
				String::from_utf8_lossy(&output.stderr)
			);
			String::from_utf8_lossy(&output.stdout).trim().to_owned()
		};

		assert_eq!(
			run(false),
			"RESOLVED",
			"the project-package mode resolves local installs"
		);
		std::fs::remove_file(&probe).ok();
		assert_eq!(
			run(true),
			"UNRESOLVED",
			"an isolated child must not import a committed package"
		);
		assert!(!probe.exists(), "the shadow package never executed");
	}
}
