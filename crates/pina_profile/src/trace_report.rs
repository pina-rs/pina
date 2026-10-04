//! Dynamic compute-unit profiles built from recorded register traces.
//!
//! Every recorded step is one executed SBF instruction and costs one compute
//! unit, so counting steps per source line, function, and call stack shows
//! where an instruction's CU went. Syscall charges are made separately by the
//! runtime and are not in the trace: syscalls are reported as invocation counts
//! at the line that made them, and only their 1-CU `call` is counted.
//!
//! ## Call stacks
//!
//! Physical frames are rebuilt by following the trace: an internal `call` or
//! `callx` pushes the call site, `exit` pops it, and a syscall returns to the
//! next instruction. Each physical frame then expands into the inlined frames
//! DWARF records for its program counter (see [`crate::dwarf`]).

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Range;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::ProfileError;
use crate::dwarf::Frame;
use crate::dwarf::SourceLocation;
use crate::dwarf::Symbolizer;
use crate::elf;
use crate::syscalls;
use crate::trace::RecordedTrace;
use crate::trace::TraceStep;

/// Schema version of the [`TraceReport`] JSON document.
pub const TRACE_SCHEMA_VERSION: u32 = 1;

/// `call imm`: a syscall or an internal call, depending on the relocated key.
const CALL_IMM: u8 = 0x85;
/// `callx`: an internal call through a register.
const CALL_REG: u8 = 0x8d;
/// `exit`: return to the caller, or end the program at depth 0.
const EXIT: u8 = 0x95;

/// The SBF input region, where the loader serializes accounts and
/// instruction data.
const INPUT_REGION: Range<u64> = 0x4_0000_0000..0x5_0000_0000;

/// The traced build that recordings are attributed to.
#[derive(Debug)]
pub struct TracedProgram {
	/// Program name, from the executable's file name.
	pub name: String,
	/// Lowercase hex SHA-256 of the executable the tests loaded.
	pub executable_sha256: String,
	/// Function and line attribution from the unstripped build.
	pub symbolizer: Symbolizer,
}

impl TracedProgram {
	/// Load the executable the tests ran and the unstripped build with DWARF.
	///
	/// Mollusk cannot load an ELF whose `.symtab` holds long Rust symbol
	/// names while tracing, so tests load the stripped `executable` and
	/// attribution reads `debug_executable`, which must have the same `.text`.
	///
	/// # Errors
	///
	/// Returns an error when either file cannot be read or parsed, or when
	/// their `.text` sections differ.
	pub fn load(executable: &Path, debug_executable: &Path) -> Result<Self, ProfileError> {
		let executable_bytes = read(executable)?;
		let debug_bytes = read(debug_executable)?;
		let loaded = elf::parse_elf(&executable_bytes, executable)?;
		let debug = elf::parse_elf(&debug_bytes, debug_executable)?;

		if loaded.text_vaddr != debug.text_vaddr || loaded.text_bytes != debug.text_bytes {
			return Err(ProfileError::DebugBuildMismatch {
				executable: executable.to_path_buf(),
				debug: debug_executable.to_path_buf(),
			});
		}

		Ok(Self {
			name: loaded.program_name,
			executable_sha256: sha256_hex(&executable_bytes),
			symbolizer: Symbolizer::new(&debug_bytes, debug_executable)?,
		})
	}
}

fn read(path: &Path) -> Result<Vec<u8>, ProfileError> {
	std::fs::read(path).map_err(|source| {
		ProfileError::Io {
			path: path.to_path_buf(),
			source,
		}
	})
}

/// Lowercase hex SHA-256, the form Mollusk writes to `.exec.sha256`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
	const DIGITS: &[u8; 16] = b"0123456789abcdef";

	Sha256::digest(bytes)
		.iter()
		.flat_map(|byte| {
			[
				DIGITS[usize::from(byte >> 4)],
				DIGITS[usize::from(byte & 0x0f)],
			]
		})
		.map(char::from)
		.collect()
}

