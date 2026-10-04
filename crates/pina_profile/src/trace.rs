//! Register traces recorded by Mollusk's `register-tracing` feature.
//!
//! With `SBF_TRACE_DIR` set, `mollusk-svm` built with `register-tracing`
//! writes one set of files per top-level program invocation. Each set is named
//! by the first 16 hex digits of the SHA-256 of the register trace:
//!
//! | File                | Contents                                                     |
//! | ------------------- | ------------------------------------------------------------ |
//! | `<id>.regs`         | One `[u64; 12]` per executed instruction: `r0`–`r10`, then the program counter as an instruction index into `.text`. |
//! | `<id>.insns`        | The 8-byte relocated instruction at each traced program counter. |
//! | `<id>.program_id`   | The base58 program id.                                       |
//! | `<id>.exec.sha256`  | Lowercase hex SHA-256 of the ELF bytes the program was loaded from. |
//!
//! Every `.regs` entry is one executed SBF instruction and costs one compute
//! unit. Syscall charges are made separately by the runtime and are not part
//! of the trace. A CPI into another SBF program produces its own trace set.
//!
//! Mollusk writes the register words in host byte order. Every platform Pina
//! supports is little-endian, so they are decoded as little-endian.

use std::path::Path;
use std::path::PathBuf;

/// Number of registers Mollusk records per step: `r0`–`r10` plus the PC.
pub const REGISTERS_PER_STEP: usize = 12;

/// Index of the program counter within a recorded register set.
pub const PC_REGISTER: usize = 11;

/// Bytes per recorded register set.
pub const REGISTER_SET_BYTES: usize = REGISTERS_PER_STEP * 8;

/// Bytes per recorded SBF instruction.
pub const INSTRUCTION_BYTES: usize = 8;

/// One executed SBF instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceStep {
	/// Registers before the instruction executed: `r0`–`r10`, then the PC.
	pub registers: [u64; REGISTERS_PER_STEP],
	/// The relocated instruction at the program counter.
	pub instruction: [u8; INSTRUCTION_BYTES],
}

impl TraceStep {
	/// The program counter, as an instruction index into `.text`.
	#[must_use]
	pub fn pc(&self) -> u64 {
		self.registers[PC_REGISTER]
	}
}

/// One top-level program invocation recorded by Mollusk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedTrace {
	/// The 16-hex-digit file stem Mollusk chose.
	pub id: String,
	/// The base58 program id, when recorded.
	pub program_id: Option<String>,
	/// SHA-256 of the loaded ELF, when Mollusk had the program bytes.
	pub executable_sha256: Option<String>,
	/// Executed instructions in order.
	pub steps: Vec<TraceStep>,
}

/// Failures reading recorded traces.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
	#[error("failed to read trace data at {path:?}: {source}")]
	Io {
		path: PathBuf,
		source: std::io::Error,
	},

	#[error("trace {path:?} is malformed: {message}")]
	Malformed { path: PathBuf, message: String },
}

/// Read every trace set in `directory`, ordered by id.
///
/// Tests run in parallel, so recording order is not reproducible; ordering by
/// id keeps the result identical for identical recordings.
///
/// # Errors
///
/// Returns [`TraceError::Io`] when the directory or a required file cannot be
/// read and [`TraceError::Malformed`] when a `.regs`/`.insns` pair is
/// truncated or inconsistent.
pub fn read_trace_dir(directory: &Path) -> Result<Vec<RecordedTrace>, TraceError> {
	let entries = std::fs::read_dir(directory)
		.and_then(|entries| {
			entries
				.map(|entry| entry.map(|entry| entry.path()))
				.collect::<Result<Vec<_>, _>>()
		})
		.map_err(|source| {
			TraceError::Io {
				path: directory.to_path_buf(),
				source,
			}
		})?;
	let mut sets: Vec<(String, PathBuf)> = entries
		.into_iter()
		.filter_map(|path| Some((trace_id(&path)?.to_owned(), path)))
		.collect();

	sets.sort();
	sets.into_iter()
		.map(|(id, regs_path)| read_trace_set(directory, id, &regs_path))
		.collect()
}

