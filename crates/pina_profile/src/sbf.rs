//! SBF instruction stream analysis.
//!
//! Walks the `.text` section in 8-byte SBF instruction units, decodes each
//! opcode, and estimates per-function compute unit costs using the cost model
//! from [`crate::cost`].

use std::cmp::Reverse;

use crate::cost::BPF_CALL_IMM;
use crate::cost::FunctionProfile;
use crate::cost::estimate_instruction_cu;
use crate::elf::ElfInfo;

/// Size of a single SBF instruction in bytes.
pub const SBF_INSTRUCTION_SIZE: u64 = 8;

/// Estimate CU and count syscalls for a byte range within the `.text` section.
///
/// Returns `(instruction_count, syscall_count, estimated_cu)`.
fn estimate_range(text_bytes: &[u8], start: u64, size: u64) -> (u64, u64, u64) {
	let start = start as usize;
	let size = size as usize;
	let end = (start + size).min(text_bytes.len());

	let mut instruction_count: u64 = 0;
	let mut syscall_count: u64 = 0;
	let mut estimated_cu: u64 = 0;

	let mut offset = start;
	while offset + 8 <= end {
		let bytes: &[u8; 8] = text_bytes[offset..offset + 8]
			.try_into()
			.unwrap_or_else(|_| panic!("slice length mismatch at offset {offset}"));

		let cu = estimate_instruction_cu(bytes);
		instruction_count += 1;
		estimated_cu += cu;

		if bytes[0] == BPF_CALL_IMM && (bytes[1] >> 4) == 0 {
			syscall_count += 1;
		}

		offset += 8;
	}

	(instruction_count, syscall_count, estimated_cu)
}

