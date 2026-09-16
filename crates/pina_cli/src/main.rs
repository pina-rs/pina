#![allow(missing_docs)]

mod cli;
mod commands;
mod idl_command;

use clap::Parser;

use crate::cli::Cli;

/// Stack for the CLI worker thread.
///
/// Clap builds the whole command tree and renders help before parsing, and
/// Windows gives a process's main thread 1 MiB by default. An unoptimized build
/// exhausts that while rendering this command surface, which surfaces as exit
/// code `0xC00000FD` instead of a diagnostic. Reserving a larger stack keeps the
/// CLI working there without depending on the build profile, and leaves room for
/// the command surface to keep growing.
const CLI_STACK_BYTES: usize = 8 * 1024 * 1024;

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
