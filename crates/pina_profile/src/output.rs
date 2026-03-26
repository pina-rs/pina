//! Output formatting for profile results.

use std::io::Write;

use crate::cost::ProgramProfile;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
	/// Human-readable text summary.
	Text,
	/// Machine-readable JSON.
	Json,
}

/// Write the profile in the requested format.
///
/// # Errors
///
/// Returns an IO error if writing fails, or a serialization error if JSON
/// formatting fails.
pub fn write_profile(
	profile: &ProgramProfile,
	format: OutputFormat,
	writer: &mut dyn Write,
) -> Result<(), OutputError> {
	match format {
		OutputFormat::Text => write_text(profile, writer),
		OutputFormat::Json => write_json(profile, writer),
	}
}

fn write_text(profile: &ProgramProfile, w: &mut dyn Write) -> Result<(), OutputError> {
	writeln!(w, "Program: {}", profile.program_name)?;
	writeln!(w, "Binary size: {} bytes", profile.binary_size)?;
	writeln!(w, "Text section: {} bytes", profile.text_size)?;
	writeln!(w, "Total instructions: {}", profile.total_instructions)?;
	writeln!(w, "Total estimated CU: {}", profile.total_cu)?;
	writeln!(w)?;

	if profile.functions.is_empty() {
		writeln!(w, "No functions found.")?;
		return Ok(());
	}

	// Column header.
	writeln!(w, "{:<50} {:>10} {:>10}", "Function", "Instrs", "Est. CU")?;
	writeln!(w, "{}", "-".repeat(72))?;

	for func in &profile.functions {
		writeln!(
			w,
			"{:<50} {:>10} {:>10}",
			truncate_name(&func.name, 50),
			func.instruction_count,
			func.estimated_cu,
		)?;
	}

	Ok(())
}

fn write_json(profile: &ProgramProfile, w: &mut dyn Write) -> Result<(), OutputError> {
	let json = serde_json::to_string_pretty(profile).map_err(OutputError::Json)?;
	write!(w, "{json}")?;
	Ok(())
}

/// Truncate a function name to fit in a column, adding `..` if needed.
fn truncate_name(name: &str, max_len: usize) -> String {
	if name.len() <= max_len {
		name.to_owned()
	} else if max_len <= 2 {
		name.chars().take(max_len).collect()
	} else {
		let truncated: String = name.chars().take(max_len - 2).collect();
		format!("{truncated}..")
	}
}

/// Errors during output formatting.
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("JSON serialization error: {0}")]
	Json(serde_json::Error),
}

#[cfg(test)]
mod tests {
	use crate::cost::FunctionProfile;

	use super::*;

	fn sample_profile() -> ProgramProfile {
		ProgramProfile {
			program_name: "my_program".to_owned(),
			binary_size: 1024,
			text_size: 800,
			total_instructions: 100,
			total_cu: 100,
			functions: vec![
				FunctionProfile {
					name: "process_instruction".to_owned(),
					offset: 0,
					size: 640,
					instruction_count: 80,
					estimated_cu: 80,
				},
				FunctionProfile {
					name: "helper".to_owned(),
					offset: 640,
					size: 160,
					instruction_count: 20,
					estimated_cu: 20,
				},
			],
		}
	}

	#[test]
	fn text_output_contains_program_name() {
		let profile = sample_profile();
		let mut buf = Vec::new();
		write_profile(&profile, OutputFormat::Text, &mut buf).unwrap();
		let output = String::from_utf8(buf).unwrap();
		assert!(output.contains("my_program"));
		assert!(output.contains("1024"));
		assert!(output.contains("process_instruction"));
		assert!(output.contains("helper"));
	}

	#[test]
	fn text_output_shows_totals() {
		let profile = sample_profile();
		let mut buf = Vec::new();
		write_profile(&profile, OutputFormat::Text, &mut buf).unwrap();
		let output = String::from_utf8(buf).unwrap();
		assert!(output.contains("Total instructions: 100"));
		assert!(output.contains("Total estimated CU: 100"));
	}

	#[test]
	fn json_output_is_valid() {
		let profile = sample_profile();
		let mut buf = Vec::new();
		write_profile(&profile, OutputFormat::Json, &mut buf).unwrap();
		let output = String::from_utf8(buf).unwrap();
		let parsed: serde_json::Value =
			serde_json::from_str(&output).unwrap_or_else(|e| panic!("Invalid JSON: {e}"));
		assert_eq!(parsed["program_name"], "my_program");
		assert_eq!(parsed["total_cu"], 100);
		assert!(parsed["functions"].is_array());
	}

	#[test]
	fn empty_functions_handled() {
		let profile = ProgramProfile {
			program_name: "empty".to_owned(),
			binary_size: 0,
			text_size: 0,
			total_instructions: 0,
			total_cu: 0,
			functions: vec![],
		};
		let mut buf = Vec::new();
		write_profile(&profile, OutputFormat::Text, &mut buf).unwrap();
		let output = String::from_utf8(buf).unwrap();
		assert!(output.contains("No functions found."));
	}

	#[test]
	fn truncate_name_short() {
		assert_eq!(truncate_name("hello", 50), "hello");
	}

	#[test]
	fn truncate_name_exact() {
		let name = "a".repeat(50);
		assert_eq!(truncate_name(&name, 50), name);
	}

	#[test]
	fn truncate_name_long() {
		let name = "a".repeat(60);
		let result = truncate_name(&name, 50);
		assert_eq!(result.len(), 50);
		assert!(result.ends_with(".."));
	}

	#[test]
	fn json_contains_all_fields() {
		let profile = sample_profile();
		let mut buf = Vec::new();
		write_profile(&profile, OutputFormat::Json, &mut buf).unwrap();
		let output = String::from_utf8(buf).unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
		assert!(parsed["binary_size"].is_number());
		assert!(parsed["text_size"].is_number());
		assert!(parsed["total_instructions"].is_number());

		let func = &parsed["functions"][0];
		assert!(func["name"].is_string());
		assert!(func["offset"].is_number());
		assert!(func["size"].is_number());
		assert!(func["instruction_count"].is_number());
		assert!(func["estimated_cu"].is_number());
	}
}
