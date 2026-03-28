//! ELF parsing for SBF binaries.
//!
//! Extracts symbol tables, `.text` section boundaries, and program metadata
//! from compiled Solana program `.so` files using the `object` crate.

use std::path::Path;

use object::Object;
use object::ObjectSection;
use object::ObjectSymbol;
use object::SymbolKind;

use crate::ProfileError;

/// A resolved symbol from the ELF symbol table.
#[derive(Debug, Clone)]
pub struct Symbol {
	/// Symbol name.
	pub name: String,
	/// Virtual address of the symbol.
	pub address: u64,
	/// Size of the symbol in bytes (0 if unknown).
	pub size: u64,
}

/// Parsed ELF information relevant to profiling.
#[derive(Debug)]
pub struct ElfInfo {
	/// Program name (derived from filename).
	pub program_name: String,
	/// Raw bytes of the `.text` section.
	pub text_bytes: Vec<u8>,
	/// Virtual address of the `.text` section start.
	pub text_vaddr: u64,
	/// Size of the `.text` section in bytes.
	pub text_size: u64,
	/// Symbols from the ELF symbol table, sorted by address.
	pub symbols: Vec<Symbol>,
}

/// Parse an ELF binary and extract profiling-relevant information.
pub fn parse_elf(data: &[u8], path: &Path) -> Result<ElfInfo, ProfileError> {
	let obj = object::File::parse(data).map_err(|e| {
		ProfileError::Elf {
			path: path.to_path_buf(),
			message: e.to_string(),
		}
	})?;

	// Find the .text section.
	let text_section = obj.section_by_name(".text").ok_or_else(|| {
		ProfileError::NoTextSection {
			path: path.to_path_buf(),
		}
	})?;

	let text_vaddr = text_section.address();
	let text_bytes = text_section.data().map_err(|e| {
		ProfileError::Elf {
			path: path.to_path_buf(),
			message: format!("Failed to read .text section data: {e}"),
		}
	})?;
	let text_size = text_bytes.len() as u64;

	// Extract function symbols sorted by address.
	let mut symbols: Vec<Symbol> = obj
		.symbols()
		.filter(|sym| sym.kind() == SymbolKind::Text && sym.size() > 0)
		.filter_map(|sym| {
			let name = sym.name().ok()?;
			if name.is_empty() {
				return None;
			}
			Some(Symbol {
				name: name.to_owned(),
				address: sym.address(),
				size: sym.size(),
			})
		})
		.collect();

	symbols.sort_by_key(|s| s.address);

	let program_name = path
		.file_stem()
		.and_then(|s| s.to_str())
		.unwrap_or("unknown")
		.to_owned();

	Ok(ElfInfo {
		program_name,
		text_bytes: text_bytes.to_vec(),
		text_vaddr,
		text_size,
		symbols,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_elf_rejects_empty_data() {
		let result = parse_elf(&[], Path::new("empty.so"));
		assert!(result.is_err());
	}

	#[test]
	fn parse_elf_rejects_garbage_data() {
		let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
		let result = parse_elf(&garbage, Path::new("garbage.so"));
		assert!(result.is_err());
	}

	#[test]
	fn symbol_sorting() {
		let mut symbols = vec![
			Symbol {
				name: "b".to_owned(),
				address: 200,
				size: 10,
			},
			Symbol {
				name: "a".to_owned(),
				address: 100,
				size: 20,
			},
		];
		symbols.sort_by_key(|s| s.address);
		assert_eq!(symbols[0].name, "a");
		assert_eq!(symbols[1].name, "b");
	}
}