/// Analyze the `.text` section and produce per-function profiles.
///
/// Uses the symbol table to attribute instructions to named functions.
/// Instructions not covered by any symbol are grouped under
/// `<unknown+offset>`. Each instruction is decoded to estimate CU costs
/// using the opcode-aware cost model.
pub fn analyze_functions(elf: &ElfInfo) -> Vec<FunctionProfile> {
	let text_len = elf.text_bytes.len() as u64;

	if text_len == 0 {
		return vec![];
	}

	if elf.symbols.is_empty() {
		let (instruction_count, syscall_count, estimated_cu) =
			estimate_range(&elf.text_bytes, 0, text_len);
		return vec![FunctionProfile {
			name: "<entire .text>".to_owned(),
			offset: 0,
			size: text_len,
			instruction_count,
			syscall_count,
			estimated_cu,
		}];
	}

	let mut functions = Vec::new();
	let mut covered_up_to: u64 = 0;

	for (i, sym) in elf.symbols.iter().enumerate() {
		let sym_offset = sym.address.saturating_sub(elf.text_vaddr);

		// Gap before this symbol → unknown region.
		if sym_offset > covered_up_to {
			let gap_size = sym_offset - covered_up_to;
			let (ic, sc, cu) = estimate_range(&elf.text_bytes, covered_up_to, gap_size);
			if ic > 0 {
				functions.push(FunctionProfile {
					name: format!("<unknown+0x{covered_up_to:x}>"),
					offset: covered_up_to,
					size: gap_size,
					instruction_count: ic,
					syscall_count: sc,
					estimated_cu: cu,
				});
			}
		}

		// Determine function size.
		let func_size = if sym.size > 0 {
			sym.size
		} else {
			let next_offset = elf
				.symbols
				.get(i + 1)
				.map_or(text_len, |s| s.address.saturating_sub(elf.text_vaddr));
			next_offset.saturating_sub(sym_offset)
		};

		// Clamp to uncovered region to avoid double-counting.
		let func_end = sym_offset.saturating_add(func_size).min(text_len);
		let func_start = sym_offset.max(covered_up_to);
		if func_start >= func_end {
			continue;
		}
		let clamped_size = func_end - func_start;

		let (ic, sc, cu) = estimate_range(&elf.text_bytes, func_start, clamped_size);

		functions.push(FunctionProfile {
			name: sym.name.clone(),
			offset: func_start,
			size: clamped_size,
			instruction_count: ic,
			syscall_count: sc,
			estimated_cu: cu,
		});

		covered_up_to = func_end;
	}

	// Trailing bytes after last symbol.
	if covered_up_to < text_len {
		let trailing_size = text_len - covered_up_to;
		let (ic, sc, cu) = estimate_range(&elf.text_bytes, covered_up_to, trailing_size);
		if ic > 0 {
			functions.push(FunctionProfile {
				name: format!("<unknown+0x{covered_up_to:x}>"),
				offset: covered_up_to,
				size: trailing_size,
				instruction_count: ic,
				syscall_count: sc,
				estimated_cu: cu,
			});
		}
	}

	functions.sort_by_key(|f| Reverse(f.estimated_cu));

	functions
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::cost::CU_PER_INSTRUCTION;
	use crate::cost::CU_PER_SYSCALL;
	use crate::elf::ElfInfo;
	use crate::elf::Symbol;

	fn make_elf(text_bytes: Vec<u8>, symbols: Vec<Symbol>) -> ElfInfo {
		let text_size = text_bytes.len() as u64;
		ElfInfo {
			program_name: "test".to_owned(),
			text_bytes,
			text_vaddr: 0x1000,
			text_size,
			symbols,
		}
	}

	/// Build a .text section with `n` NOP-like instructions (opcode 0x07 =
	/// ADD64 imm).
	fn nop_text(n: usize) -> Vec<u8> {
		let mut bytes = Vec::with_capacity(n * 8);
		for _ in 0..n {
			bytes.extend_from_slice(&[0x07, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
		}
		bytes
	}

	/// Build a .text section with `n_regular` regular instructions followed by
	/// `n_syscalls` syscall instructions.
	fn mixed_text(n_regular: usize, n_syscalls: usize) -> Vec<u8> {
		let mut bytes = Vec::with_capacity((n_regular + n_syscalls) * 8);
		for _ in 0..n_regular {
			bytes.extend_from_slice(&[0x07, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
		}
		for _ in 0..n_syscalls {
			// syscall: opcode 0x85, src_reg=0
			bytes.extend_from_slice(&[0x85, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
		}
		bytes
	}

	#[test]
	fn empty_text_section() {
		let elf = make_elf(vec![], vec![]);
		let result = analyze_functions(&elf);
		assert!(result.is_empty());
	}

	#[test]
	fn no_symbols_reports_entire_text() {
		let elf = make_elf(nop_text(10), vec![]);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].name, "<entire .text>");
		assert_eq!(result[0].instruction_count, 10);
		assert_eq!(result[0].estimated_cu, 10);
		assert_eq!(result[0].syscall_count, 0);
	}

	#[test]
	fn syscalls_increase_cu_estimate() {
		// 5 regular + 3 syscalls
		let elf = make_elf(mixed_text(5, 3), vec![]);
		let result = analyze_functions(&elf);
		assert_eq!(result[0].instruction_count, 8);
		assert_eq!(result[0].syscall_count, 3);
		assert_eq!(
			result[0].estimated_cu,
			5 * CU_PER_INSTRUCTION + 3 * CU_PER_SYSCALL
		);
	}

	#[test]
	fn single_symbol_covers_entire_text() {
		let elf = make_elf(
			nop_text(10),
			vec![Symbol {
				name: "entrypoint".to_owned(),
				address: 0x1000,
				size: 80,
			}],
		);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].name, "entrypoint");
		assert_eq!(result[0].instruction_count, 10);
	}

	#[test]
	fn multiple_symbols_partitioned() {
		let elf = make_elf(
			nop_text(20),
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 80,
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x1050,
					size: 80,
				},
			],
		);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 2);
		let names: Vec<&str> = result.iter().map(|f| f.name.as_str()).collect();
		assert!(names.contains(&"func_a"));
		assert!(names.contains(&"func_b"));
	}

	#[test]
	fn gap_between_symbols_reported_as_unknown() {
		let elf = make_elf(
			nop_text(30),
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 80,
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x10A0,
					size: 80,
				},
			],
		);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 3);
		let unknown = result.iter().find(|f| f.name.starts_with("<unknown"));
		assert!(unknown.is_some());
	}

	#[test]
	fn zero_size_symbol_inferred_from_next() {
		let elf = make_elf(
			nop_text(20),
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 0,
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x1050,
					size: 0,
				},
			],
		);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn trailing_bytes_after_last_symbol() {
		let elf = make_elf(
			nop_text(20),
			vec![Symbol {
				name: "func_a".to_owned(),
				address: 0x1000,
				size: 80,
			}],
		);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 2);
		let trailing = result.iter().find(|f| f.name.starts_with("<unknown"));
		assert!(trailing.is_some());
	}

	#[test]
	fn results_sorted_by_cu_descending() {
		// func with syscalls should have higher CU than func without
		let mut text = nop_text(5); // func_small: 5 regular
		text.extend_from_slice(&mixed_text(2, 3).as_slice()); // func_large: 2 regular + 3 syscalls
		let elf = make_elf(
			text,
			vec![
				Symbol {
					name: "small".to_owned(),
					address: 0x1000,
					size: 40, // 5 instructions
				},
				Symbol {
					name: "large".to_owned(),
					address: 0x1028,
					size: 40, // 5 instructions but with syscalls
				},
			],
		);
		let result = analyze_functions(&elf);
		assert!(result[0].estimated_cu >= result[1].estimated_cu);
		assert_eq!(result[0].name, "large");
	}

	#[test]
	fn non_aligned_text_ignores_partial_instruction() {
		// 13 bytes = 1 full instruction + 5 leftover
		let mut text = vec![0x07, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
		text.extend_from_slice(&[0x00; 5]);
		let elf = make_elf(text, vec![]);
		let result = analyze_functions(&elf);
		assert_eq!(result[0].instruction_count, 1);
	}

	#[test]
	fn overlapping_symbols_not_double_counted() {
		let elf = make_elf(
			nop_text(15),
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 80,
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x1028,
					size: 80,
				},
			],
		);
		let result = analyze_functions(&elf);
		let total: u64 = result.iter().map(|f| f.instruction_count).sum();
		assert_eq!(total, 15);
	}

	#[test]
	fn nested_symbol_skipped() {
		let elf = make_elf(
			nop_text(20),
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 160,
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x1010,
					size: 16,
				},
			],
		);
		let result = analyze_functions(&elf);
		let total: u64 = result.iter().map(|f| f.instruction_count).sum();
		assert_eq!(total, 20);
	}

	#[test]
	fn per_function_syscall_attribution() {
		// func_a: 5 regular, func_b: 2 regular + 3 syscalls
		let mut text = nop_text(5);
		text.extend(mixed_text(2, 3));
		let elf = make_elf(
			text,
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 40,
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x1028,
					size: 40,
				},
			],
		);
		let result = analyze_functions(&elf);
		let func_a = result.iter().find(|f| f.name == "func_a").unwrap();
		let func_b = result.iter().find(|f| f.name == "func_b").unwrap();

		assert_eq!(func_a.syscall_count, 0);
		assert_eq!(func_a.estimated_cu, 5);

		assert_eq!(func_b.syscall_count, 3);
		assert_eq!(func_b.estimated_cu, 2 + 3 * CU_PER_SYSCALL);
	}
}
