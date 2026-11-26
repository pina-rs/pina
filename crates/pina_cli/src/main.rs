#![allow(missing_docs)]

mod cli;
mod commands;
mod idl_command;

use clap::Parser;

use crate::cli::Cli;

/// Stack for the CLI worker thread on Windows.
///
/// Clap builds the whole command tree and renders help before parsing, and
/// Windows gives a process's main thread 1 MiB by default. An unoptimized build
/// exhausts that while rendering this command surface, which surfaces as exit
/// code `0xC00000FD` instead of a diagnostic.
#[cfg(windows)]
const CLI_STACK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
enum FastAction {
	Version,
	Docs(Option<String>),
}

fn match_fast_action<I, S>(args: I) -> Option<FastAction>
where
	I: IntoIterator<Item = S>,
	S: Into<std::ffi::OsString>,
{
	let mut iter = args.into_iter().map(Into::into);
	let _program = iter.next();
	let first = iter.next()?;
	let second = iter.next();
	if iter.next().is_some() {
		return None;
	}

	if second.is_none() && (first == "--version" || first == "-V") {
		return Some(FastAction::Version);
	}

	if first == "docs" {
		match second {
			None => return Some(FastAction::Docs(None)),
			Some(topic) => {
				let topic = topic.into_string().ok()?;
				if !topic.starts_with('-') {
					return Some(FastAction::Docs(Some(topic)));
				}
			}
		}
	}

	None
}

/// Fast-path trivial invocations (`--version`, `-V`, `docs [topic]`) before
/// constructing the full 25-subcommand Clap command tree.
fn try_fast_path() -> bool {
	match match_fast_action(std::env::args_os()) {
		Some(FastAction::Version) => {
			println!("pina {}", env!("CARGO_PKG_VERSION"));
			true
		}
		Some(FastAction::Docs(topic)) => {
			commands::run_docs(topic.as_deref());
			true
		}
		None => false,
	}
}

/// Parses the command surface and runs the requested command.
///
/// On Windows this runs on a worker thread with a reserved stack, because the
/// platform's 1 MiB main-thread default is smaller than parsing this command
/// surface needs. Elsewhere the main thread already reserves enough (8 MiB is
/// the common ulimit), so the CLI starts directly and keeps its startup cost.
#[cfg(not(windows))]
fn main() {
	if try_fast_path() {
		return;
	}
	commands::run(Cli::parse());
}

#[cfg(windows)]
fn main() {
	if try_fast_path() {
		return;
	}
	let cli = std::thread::Builder::new()
		.stack_size(CLI_STACK_BYTES)
		.spawn(|| commands::run(Cli::parse()))
		.unwrap_or_else(|error| {
			eprintln!("failed to start the CLI thread: {error}");
			std::process::exit(1);
		});

	if cli.join().is_err() {
		// The worker already printed its panic; exit non-zero so a caller cannot
		// mistake a failed run for a successful one.
		std::process::exit(101);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn match_fast_action_handles_supported_and_fallback_invocations() {
		assert_eq!(match_fast_action(["pina"]), None);
		assert_eq!(
			match_fast_action(["pina", "--version"]),
			Some(FastAction::Version)
		);
		assert_eq!(match_fast_action(["pina", "-V"]), Some(FastAction::Version));
		assert_eq!(match_fast_action(["pina", "--version", "extra"]), None);
		assert_eq!(
			match_fast_action(["pina", "docs"]),
			Some(FastAction::Docs(None))
		);
		assert_eq!(
			match_fast_action(["pina", "docs", "pina-overview"]),
			Some(FastAction::Docs(Some("pina-overview".to_owned())))
		);
		assert_eq!(match_fast_action(["pina", "docs", "--help"]), None);
		assert_eq!(match_fast_action(["pina", "docs", "a", "b"]), None);
		assert_eq!(match_fast_action(["pina", "--help"]), None);
	}
}
