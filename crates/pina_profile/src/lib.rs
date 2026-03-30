//! Static CU profiler for Solana SBF programs.
//!
//! Analyzes compiled `.so` ELF binaries to estimate per-function compute unit
//! costs without requiring a running validator.

pub mod cost;
pub mod elf;
pub mod output;
pub mod sbf;

use std::path::Path;

pub use cost::FunctionProfile;
pub use cost::ProgramProfile;
pub use output::OutputFormat;

/// Profile a compiled SBF program at the given path.
///
/// Returns a [`ProgramProfile`] containing per-function CU estimates and
/// binary metadata.
///
/// # Errors
///
/// Returns a [`ProfileError`] if the file cannot be read, is not a valid ELF,
/// or contains no SBF text sections.
pub fn profile_program(path: &Path) -> Result<ProgramProfile, ProfileError> {
	let data = std::fs::read(path).map_err(|e| {
		ProfileError::Io {
			path: path.to_path_buf(),
			source: e,
		}
	})?;

	let elf_info = elf::parse_elf(&data, path)?;
	let functions = sbf::analyze_functions(&elf_info);
	let total_instructions = functions.iter().map(|f| f.instruction_count).sum();
	let total_syscalls = functions.iter().map(|f| f.syscall_count).sum();
	let total_cu = functions.iter().map(|f| f.estimated_cu).sum();

	Ok(ProgramProfile {
		program_name: elf_info.program_name,
		binary_size: data.len() as u64,
		text_size: elf_info.text_size,
		total_instructions,
		total_syscalls,
		total_cu,
		functions,
	})
}

/// Errors produced during profiling.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
	#[error("IO error at {path}: {source}")]
	Io {
		path: std::path::PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to parse ELF at {path}: {message}")]
	Elf {
		path: std::path::PathBuf,
		message: String,
	},

	#[error("No SBF text section found in {path}")]
	NoTextSection { path: std::path::PathBuf },
}