/// A dynamic profile of every traced instruction of one program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceReport {
	/// Always [`TRACE_SCHEMA_VERSION`].
	pub schema_version: u32,
	/// Program name.
	pub program: String,
	/// SHA-256 of the traced executable.
	pub executable_sha256: String,
	/// Whether the traced build carried a DWARF line table.
	pub line_info: bool,
	/// Trace sets found, including other programs'.
	pub recorded_traces: usize,
	/// Trace sets that executed a different program or build.
	pub skipped_traces: usize,
	/// How the traced build's code compares with the release build, when known.
	pub release_build: Option<ReleaseComparison>,
	/// One profile per distinct execution path, in recording order.
	pub instructions: Vec<InstructionProfile>,
}

/// Whether debug information changed the traced build's code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseComparison {
	/// The two `.text` sections are byte-identical.
	pub identical_text: bool,
	/// Instruction slots in the traced build's `.text`.
	pub traced_instructions: u64,
	/// Instruction slots in the release build's `.text`.
	pub release_instructions: u64,
	/// Slots that differ, counting any length difference.
	pub differing_instructions: u64,
}

impl ReleaseComparison {
	/// Compare the `.text` sections of a traced build and a release build.
	///
	/// # Errors
	///
	/// Returns an error when either file cannot be read or parsed.
	pub fn between(traced: &Path, release: &Path) -> Result<Self, ProfileError> {
		let traced_info = elf::parse_elf(&read(traced)?, traced)?;
		let release_info = elf::parse_elf(&read(release)?, release)?;

		Ok(Self::of(&traced_info.text_bytes, &release_info.text_bytes))
	}

	/// Compare the `.text` bytes of the traced and release builds slot by slot.
	#[must_use]
	pub fn of(traced_text: &[u8], release_text: &[u8]) -> Self {
		let traced_instructions = (traced_text.len() / 8) as u64;
		let release_instructions = (release_text.len() / 8) as u64;
		let differing_pairs = traced_text
			.chunks(8)
			.zip(release_text.chunks(8))
			.filter(|(traced, release)| traced != release)
			.count() as u64;

		Self {
			identical_text: traced_text == release_text,
			traced_instructions,
			release_instructions,
			differing_instructions: differing_pairs
				+ traced_instructions.abs_diff(release_instructions),
		}
	}
}

/// The leading instruction-data bytes a trace loaded, read back from the
/// register that received them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedDiscriminator {
	/// Width of the load in bytes: 1, 2, 4, or 8.
	pub width: u8,
	/// The loaded value, little-endian.
	pub value: u64,
}

/// The profile of one distinct execution path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionProfile {
	/// Unique display label: the instruction name, or `trace <id>`.
	pub name: String,
	/// The program instruction whose discriminator the trace loaded.
	pub instruction: Option<String>,
	/// The discriminator bytes the trace loaded, when observed.
	pub discriminator: Option<ObservedDiscriminator>,
	/// Every recording that followed exactly this path.
	pub trace_ids: Vec<String>,
	/// Executed SBF instructions, one CU each.
	pub executed_instructions: u64,
	/// Syscalls made; their runtime charges are not included.
	pub syscall_invocations: u64,
	/// Executed instructions with no source line.
	pub unattributed_instructions: u64,
	/// Cost per source line, most expensive first.
	pub lines: Vec<LineCost>,
	/// Cost per function, by self cost.
	pub functions: Vec<FunctionCost>,
	/// Syscall invocations by name.
	pub syscalls: Vec<SyscallCount>,
	/// Cost per call stack, outermost frame first, sorted by frames.
	pub stacks: Vec<StackCost>,
}

/// Executed instructions attributed to one source line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineCost {
	/// Path as recorded in the debug information.
	pub file: String,
	/// 1-based line number.
	pub line: u32,
	/// Executed instructions whose innermost frame is this line.
	pub executed_instructions: u64,
	/// Syscalls made from this line.
	pub syscalls: Vec<SyscallCount>,
}

/// Executed instructions attributed to one function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCost {
	/// Function name: demangled for physical frames, DWARF's name for inlined ones.
	pub name: String,
	/// Instructions executed while the function was the innermost frame.
	pub self_instructions: u64,
	/// Instructions executed while the function was anywhere on the stack.
	pub inclusive_instructions: u64,
}

/// Invocations of one syscall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyscallCount {
	/// Syscall name, or `syscall 0x<key>` for an unknown key.
	pub name: String,
	/// Number of invocations.
	pub invocations: u64,
}

