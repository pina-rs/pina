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

/// Errors produced during end-to-end Codama generation.
#[derive(Debug, thiserror::Error)]
pub enum CodamaError {
	#[error("Failed to read examples directory at {path}: {source}")]
	ReadExamples {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("No examples found in {path}")]
	NoExamples { path: PathBuf },

	#[error("Unknown example `{example}`. Available examples: {available}")]
	UnknownExample { example: String, available: String },

	#[error("Failed to create directory {path}: {source}")]
	CreateDir {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("IDL generation failed for `{example}` ({path}): {source}")]
	GenerateIdl {
		example: String,
		path: PathBuf,
		source: IdlError,
	},

	#[error("Failed to serialize generated IDL for `{example}`: {source}")]
	SerializeIdl {
		example: String,
		source: serde_json::Error,
	},

	#[error("Failed to write generated IDL to {path}: {source}")]
	WriteIdl {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("Rust client rendering failed for {path}: {source}")]
	RenderRust {
		path: PathBuf,
		source: pina_codama_renderer::RenderError,
	},

	#[error("Failed to run `{cmd}`: {source}")]
	RunCommand { cmd: String, source: std::io::Error },

	#[error("`{cmd}` failed with status {status}{details}")]
	CommandFailed {
		cmd: String,
		status: i32,
		details: String,
	},
}
