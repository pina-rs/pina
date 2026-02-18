use std::path::PathBuf;

/// Errors produced during IDL generation.
#[derive(Debug, thiserror::Error)]
pub enum IdlError {
	#[error("IO error at {path}: {source}")]
	Io {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Failed to parse Rust source in {path}: {message}")]
	Parse { path: PathBuf, message: String },

	#[error("No program ID found (declare_id! macro missing)")]
	NoProgramId,

	#[error("No entrypoint dispatch found (process_instruction match missing)")]
	NoEntrypoint,

	#[error("Could not resolve accounts struct `{name}` referenced in entrypoint dispatch")]
	UnresolvedAccounts { name: String },

	#[error(
		"Could not resolve instruction struct for variant `{variant}` of discriminator \
		 `{discriminator}`"
	)]
	UnresolvedInstruction {
		discriminator: String,
		variant: String,
	},

	#[error("{0}")]
	Other(String),
}

impl IdlError {
	pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
		Self::Io {
			path: path.into(),
			source,
		}
	}

	pub fn parse(path: impl Into<PathBuf>, err: &syn::Error) -> Self {
		Self::Parse {
			path: path.into(),
			message: err.to_string(),
		}
	}
}
