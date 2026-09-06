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
	#[error("unsafe generated output path `{path}`: {reason}")]
	UnsafeOutputPath { path: PathBuf, reason: String },
	#[error("generated Rust source `{path}` is invalid: {reason}")]
	InvalidGeneratedSource { path: PathBuf, reason: String },
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
	#[error("unsupported account `{account}` at `{context}`: {reason}")]
	UnsupportedAccount {
		context: String,
		account: String,
		reason: String,
	},
	#[error("missing required discriminator for `{context}`")]
	MissingDiscriminator { context: String },
	#[error("unsupported discriminator for `{context}`: {reason}")]
	UnsupportedDiscriminator { context: String, reason: String },
}