/// The trace id of a `<id>.regs` path.
fn trace_id(path: &Path) -> Option<&str> {
	if path.extension()? != "regs" {
		return None;
	}

	path.file_stem()?.to_str()
}

fn read_trace_set(
	directory: &Path,
	id: String,
	regs_path: &Path,
) -> Result<RecordedTrace, TraceError> {
	let insns_path = directory.join(format!("{id}.insns"));
	let registers = read_required(regs_path)?;
	let instructions = read_required(&insns_path)?;
	let steps = parse_steps(&registers, &instructions).map_err(|message| {
		TraceError::Malformed {
			path: regs_path.to_path_buf(),
			message,
		}
	})?;

	Ok(RecordedTrace {
		program_id: read_optional(&directory.join(format!("{id}.program_id")))?,
		executable_sha256: read_optional(&directory.join(format!("{id}.exec.sha256")))?,
		id,
		steps,
	})
}

fn read_required(path: &Path) -> Result<Vec<u8>, TraceError> {
	std::fs::read(path).map_err(|source| {
		TraceError::Io {
			path: path.to_path_buf(),
			source,
		}
	})
}

/// Read a small text companion file, treating a missing file as absent.
fn read_optional(path: &Path) -> Result<Option<String>, TraceError> {
	match std::fs::read_to_string(path) {
		Ok(text) => Ok(Some(text.trim().to_owned())),
		Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(source) => {
			Err(TraceError::Io {
				path: path.to_path_buf(),
				source,
			})
		}
	}
}

