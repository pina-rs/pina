use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, RenderError>;

#[derive(Debug, Error)]
pub enum RenderError {
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
	#[error("unsupported type `{kind}` at `{context}`: {reason}")]
	UnsupportedType {
		context: String,
		kind: &'static str,
		reason: String,
	},
	#[error("unsupported value `{kind}` at `{context}`: {reason}")]
	UnsupportedValue {
		context: String,
		kind: &'static str,
		reason: String,
	},
	#[error("missing required discriminator for `{context}`")]
	MissingDiscriminator { context: String },
	#[error("unsupported discriminator for `{context}`: {reason}")]
	UnsupportedDiscriminator { context: String, reason: String },
	#[error("missing PDA `{pda}` for account `{account}`")]
	MissingPda { account: String, pda: String },
	#[error("failed to run command `{command}`: {source}")]
	CommandExec {
		command: String,
		source: std::io::Error,
	},
	#[error("command `{command}` failed with status {status}")]
	CommandFailed { command: String, status: i32 },
}