/// Executed instructions attributed to one call stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackCost {
	/// Function names, outermost first.
	pub frames: Vec<String>,
	/// Instructions executed with exactly this stack.
	pub executed_instructions: u64,
}

impl TraceReport {
	/// Name and order the profiles from their observed discriminators.
	///
	/// `resolve` maps a discriminator to the program instruction it selects.
	/// Profiles are ordered by instruction name (unmatched last), then by
	/// executed instructions (most first), then by trace id, so identical
	/// recordings always produce the same report. Unmatched profiles are
	/// labelled `trace <id>`, and repeated labels get a ` #2`, ` #3`, ...
	/// suffix in that order.
	pub fn assign_names(&mut self, resolve: impl Fn(&ObservedDiscriminator) -> Option<String>) {
		for profile in &mut self.instructions {
			profile.instruction = profile.discriminator.as_ref().and_then(&resolve);
		}

		self.instructions.sort_by(|left, right| {
			left.instruction
				.is_none()
				.cmp(&right.instruction.is_none())
				.then_with(|| left.instruction.cmp(&right.instruction))
				.then_with(|| right.executed_instructions.cmp(&left.executed_instructions))
				.then_with(|| left.trace_ids.cmp(&right.trace_ids))
		});

		let mut seen: HashMap<String, usize> = HashMap::new();

		for profile in &mut self.instructions {
			let base = profile.instruction.clone().unwrap_or_else(|| {
				format!(
					"trace {}",
					profile.trace_ids.first().map_or("", String::as_str)
				)
			});
			let count = seen.entry(base.clone()).or_insert(0);
			*count += 1;
			profile.name = if *count == 1 {
				base
			} else {
				format!("{base} #{count}")
			};
		}
	}
}

/// Profile every recording that executed `program`.
///
/// Recordings with identical program-counter sequences followed the same
/// path and are reported once, listing every trace id. Profiles are named
/// `trace <id>` until [`TraceReport::assign_names`] resolves instructions.
#[must_use]
pub fn analyze_traces(traces: &[RecordedTrace], program: &TracedProgram) -> TraceReport {
	let matching = traces.iter().filter(|trace| {
		trace.executable_sha256.as_deref() == Some(program.executable_sha256.as_str())
	});
	let mut groups: Vec<Vec<&RecordedTrace>> = Vec::new();
	let mut paths: HashMap<Vec<u64>, usize> = HashMap::new();

	for trace in matching {
		let path: Vec<u64> = trace.steps.iter().map(TraceStep::pc).collect();

		let next_group = groups.len();
		let group = *paths.entry(path).or_insert(next_group);

		if group == next_group {
			groups.push(Vec::new());
		}

		groups[group].push(trace);
	}

	let matched: usize = groups.iter().map(Vec::len).sum();
	let mut report = TraceReport {
		schema_version: TRACE_SCHEMA_VERSION,
		program: program.name.clone(),
		executable_sha256: program.executable_sha256.clone(),
		line_info: program.symbolizer.has_line_info(),
		recorded_traces: traces.len(),
		skipped_traces: traces.len() - matched,
		release_build: None,
		instructions: groups
			.iter()
			.map(|group| profile_group(group, &program.symbolizer))
			.collect(),
	};
	report.assign_names(|_| None);
	report
}

/// The leading instruction-data bytes the program loaded.
///
/// With SIMD-0321 the loader passes the instruction-data address in `r2`.
/// The first load from exactly that address reads the discriminator, and the
/// next step's destination register holds the loaded value. Returns `None`
/// when `r2` does not point into the input region (an older loader) or the
/// program never loads from it.
#[must_use]
pub fn observed_discriminator(steps: &[TraceStep]) -> Option<ObservedDiscriminator> {
	let data_address = steps.first()?.registers[2];

	if !INPUT_REGION.contains(&data_address) {
		return None;
	}

	steps.windows(2).find_map(|window| {
		let (step, next) = (window[0], window[1]);
		let instruction = step.instruction;
		let width: u8 = match instruction[0] {
			0x71 => 1,
			0x69 => 2,
			0x61 => 4,
			0x79 => 8,
			_ => return None,
		};
		let destination = usize::from(instruction[1] & 0x0f);
		let source = usize::from(instruction[1] >> 4);

		if destination > 10 || source > 10 {
			return None;
		}

		let offset = i64::from(i16::from_le_bytes([instruction[2], instruction[3]]));
		let address = step.registers[source].wrapping_add_signed(offset);

		if address != data_address {
			return None;
		}

		let mask = u64::MAX >> (64 - u32::from(width) * 8);
		Some(ObservedDiscriminator {
			width,
			value: next.registers[destination] & mask,
		})
	})
}

