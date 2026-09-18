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

/// Parses the command surface and runs the requested command.
///
/// On Windows this runs on a worker thread with a reserved stack, because the
/// platform's 1 MiB main-thread default is smaller than parsing this command
/// surface needs. Elsewhere the main thread already reserves enough (8 MiB is
/// the common ulimit), so the CLI starts directly and keeps its startup cost.
#[cfg(not(windows))]
fn main() {
	commands::run(Cli::parse());
}

#[cfg(windows)]
fn main() {
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