/// Decode a `.regs`/`.insns` pair into steps.
///
/// # Errors
///
/// Returns a description of the problem when either buffer has a partial
/// record or the two disagree on the number of executed instructions.
pub fn parse_steps(registers: &[u8], instructions: &[u8]) -> Result<Vec<TraceStep>, String> {
	let (register_sets, partial_set) = registers.as_chunks::<REGISTER_SET_BYTES>();
	let (instruction_words, partial_word) = instructions.as_chunks::<INSTRUCTION_BYTES>();

	if !partial_set.is_empty() {
		return Err(format!(
			"register data is {} bytes, not a multiple of {REGISTER_SET_BYTES}",
			registers.len()
		));
	}

	if !partial_word.is_empty() {
		return Err(format!(
			"instruction data is {} bytes, not a multiple of {INSTRUCTION_BYTES}",
			instructions.len()
		));
	}

	if register_sets.len() != instruction_words.len() {
		return Err(format!(
			"{} register sets but {} instructions",
			register_sets.len(),
			instruction_words.len()
		));
	}

	let steps = register_sets
		.iter()
		.zip(instruction_words)
		.map(|(register_set, instruction)| {
			let (words, _) = register_set.as_chunks::<8>();
			let mut registers = [0_u64; REGISTERS_PER_STEP];

			for (register, word) in registers.iter_mut().zip(words) {
				*register = u64::from_le_bytes(*word);
			}

			TraceStep {
				registers,
				instruction: *instruction,
			}
		})
		.collect();

	Ok(steps)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
		result.unwrap_or_else(|error| panic!("{what}: {error}"))
	}

	fn some<T>(option: Option<T>, what: &str) -> T {
		option.unwrap_or_else(|| panic!("{what}"))
	}

	fn encode(steps: &[([u64; REGISTERS_PER_STEP], [u8; 8])]) -> (Vec<u8>, Vec<u8>) {
		let mut registers = Vec::new();
		let mut instructions = Vec::new();

		for (step_registers, instruction) in steps {
			for register in step_registers {
				registers.extend_from_slice(&register.to_le_bytes());
			}
			instructions.extend_from_slice(instruction);
		}

		(registers, instructions)
	}

	fn registers_with_pc(pc: u64) -> [u64; REGISTERS_PER_STEP] {
		let mut registers = [0; REGISTERS_PER_STEP];
		registers[1] = 0x4_0000_0000;
		registers[PC_REGISTER] = pc;
		registers
	}

	#[test]
	fn parse_steps_decodes_registers_and_instructions() {
		let exit = [0x95, 0, 0, 0, 0, 0, 0, 0];
		let (registers, instructions) = encode(&[(registers_with_pc(7), exit)]);
		let steps = ok(
			parse_steps(&registers, &instructions),
			"valid trace rejected",
		);

		assert_eq!(steps.len(), 1);
		assert_eq!(steps[0].pc(), 7);
		assert_eq!(steps[0].registers[1], 0x4_0000_0000);
		assert_eq!(steps[0].instruction, exit);
	}

	#[test]
	fn parse_steps_rejects_truncated_and_inconsistent_data() {
		let (registers, instructions) = encode(&[(registers_with_pc(0), [0x95; 8])]);

		let truncated_registers = parse_steps(&registers[..95], &instructions)
			.expect_err("partial register set must be rejected");
		assert!(truncated_registers.contains("not a multiple of 96"));

		let truncated_instructions = parse_steps(&registers, &instructions[..7])
			.expect_err("partial instruction must be rejected");
		assert!(truncated_instructions.contains("not a multiple of 8"));

		let mismatched = parse_steps(&registers, &[instructions.clone(), instructions].concat())
			.expect_err("count mismatch must be rejected");
		assert_eq!(mismatched, "1 register sets but 2 instructions");
	}

	#[test]
	fn read_trace_dir_reads_sets_with_optional_companions() {
		let directory = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let (registers, instructions) = encode(&[(registers_with_pc(3), [0x95; 8])]);
		let write = |name: &str, bytes: &[u8]| {
			ok(std::fs::write(directory.path().join(name), bytes), name);
		};
		write("00000000000000aa.regs", &registers);
		write("00000000000000aa.insns", &instructions);
		write("00000000000000aa.program_id", b"Program1111\n");
		write("00000000000000aa.exec.sha256", b"abc123");
		write("00000000000000bb.regs", &registers);
		write("00000000000000bb.insns", &instructions);
		write("program_ids.map", b"ignored");

		let traces = ok(read_trace_dir(directory.path()), "read traces");
		let find = |id: &str| traces.iter().find(|trace| trace.id == id);
		let with_companions = some(find("00000000000000aa"), "trace aa missing");
		let without_companions = some(find("00000000000000bb"), "trace bb missing");

		assert_eq!(traces.len(), 2);
		assert_eq!(with_companions.program_id.as_deref(), Some("Program1111"));
		assert_eq!(with_companions.executable_sha256.as_deref(), Some("abc123"));
		assert_eq!(with_companions.steps[0].pc(), 3);
		assert_eq!(without_companions.program_id, None);
		assert_eq!(without_companions.executable_sha256, None);
	}

	#[test]
	fn read_trace_dir_reports_missing_and_malformed_files() {
		let missing_directory = read_trace_dir(Path::new("/nonexistent/pina-trace"))
			.expect_err("missing directory must fail");
		assert!(matches!(missing_directory, TraceError::Io { .. }));

		let directory = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let regs = directory.path().join("0000000000000001.regs");
		ok(std::fs::write(regs, [0_u8; 96]), "write regs");
		let missing_instructions =
			read_trace_dir(directory.path()).expect_err("missing .insns must fail");
		assert!(
			missing_instructions.to_string().contains(".insns"),
			"{missing_instructions}"
		);

		let insns = directory.path().join("0000000000000001.insns");
		ok(std::fs::write(insns, [0_u8; 3]), "write insns");
		let malformed = read_trace_dir(directory.path()).expect_err("truncated .insns must fail");
		assert!(
			matches!(malformed, TraceError::Malformed { .. }),
			"{malformed}"
		);
	}

	#[test]
	fn read_optional_reports_unreadable_companions() {
		let directory = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		// A directory where a companion file is expected cannot be read as text.
		let error = read_optional(directory.path()).expect_err("directory read must fail");

		assert!(matches!(error, TraceError::Io { .. }));
	}
}
