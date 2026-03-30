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

#[cfg(test)]
mod tests {
	use super::*;

	fn dummy_io_error() -> std::io::Error {
		std::io::Error::new(std::io::ErrorKind::NotFound, "not found")
	}

	#[test]
	fn idl_error_io_display() {
		let err = IdlError::io("/tmp/test.rs", dummy_io_error());
		let msg = err.to_string();
		assert!(msg.contains("/tmp/test.rs"));
		assert!(msg.contains("not found"));
	}

	#[test]
	fn idl_error_parse_display() {
		let syn_err = syn::Error::new(proc_macro2::Span::call_site(), "bad syntax");
		let err = IdlError::parse("/tmp/lib.rs", &syn_err);
		let msg = err.to_string();
		assert!(msg.contains("/tmp/lib.rs"));
		assert!(msg.contains("bad syntax"));
	}

	#[test]
	fn idl_error_no_program_id_display() {
		let msg = IdlError::NoProgramId.to_string();
		assert!(msg.contains("declare_id!"));
	}

	#[test]
	fn idl_error_no_entrypoint_display() {
		let msg = IdlError::NoEntrypoint.to_string();
		assert!(msg.contains("process_instruction"));
	}

	#[test]
	fn idl_error_unresolved_accounts_display() {
		let err = IdlError::UnresolvedAccounts {
			name: "MyAccounts".to_owned(),
		};
		assert!(err.to_string().contains("MyAccounts"));
	}

	#[test]
	fn idl_error_unresolved_instruction_display() {
		let err = IdlError::UnresolvedInstruction {
			discriminator: "MyIx".to_owned(),
			variant: "Init".to_owned(),
		};
		let msg = err.to_string();
		assert!(msg.contains("MyIx"));
		assert!(msg.contains("Init"));
	}

	#[test]
	fn idl_error_other_display() {
		let err = IdlError::Other("something went wrong".to_owned());
		assert_eq!(err.to_string(), "something went wrong");
	}

	#[test]
	fn codama_error_read_examples_display() {
		let err = CodamaError::ReadExamples {
			path: PathBuf::from("/tmp/examples"),
			source: dummy_io_error(),
		};
		assert!(err.to_string().contains("/tmp/examples"));
	}

	#[test]
	fn codama_error_no_examples_display() {
		let err = CodamaError::NoExamples {
			path: PathBuf::from("/tmp/examples"),
		};
		assert!(err.to_string().contains("/tmp/examples"));
	}

	#[test]
	fn codama_error_unknown_example_display() {
		let err = CodamaError::UnknownExample {
			example: "missing".to_owned(),
			available: "a, b, c".to_owned(),
		};
		let msg = err.to_string();
		assert!(msg.contains("missing"));
		assert!(msg.contains("a, b, c"));
	}

	#[test]
	fn codama_error_command_failed_display() {
		let err = CodamaError::CommandFailed {
			cmd: "npx codama".to_owned(),
			status: 1,
			details: ": permission denied".to_owned(),
		};
		let msg = err.to_string();
		assert!(msg.contains("npx codama"));
		assert!(msg.contains("1"));
		assert!(msg.contains("permission denied"));
	}
}
