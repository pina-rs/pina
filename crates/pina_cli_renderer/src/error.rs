use std::path::PathBuf;

use thiserror::Error;

/// Result alias for rendering operations.
pub type Result<T> = std::result::Result<T, RenderError>;

/// Errors raised while reading an IDL or rendering a CLI crate.
#[derive(Debug, Error)]
pub enum RenderError {
	/// A filesystem path required by rendering could not be read from disk.
	#[error("failed to read `{path}`: {source}")]
	ReadFile {
		path: PathBuf,
		source: std::io::Error,
	},
	#[error("failed to write `{path}`: {source}")]
	WriteFile {
		path: PathBuf,
		source: std::io::Error,
	},
	#[error("failed to parse IDL `{path}` as Codama root node: {source}")]
	ParseIdl {
		path: PathBuf,
		source: serde_json::Error,
	},
	#[error("unsafe generated output path `{path}`: {reason}")]
	UnsafeOutputPath { path: PathBuf, reason: String },
	#[error("cannot {mode} generated CLI at `{path}`: {reason}")]
	InvalidGenerationState {
		path: PathBuf,
		mode: &'static str,
		reason: &'static str,
	},
	#[error("unsupported IDL shape at `{context}`: {reason}")]
	UnsupportedIdl { context: String, reason: String },
	#[error("scaffolded CLI crates need a client package name and path")]
	MissingClientConfig,
}
