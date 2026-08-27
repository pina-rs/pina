mod cli;
mod commands;
mod idl_command;

use clap::Parser;

use crate::cli::Cli;

fn main() {
	commands::run(Cli::parse());
}