/// How a `call imm` transferred control.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CallKind {
	Internal,
	Syscall(String),
}

/// Classify a `call imm` from its relocated key and where execution went next.
///
/// A known syscall key is a syscall. Otherwise the call is internal unless
/// execution simply continued at the next instruction without the key naming
/// that instruction as a function, which means an unknown syscall returned.
fn classify_call(instruction: [u8; 8], pc: u64, next_pc: Option<u64>) -> CallKind {
	let key = u32::from_le_bytes([
		instruction[4],
		instruction[5],
		instruction[6],
		instruction[7],
	]);

	if let Some(name) = syscalls::syscall_name(key) {
		return CallKind::Syscall(name.to_owned());
	}

	let returned_inline =
		next_pc.is_some_and(|next| next == pc + 1 && key != syscalls::internal_call_key(next));

	if returned_inline {
		return CallKind::Syscall(format!("syscall 0x{key:08x}"));
	}

	CallKind::Internal
}

/// Interned caller chains: each chain is a call-site PC plus the chain that
/// was active when the call was made.
#[derive(Debug, Default)]
struct CallerChains {
	links: Vec<(Option<usize>, u64)>,
	index: HashMap<(Option<usize>, u64), usize>,
}

impl CallerChains {
	fn push(&mut self, parent: Option<usize>, call_pc: u64) -> usize {
		let links = &mut self.links;
		*self.index.entry((parent, call_pc)).or_insert_with(|| {
			links.push((parent, call_pc));
			links.len() - 1
		})
	}

	/// Call-site PCs of a chain, outermost first.
	fn call_sites(&self, mut chain: Option<usize>) -> Vec<u64> {
		let mut sites = Vec::new();

		while let Some(link) = chain {
			let (parent, call_pc) = self.links[link];
			sites.push(call_pc);
			chain = parent;
		}

		sites.reverse();
		sites
	}
}

/// Executed-instruction counts keyed by caller chain and PC, plus syscalls by PC.
#[derive(Debug, Default)]
struct PathCounts {
	chains: CallerChains,
	steps: HashMap<(Option<usize>, u64), u64>,
	syscalls: HashMap<u64, BTreeMap<String, u64>>,
}

fn count_path(steps: &[TraceStep]) -> PathCounts {
	let mut counts = PathCounts::default();
	let mut current: Option<usize> = None;
	let mut returns: Vec<Option<usize>> = Vec::new();

	for (index, step) in steps.iter().enumerate() {
		let pc = step.pc();
		*counts.steps.entry((current, pc)).or_default() += 1;

		let next_pc = steps.get(index + 1).map(TraceStep::pc);
		let call = match step.instruction[0] {
			CALL_IMM => classify_call(step.instruction, pc, next_pc),
			CALL_REG => CallKind::Internal,
			EXIT => {
				if let Some(caller) = returns.pop() {
					current = caller;
				}
				continue;
			}
			_ => continue,
		};

		match call {
			CallKind::Internal => {
				returns.push(current);
				current = Some(counts.chains.push(current, pc));
			}
			CallKind::Syscall(name) => {
				*counts
					.syscalls
					.entry(pc)
					.or_default()
					.entry(name)
					.or_default() += 1;
			}
		}
	}

	counts
}

