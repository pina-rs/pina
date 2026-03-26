//! SBF instruction stream analysis.
//!
//! Walks the `.text` section byte-by-byte (in 8-byte SBF instruction units)
//! and counts instructions per function symbol.

use crate::cost::CU_PER_INSTRUCTION;
use crate::cost::FunctionProfile;
use crate::elf::ElfInfo;

/// Size of a single SBF instruction in bytes.
pub const SBF_INSTRUCTION_SIZE: u64 = 8;

/// Analyze the `.text` section and produce per-function profiles.
///
/// Uses the symbol table to attribute instructions to named functions.
/// Instructions not covered by any symbol are grouped under
/// `<unknown+offset>`.
pub fn analyze_functions(elf: &ElfInfo) -> Vec<FunctionProfile> {
	let text_len = elf.text_bytes.len() as u64;

	if text_len == 0 {
		return vec![];
	}

	let total_instructions = text_len / SBF_INSTRUCTION_SIZE;

	if elf.symbols.is_empty() {
		// No symbols — report the entire .text section as one block.
		return vec![FunctionProfile {
			name: "<entire .text>".to_owned(),
			offset: 0,
			size: text_len,
			instruction_count: total_instructions,
			estimated_cu: total_instructions * CU_PER_INSTRUCTION,
		}];
	}

	let mut functions = Vec::new();

	// Track coverage to find gaps.
	let mut covered_up_to: u64 = 0;

	for (i, sym) in elf.symbols.iter().enumerate() {
		// Symbol addresses are virtual; convert to .text-relative offset.
		let sym_offset = sym.address.saturating_sub(elf.text_vaddr);

		// If there's a gap before this symbol, create an unknown entry.
		if sym_offset > covered_up_to {
			let gap_size = sym_offset - covered_up_to;
			let gap_instructions = gap_size / SBF_INSTRUCTION_SIZE;
			if gap_instructions > 0 {
				functions.push(FunctionProfile {
					name: format!("<unknown+0x{covered_up_to:x}>"),
					offset: covered_up_to,
					size: gap_size,
					instruction_count: gap_instructions,
					estimated_cu: gap_instructions * CU_PER_INSTRUCTION,
				});
			}
		}

		// Determine the function's size.
		let func_size = if sym.size > 0 {
			sym.size
		} else {
			// Estimate from next symbol or end of .text.
			let next_offset = elf
				.symbols
				.get(i + 1)
				.map_or(text_len, |s| s.address.saturating_sub(elf.text_vaddr));
			next_offset.saturating_sub(sym_offset)
		};

		// Clamp to .text bounds.
		let func_size = func_size.min(text_len.saturating_sub(sym_offset));
		let instruction_count = func_size / SBF_INSTRUCTION_SIZE;

		functions.push(FunctionProfile {
			name: sym.name.clone(),
			offset: sym_offset,
			size: func_size,
			instruction_count,
			estimated_cu: instruction_count * CU_PER_INSTRUCTION,
		});

		covered_up_to = sym_offset + func_size;
	}

	// Trailing bytes after the last symbol.
	if covered_up_to < text_len {
		let trailing_size = text_len - covered_up_to;
		let trailing_instructions = trailing_size / SBF_INSTRUCTION_SIZE;
		if trailing_instructions > 0 {
			functions.push(FunctionProfile {
				name: format!("<unknown+0x{covered_up_to:x}>"),
				offset: covered_up_to,
				size: trailing_size,
				instruction_count: trailing_instructions,
				estimated_cu: trailing_instructions * CU_PER_INSTRUCTION,
			});
		}
	}

	// Sort by CU descending for the output.
	functions.sort_by_key(|f| std::cmp::Reverse(f.estimated_cu));

	functions
}

#[cfg(test)]
mod tests {
	use crate::elf::ElfInfo;
	use crate::elf::Symbol;

	use super::*;

