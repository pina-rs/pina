//! `rustc` wrapper that runs Pina's security lints.
//!
//! The driver is used as `RUSTC_WRAPPER`: cargo invokes it with the arguments
//! it would have passed to `rustc`, the driver registers every lint compiled
//! into [`pina_lints`], and compilation continues normally. Because the lints
//! are linked into the driver, no dynamic library loading or external lint
//! tooling is required.
//!
//! # Environment variables
//!
//! - `PINA_LINT_NO_DEPS`: when set to a value other than `0`, only the crate
//!   cargo identifies as the primary package is linted (dependency crates are
//!   skipped entirely).
//! - `PINA_LINT_LEVELS`: a comma-separated list of `lint=level` pairs where
//!   `level` is `allow`, `warn`, or `deny`. Each pair is forwarded to `rustc`
//!   as a `-A`/`-W`/`-D` lint level argument, overriding the lint's default.
//! - `PINA_LINT_LIST`: when set to a value other than `0`, the driver prints
//!   the lint catalog and exits instead of compiling.
//!
//! `PINA_LINT_NO_DEPS` and `PINA_LINT_LEVELS` are recorded in dep-info, so
//! changing them invalidates cargo's cached check results.

// Linking against `rustc_private` requires nightly features; the
// workspace-wide `unstable_features = "deny"` lint is waived for this binary.
#![feature(rustc_private)]
#![allow(unstable_features)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::hash_map::DefaultHasher;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::exit;

use pina_lints::LINTS;

/// Environment variable holding the configured lint levels.
const PINA_LINT_LEVELS: &str = "PINA_LINT_LEVELS";

/// Environment variable enabling primary-package-only linting.
const PINA_LINT_NO_DEPS: &str = "PINA_LINT_NO_DEPS";

/// Environment variable requesting catalog output instead of compilation.
const PINA_LINT_LIST: &str = "PINA_LINT_LIST";

/// Environment variable restricting linting to the named lint.
///
/// The bundled UI tests use this to reproduce Dylint's one-library-per-fixture
/// test shape while shipping every lint in one driver.
const PINA_LINT_ONLY: &str = "PINA_LINT_ONLY";

/// Environment variables that change lint behavior and must invalidate
/// cargo's cached check results.
const UNTRACKED_STATE_VARS: &[&str] = &[PINA_LINT_LEVELS, PINA_LINT_NO_DEPS, PINA_LINT_ONLY];

/// Dep-info key recording the hash of [`UNTRACKED_STATE_VARS`].
const UNTRACKED_STATE_VAR: &str = "PINA_LINT_UNTRACKED_STATE";

struct Callbacks;

impl rustc_driver::Callbacks for Callbacks {
	fn config(&mut self, config: &mut rustc_interface::Config) {
		let previous = config.register_lints.take();
		config.register_lints = Some(Box::new(move |sess, lint_store| {
			if let Some(previous) = &previous {
				previous(sess, lint_store);
			}

			// Record the lint configuration in dep-info so cargo re-checks
			// the crate whenever the configuration changes.
			for var in UNTRACKED_STATE_VARS {
				let value = env::var(var).ok();
				sess.psess.env_depinfo.lock().insert((
					rustc_span::Symbol::intern(var),
					value.as_deref().map(rustc_span::Symbol::intern),
				));
			}

			if no_deps_enabled() && env::var_os("CARGO_PRIMARY_PACKAGE").is_none() {
				return;
			}

			match env::var(PINA_LINT_ONLY)
				.ok()
				.filter(|value| !value.is_empty())
			{
				Some(name) => {
					if pina_lints::LINT_NAMES.contains(&name.as_str()) {
						pina_lints::register_selected_lints(sess, lint_store, &[&name]);
					} else {
						sess.dcx().err(format!(
							"unknown lint `{name}` in {}; known lints: {}",
							PINA_LINT_ONLY,
							pina_lints::LINT_NAMES.join(", "),
						));
					}
				}
				None => pina_lints::register_all_lints(sess, lint_store),
			}

			if list_enabled() {
				list_lints();
				exit(0);
			}
		}));

		// Choose to be compatible with Clippy: zero MIR optimizations so
		// `--fix` suggestions match Clippy's.
		config.opts.unstable_opts.mir_opt_level = Some(0);
	}
}

fn main() {
	let args: Vec<OsString> = env::args_os().collect();

	if args.len() <= 1 || args.iter().any(|arg| arg == "-V") {
		let toolchain = env::var("RUSTUP_TOOLCHAIN").unwrap_or_default();
		println!("{} {}", toolchain, env!("CARGO_PKG_VERSION"));
		return;
	}

	if let Err(message) = run(&args[1..]) {
		eprintln!("error: {message}");
		exit(1);
	}
}