fn profile_group(group: &[&RecordedTrace], symbolizer: &Symbolizer) -> InstructionProfile {
	let steps = group
		.first()
		.map_or(&[][..], |trace| trace.steps.as_slice());
	let counts = count_path(steps);
	let mut frame_cache: HashMap<u64, Vec<Frame>> = HashMap::new();
	let mut frames = |pc: u64| -> Vec<Frame> {
		frame_cache
			.entry(pc)
			.or_insert_with(|| symbolizer.frames(pc))
			.clone()
	};
	let mut lines: HashMap<SourceLocation, u64> = HashMap::new();
	let mut self_costs: HashMap<String, u64> = HashMap::new();
	let mut inclusive_costs: HashMap<String, u64> = HashMap::new();
	let mut stacks: BTreeMap<Vec<String>, u64> = BTreeMap::new();
	let mut unattributed = 0_u64;

	for (&(chain, pc), &count) in &counts.steps {
		let own = frames(pc);
		let stack: Vec<String> = counts
			.chains
			.call_sites(chain)
			.into_iter()
			.flat_map(&mut frames)
			.chain(own.iter().cloned())
			.map(|frame| frame.function)
			.collect();

		match own.last().and_then(|frame| frame.location.clone()) {
			Some(location) => *lines.entry(location).or_default() += count,
			None => unattributed += count,
		}

		let innermost = stack.last().cloned().unwrap_or_default();
		*self_costs.entry(innermost).or_default() += count;

		let mut seen = HashSet::new();
		for name in &stack {
			if seen.insert(name.as_str()) {
				*inclusive_costs.entry(name.clone()).or_default() += count;
			}
		}

		*stacks.entry(stack).or_default() += count;
	}

	let mut line_syscalls: HashMap<SourceLocation, BTreeMap<String, u64>> = HashMap::new();
	let mut syscall_totals: BTreeMap<String, u64> = BTreeMap::new();

	for (pc, by_name) in &counts.syscalls {
		let location = frames(*pc).last().and_then(|frame| frame.location.clone());

		for (name, invocations) in by_name {
			*syscall_totals.entry(name.clone()).or_default() += invocations;

			if let Some(location) = &location {
				*line_syscalls
					.entry(location.clone())
					.or_default()
					.entry(name.clone())
					.or_default() += invocations;
			}
		}
	}

	InstructionProfile {
		name: String::new(),
		instruction: None,
		discriminator: observed_discriminator(steps),
		trace_ids: group.iter().map(|trace| trace.id.clone()).collect(),
		executed_instructions: steps.len() as u64,
		syscall_invocations: syscall_totals.values().sum(),
		unattributed_instructions: unattributed,
		lines: line_costs(lines, &mut line_syscalls),
		functions: function_costs(&self_costs, inclusive_costs),
		syscalls: syscall_counts(syscall_totals),
		stacks: stacks
			.into_iter()
			.map(|(frames, executed_instructions)| {
				StackCost {
					frames,
					executed_instructions,
				}
			})
			.collect(),
	}
}

fn line_costs(
	lines: HashMap<SourceLocation, u64>,
	line_syscalls: &mut HashMap<SourceLocation, BTreeMap<String, u64>>,
) -> Vec<LineCost> {
	let mut costs: Vec<LineCost> = lines
		.into_iter()
		.map(|(location, executed_instructions)| {
			let syscalls = line_syscalls.remove(&location).unwrap_or_default();
			LineCost {
				file: location.file,
				line: location.line,
				executed_instructions,
				syscalls: syscall_counts(syscalls),
			}
		})
		.collect();

	costs.sort_by(|left, right| {
		right
			.executed_instructions
			.cmp(&left.executed_instructions)
			.then_with(|| left.file.cmp(&right.file))
			.then_with(|| left.line.cmp(&right.line))
	});
	costs
}

fn function_costs(
	self_costs: &HashMap<String, u64>,
	inclusive_costs: HashMap<String, u64>,
) -> Vec<FunctionCost> {
	let mut costs: Vec<FunctionCost> = inclusive_costs
		.into_iter()
		.map(|(name, inclusive_instructions)| {
			FunctionCost {
				self_instructions: self_costs.get(&name).copied().unwrap_or(0),
				name,
				inclusive_instructions,
			}
		})
		.collect();

	costs.sort_by(|left, right| {
		right
			.self_instructions
			.cmp(&left.self_instructions)
			.then_with(|| {
				right
					.inclusive_instructions
					.cmp(&left.inclusive_instructions)
			})
			.then_with(|| left.name.cmp(&right.name))
	});
	costs
}