	fn make_elf(text_size: u64, symbols: Vec<Symbol>) -> ElfInfo {
		ElfInfo {
			program_name: "test".to_owned(),
			text_bytes: vec![0u8; text_size as usize],
			text_vaddr: 0x1000,
			text_size,
			symbols,
		}
	}

	#[test]
	fn empty_text_section() {
		let elf = make_elf(0, vec![]);
		let result = analyze_functions(&elf);
		assert!(result.is_empty());
	}

	#[test]
	fn no_symbols_reports_entire_text() {
		let elf = make_elf(80, vec![]);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].name, "<entire .text>");
		assert_eq!(result[0].instruction_count, 10); // 80 / 8
		assert_eq!(result[0].estimated_cu, 10);
	}

	#[test]
	fn single_symbol_covers_entire_text() {
		let elf = make_elf(
			80,
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
			160,
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
		// Sorted by CU descending — both equal, so order may vary.
		let names: Vec<&str> = result.iter().map(|f| f.name.as_str()).collect();
		assert!(names.contains(&"func_a"));
		assert!(names.contains(&"func_b"));
		assert_eq!(result[0].instruction_count, 10);
		assert_eq!(result[1].instruction_count, 10);
	}

	#[test]
	fn gap_between_symbols_reported_as_unknown() {
		let elf = make_elf(
			240,
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 80,
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x10A0, // gap of 80 bytes (0x50 to 0xA0)
					size: 80,
				},
			],
		);
		let result = analyze_functions(&elf);
		// func_a (80B), unknown gap (80B), func_b (80B)
		assert_eq!(result.len(), 3);
		let unknown = result.iter().find(|f| f.name.starts_with("<unknown"));
		assert!(unknown.is_some());
		assert_eq!(unknown.unwrap().instruction_count, 10);
	}

	#[test]
	fn zero_size_symbol_inferred_from_next() {
		let elf = make_elf(
			160,
			vec![
				Symbol {
					name: "func_a".to_owned(),
					address: 0x1000,
					size: 0, // unknown size — infer from next
				},
				Symbol {
					name: "func_b".to_owned(),
					address: 0x1050,
					size: 0, // unknown size — infer from end
				},
			],
		);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 2);
		for func in &result {
			assert_eq!(func.instruction_count, 10);
		}
	}

	#[test]
	fn trailing_bytes_after_last_symbol() {
		let elf = make_elf(
			160,
			vec![Symbol {
				name: "func_a".to_owned(),
				address: 0x1000,
				size: 80,
			}],
		);
		let result = analyze_functions(&elf);
		// func_a (80B) + trailing unknown (80B)
		assert_eq!(result.len(), 2);
		let trailing = result.iter().find(|f| f.name.starts_with("<unknown"));
		assert!(trailing.is_some());
	}

	#[test]
	fn results_sorted_by_cu_descending() {
		let elf = make_elf(
			240,
			vec![
				Symbol {
					name: "small".to_owned(),
					address: 0x1000,
					size: 16,
				},
				Symbol {
					name: "large".to_owned(),
					address: 0x1010,
					size: 160,
				},
			],
		);
		let result = analyze_functions(&elf);
		assert!(result[0].estimated_cu >= result[1].estimated_cu);
		// Largest function should be first.
		assert_eq!(result[0].name, "large");
	}

	#[test]
	fn non_aligned_text_ignores_partial_instruction() {
		// 13 bytes = 1 full instruction (8B) + 5 leftover bytes
		let elf = make_elf(13, vec![]);
		let result = analyze_functions(&elf);
		assert_eq!(result.len(), 1);
		assert_eq!(result[0].instruction_count, 1); // 13 / 8 = 1
	}

	#[test]
	fn cu_calculation_uses_cost_constant() {
		let elf = make_elf(
			800,
			vec![Symbol {
				name: "big_fn".to_owned(),
				address: 0x1000,
				size: 800,
			}],
		);
		let result = analyze_functions(&elf);
		assert_eq!(result[0].instruction_count, 100);
		assert_eq!(result[0].estimated_cu, 100 * CU_PER_INSTRUCTION);
	}
}