/// Build the `rustc` argument list and hand control to the compiler.
fn run(args: &[OsString]) -> Result<(), String> {
	let mut rustc_args = Vec::new();
	rustc_args.push("rustc".to_owned());

	let mut args = args.iter();
	if let Some(first) = args.next()
		&& !is_rustc(first)
	{
		rustc_args.push(first.to_string_lossy().to_string());
	}
	rustc_args.extend(args.map(|arg| arg.to_string_lossy().to_string()));

	if let Some(sysroot) = sysroot() {
		rustc_args.push("--sysroot".to_owned());
		rustc_args.push(sysroot.to_string_lossy().to_string());
	}

	rustc_args.extend(level_args(PINA_LINT_LEVELS)?);

	if let Some(state) = untracked_state() {
		rustc_args.push("--allow=rustc::internal".to_owned());
		rustc_args.push("-Zunstable-options".to_owned());
		rustc_args.push(format!("--env-set={UNTRACKED_STATE_VAR}={state}"));
	}

	rustc_driver::run_compiler(&rustc_args, &mut Callbacks);

	Ok(())
}

/// Return whether the argument names a `rustc` executable.
fn is_rustc(arg: &OsStr) -> bool {
	Path::new(arg).file_stem() == Some(OsStr::new("rustc"))
}

/// Resolve the sysroot of the toolchain the driver must behave like.
fn sysroot() -> Option<PathBuf> {
	if let (Ok(home), Ok(toolchain)) = (env::var("RUSTUP_HOME"), env::var("RUSTUP_TOOLCHAIN")) {
		return Some(PathBuf::from(home).join("toolchains").join(toolchain));
	}

	// Non-rustup installs still expose a sysroot through the active `rustc`.
	let output = Command::new("rustc")
		.arg("--print")
		.arg("sysroot")
		.output()
		.ok()?;
	if !output.status.success() {
		return None;
	}
	String::from_utf8(output.stdout)
		.ok()
		.map(|sysroot| sysroot.trim().to_owned())
		.filter(|sysroot| !sysroot.is_empty())
		.map(PathBuf::from)
}

/// Translate the `lint=level` pairs held by `var` into `rustc` lint arguments.
fn level_args(var: &str) -> Result<Vec<String>, String> {
	let Some(value) = env::var(var).ok().filter(|value| !value.is_empty()) else {
		return Ok(Vec::new());
	};

	let mut args = Vec::new();
	for entry in value.split(',') {
		let entry = entry.trim();
		if entry.is_empty() {
			continue;
		}

		let (name, level) = entry
			.split_once('=')
			.ok_or_else(|| format!("invalid `{var}` entry `{entry}`; expected `lint=level`"))?;

		let flag = match level.trim() {
			"allow" => "-A",
			"warn" => "-W",
			"deny" => "-D",
			other => {
				return Err(format!(
					"invalid `{var}` level `{other}` for lint `{name}`; expected `allow`, `warn`, \
					 or `deny`",
				));
			}
		};
		let name = name.trim();
		if name.is_empty() {
			return Err(format!(
				"invalid `{var}` entry `{entry}`; missing lint name"
			));
		}

		args.push(format!("{flag}{name}"));
	}
	Ok(args)
}

/// Return whether primary-package-only linting is enabled.
fn no_deps_enabled() -> bool {
	env::var(PINA_LINT_NO_DEPS).is_ok_and(|value| value != "0")
}

/// Return whether catalog output was requested.
fn list_enabled() -> bool {
	env::var(PINA_LINT_LIST).is_ok_and(|value| value != "0")
}

/// Hash the values of the untracked environment variables.
///
/// The hash is recorded in dep-info through `--env-set`, so any configuration
/// change re-checks the crate even when the source is unchanged.
fn untracked_state() -> Option<String> {
	let mut hasher = DefaultHasher::new();
	for var in UNTRACKED_STATE_VARS {
		env::var(var).unwrap_or_default().hash(&mut hasher);
	}
	Some(format!("{:016x}", hasher.finish()))
}

/// Print the lint catalog in the format Dylint uses for `--list`.
fn list_lints() {
	let name_width = LINTS
		.iter()
		.map(|lint| lint.name.to_lowercase().len())
		.max()
		.unwrap_or_default();
	let level_width = LINTS
		.iter()
		.map(|lint| lint.default_level.as_str().len())
		.max()
		.unwrap_or_default();

	for lint in LINTS {
		println!(
			"    {:<name_width$}    {:<level_width$}    {}",
			lint.name.to_lowercase(),
			lint.default_level.as_str(),
			lint.desc,
			name_width = name_width,
			level_width = level_width,
		);
	}
}