fn syscall_counts(totals: BTreeMap<String, u64>) -> Vec<SyscallCount> {
	let mut counts: Vec<SyscallCount> = totals
		.into_iter()
		.map(|(name, invocations)| SyscallCount { name, invocations })
		.collect();

	// The map is ordered by name, so a stable sort keeps names ordered within
	// equal counts.
	counts.sort_by_key(|count| std::cmp::Reverse(count.invocations));
	counts
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::trace::PC_REGISTER;
	use crate::trace::REGISTERS_PER_STEP;

	fn step(pc: u64, instruction: [u8; 8]) -> TraceStep {
		let mut registers = [0; REGISTERS_PER_STEP];
		registers[PC_REGISTER] = pc;
		TraceStep {
			registers,
			instruction,
		}
	}

	fn call_imm(key: u32) -> [u8; 8] {
		let key = key.to_le_bytes();
		[CALL_IMM, 0x10, 0, 0, key[0], key[1], key[2], key[3]]
	}

	const NOP: [u8; 8] = [0x07, 0, 0, 0, 1, 0, 0, 0];
	const EXIT_INSTRUCTION: [u8; 8] = [EXIT, 0, 0, 0, 0, 0, 0, 0];
	const CALLX: [u8; 8] = [CALL_REG, 0x01, 0, 0, 0, 0, 0, 0];

	#[test]
	fn classify_call_distinguishes_syscalls_from_internal_calls() {
		let sol_log = syscalls::murmur3_32(b"sol_log_");

		assert_eq!(
			classify_call(call_imm(sol_log), 4, Some(5)),
			CallKind::Syscall("sol_log_".to_owned())
		);
		assert_eq!(
			classify_call(call_imm(0xdead_beef), 4, Some(5)),
			CallKind::Syscall("syscall 0xdeadbeef".to_owned())
		);
		assert_eq!(
			classify_call(call_imm(syscalls::internal_call_key(9)), 4, Some(9)),
			CallKind::Internal
		);
		// A call into the very next instruction is internal when the key names it.
		assert_eq!(
			classify_call(call_imm(syscalls::internal_call_key(5)), 4, Some(5)),
			CallKind::Internal
		);
		// A trailing call whose effect was never observed changes nothing.
		assert_eq!(
			classify_call(call_imm(0xdead_beef), 4, None),
			CallKind::Internal
		);
	}

	#[test]
	fn count_path_rebuilds_call_stacks_and_counts_syscalls() {
		let sol_log = call_imm(syscalls::murmur3_32(b"sol_log_"));
		// main(0) calls helper(10), which calls through callx into leaf(20),
		// then main logs and exits.
		let steps = [
			step(0, call_imm(syscalls::internal_call_key(10))),
			step(10, CALLX),
			step(20, NOP),
			step(21, EXIT_INSTRUCTION),
			step(11, EXIT_INSTRUCTION),
			step(1, sol_log),
			step(2, EXIT_INSTRUCTION),
		];
		let counts = count_path(&steps);
		let chain_of = |pc: u64| {
			let key = counts.steps.keys().find(|(_, key_pc)| *key_pc == pc);
			let sites = key.map(|(chain, _)| counts.chains.call_sites(*chain));
			sites.unwrap_or_else(|| panic!("pc {pc} was not counted"))
		};

		assert_eq!(chain_of(0), Vec::<u64>::new());
		assert_eq!(chain_of(10), vec![0]);
		assert_eq!(chain_of(20), vec![0, 10]);
		assert_eq!(chain_of(11), vec![0]);
		assert_eq!(chain_of(1), Vec::<u64>::new());
		assert_eq!(counts.syscalls[&1]["sol_log_"], 1);
		assert_eq!(counts.steps.values().sum::<u64>(), 7);
		// Interning reuses a chain for a repeated call from the same site.
		let mut chains = CallerChains::default();
		assert_eq!(chains.push(None, 3), chains.push(None, 3));
	}

	#[test]
	fn exit_at_depth_zero_keeps_the_root_stack() {
		let steps = [step(0, EXIT_INSTRUCTION), step(1, NOP)];
		let counts = count_path(&steps);

		assert!(counts.steps.keys().all(|(chain, _)| chain.is_none()));
	}

	fn load(pc: u64, opcode: u8, destination: u8, source: u8, offset: i16) -> TraceStep {
		let offset = offset.to_le_bytes();
		step(
			pc,
			[
				opcode,
				(source << 4) | destination,
				offset[0],
				offset[1],
				0,
				0,
				0,
				0,
			],
		)
	}

	#[test]
	fn observed_discriminator_reads_the_first_load_from_r2() {
		let data = 0x4_0000_50e0;
		let mut first = load(0, 0x79, 4, 2, -8); // length prefix at r2 - 8
		first.registers[2] = data;
		let mut length = load(1, 0x71, 5, 2, 0);
		length.registers[2] = data;
		length.registers[4] = 1;
		let mut loaded = step(2, NOP);
		loaded.registers[5] = 0x1ff; // only the low byte was loaded

		let discriminator = observed_discriminator(&[first, length, loaded]);

		assert_eq!(
			discriminator,
			Some(ObservedDiscriminator {
				width: 1,
				value: 0xff,
			})
		);
	}

	#[test]
	fn observed_discriminator_handles_every_width_and_missing_data() {
		let data = 0x4_0000_0010;
		for (opcode, width, mask) in [
			(0x69_u8, 2_u8, 0xffff_u64),
			(0x61, 4, 0xffff_ffff),
			(0x79, 8, u64::MAX),
		] {
			let mut first = load(0, opcode, 3, 1, 0x10);
			first.registers[1] = 0x4_0000_0000;
			first.registers[2] = data;
			let mut next = step(1, NOP);
			next.registers[3] = u64::MAX;

			assert_eq!(
				observed_discriminator(&[first, next]),
				Some(ObservedDiscriminator { width, value: mask })
			);
		}

		// r2 outside the input region: a loader without SIMD-0321.
		assert_eq!(observed_discriminator(&[step(0, NOP), step(1, NOP)]), None);
		// Invalid register numbers never index past the register file.
		let mut invalid = load(0, 0x71, 11, 2, 0);
		invalid.registers[2] = data;
		assert_eq!(observed_discriminator(&[invalid, step(1, NOP)]), None);
		assert_eq!(observed_discriminator(&[]), None);
	}

	#[test]
	fn release_comparison_counts_differing_slots_and_length_changes() {
		let same = ReleaseComparison::of(&[1; 16], &[1; 16]);
		assert!(same.identical_text);
		assert_eq!(same.differing_instructions, 0);

		let mut changed = vec![1_u8; 24];
		changed[9] = 2;
		let different = ReleaseComparison::of(&changed, &[1; 16]);
		assert!(!different.identical_text);
		assert_eq!(different.traced_instructions, 3);
		assert_eq!(different.release_instructions, 2);
		assert_eq!(different.differing_instructions, 2);
	}

	#[test]
	fn assign_names_labels_unmatched_and_repeated_profiles() {
		let profile = |id: &str, value: Option<u64>| {
			InstructionProfile {
				name: String::new(),
				instruction: None,
				discriminator: value.map(|value| ObservedDiscriminator { width: 1, value }),
				trace_ids: vec![id.to_owned()],
				executed_instructions: 0,
				syscall_invocations: 0,
				unattributed_instructions: 0,
				lines: Vec::new(),
				functions: Vec::new(),
				syscalls: Vec::new(),
				stacks: Vec::new(),
			}
		};
		let mut report = TraceReport {
			schema_version: TRACE_SCHEMA_VERSION,
			program: "demo".to_owned(),
			executable_sha256: String::new(),
			line_info: false,
			recorded_traces: 3,
			skipped_traces: 0,
			release_build: None,
			instructions: vec![
				profile("a", Some(1)),
				profile("b", Some(1)),
				profile("c", None),
			],
		};

		report.assign_names(|discriminator| {
			(discriminator.value == 1).then(|| "increment".to_owned())
		});
		let names: Vec<&str> = report
			.instructions
			.iter()
			.map(|p| p.name.as_str())
			.collect();

		assert_eq!(names, ["increment", "increment #2", "trace c"]);
		assert_eq!(
			report.instructions[1].instruction.as_deref(),
			Some("increment")
		);
	}

	#[test]
	fn sha256_hex_is_lowercase() {
		assert_eq!(
			sha256_hex(b"abc"),
			"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
		);
	}
}
