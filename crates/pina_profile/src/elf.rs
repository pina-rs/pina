//! ELF parsing for SBF binaries.
//!
//! Extracts symbol tables, `.text` section boundaries, and program metadata
//! from compiled Solana program `.so` files using the `object` crate.
//!
//! ## Symbol sources
//!
//! `cargo build-sbf --sbf-out-dir <dir>` publishes a stripped copy of the
//! program whose only symbols are the exported `.dynsym` entries (usually just
//! `entrypoint`). The linker output it was copied from keeps the full
//! `.symtab` under `<target>/<triple>/release/`. [`parse_elf`] prefers
//! `.symtab`, falls back to `.dynsym`, and [`unstripped_symbols`] recovers the
//! full table for a stripped artifact from that intermediate when its `.text`
//! is byte-identical.

use std::path::Path;

use object::Architecture;
use object::BinaryFormat;
use object::Object;
use object::ObjectSection;
use object::ObjectSegment;
use object::ObjectSymbol;
use object::SymbolKind;

use crate::ProfileError;

/// Directories, relative to the Cargo target directory, where `cargo build-sbf`
/// leaves the unstripped linker output of a release build.
///
/// Current platform tools use the `sbpf-solana-solana` triple; older releases
/// used `sbf-solana-solana`.
pub const UNSTRIPPED_ARTIFACT_DIRS: &[&str] =
	&["sbpf-solana-solana/release", "sbf-solana-solana/release"];

/// A resolved symbol from the ELF symbol table.
#[derive(Debug, Clone)]
pub struct Symbol {
	/// Demangled symbol name, without the legacy mangling hash suffix.
	pub name: String,
	/// Virtual address of the symbol.
	pub address: u64,
	/// Size of the symbol in bytes (0 if unknown).
	pub size: u64,
}

/// The ELF table that supplied [`ElfInfo::symbols`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTable {
	/// The full `.symtab` of an unstripped build.
	Full,
	/// Only the exported `.dynsym` entries survived stripping, or the binary
	/// carries no function symbols at all.
	Dynamic,
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
	/// Function symbols inside `.text`, sorted by address.
	pub symbols: Vec<Symbol>,
	/// Which table supplied `symbols`.
	pub symbol_table: SymbolTable,
}

/// Parse an ELF binary and extract profiling-relevant information.
///
/// # Errors
///
/// Returns [`ProfileError::Elf`] when the data is not an SBF/BPF ELF and
/// [`ProfileError::NoTextSection`] when it has no `.text` section.
pub fn parse_elf(data: &[u8], path: &Path) -> Result<ElfInfo, ProfileError> {
	// Parse the ELF file
	let obj = object::File::parse(data).map_err(|e| {
		ProfileError::Elf {
			path: path.to_path_buf(),
			message: e.to_string(),
		}
	})?;

	// Validate this is an ELF targeting SBF/BPF.
	if obj.format() != BinaryFormat::Elf {
		return Err(ProfileError::Elf {
			path: path.to_path_buf(),
			message: format!("expected ELF format, got {:?}", obj.format()),
		});
	}

	match obj.architecture() {
		Architecture::Bpf | Architecture::Sbf => {}
		arch => {
			return Err(ProfileError::Elf {
				path: path.to_path_buf(),
				message: format!("expected SBF/BPF architecture, got {arch:?}"),
			});
		}
	}

	// Find the `.text` section. sbpf-linker emits static-syscall programs as
	// program-header-only ELFs with no section table at all, so fall back to
	// the executable `PT_LOAD` segment when there are no sections to consult.
	let (text_vaddr, text_bytes) = if let Some(text_section) = obj.section_by_name(".text") {
		let vaddr = text_section.address();
		let bytes = text_section
			.data()
			.map_err(|e| {
				ProfileError::Elf {
					path: path.to_path_buf(),
					message: format!("Failed to read .text section data: {e}"),
				}
			})?
			.to_vec();
		(vaddr, bytes)
	} else if obj.sections().next().is_none() {
		executable_text_segment(&obj, path)?
	} else {
		return Err(ProfileError::NoTextSection {
			path: path.to_path_buf(),
		});
	};
	let text_size = text_bytes.len() as u64;
	let text_range = text_vaddr..text_vaddr.saturating_add(text_size);

	// Stripped artifacts keep only `.dynsym`, so fall back to it rather than
	// reporting the whole program as one anonymous function.
	let full_symbols = text_symbols(obj.symbols(), &text_range);
	let (symbols, symbol_table) = if full_symbols.is_empty() {
		(
			text_symbols(obj.dynamic_symbols(), &text_range),
			SymbolTable::Dynamic,
		)
	} else {
		(full_symbols, SymbolTable::Full)
	};

	// Extract program name from path
	let program_name = path
		.file_stem()
		.and_then(|s| s.to_str())
		.unwrap_or("unknown")
		.to_owned();

	Ok(ElfInfo {
		program_name,
		text_bytes,
		text_vaddr,
		text_size,
		symbols,
		symbol_table,
	})
}

/// Collect named function symbols inside `.text`, sorted by address.
///
/// Zero-sized symbols are kept so downstream span inference can run.
fn text_symbols<'data, S: ObjectSymbol<'data>>(
	symbols: impl Iterator<Item = S>,
	text_range: &std::ops::Range<u64>,
) -> Vec<Symbol> {
	let mut symbols: Vec<Symbol> = symbols
		.filter(|sym| sym.kind() == SymbolKind::Text)
		.filter_map(|sym| {
			let name = sym.name().ok()?;
			let address = sym.address();

			if name.is_empty() || !text_range.contains(&address) {
				return None;
			}

			Some(Symbol {
				name: demangle(name),
				address,
				size: sym.size(),
			})
		})
		.collect();

	symbols.sort_by_key(|s| s.address);
	symbols
}

/// Demangle a Rust symbol name, dropping the legacy `::h<hash>` suffix.
///
/// The hash changes whenever any crate metadata changes, so keeping it would
/// make every function look renamed between two builds. Names that are not
/// Rust symbols are returned unchanged.
#[must_use]
pub fn demangle(name: &str) -> String {
	rustc_demangle::try_demangle(name)
		.map_or_else(|_| name.to_owned(), |symbol| format!("{symbol:#}"))
}

/// Recover the full symbol table for a stripped artifact from the unstripped
/// `cargo build-sbf` intermediate.
///
/// `path` is the stripped artifact, conventionally `<target>/deploy/<lib>.so`.
/// Each directory in [`UNSTRIPPED_ARTIFACT_DIRS`] below the artifact's
/// grandparent is checked for a file with the same name. Its symbols are
/// returned only when it has a full `.symtab` and a `.text` section that is
/// byte-identical at the same address, so a stale or unrelated build can never
/// mislabel functions. Unreadable candidates are skipped: they only mean the
/// richer symbols are unavailable.
#[must_use]
pub fn unstripped_symbols(path: &Path, stripped: &ElfInfo) -> Option<Vec<Symbol>> {
	let file_name = path.file_name()?;
	let target_dir = path.parent()?.parent()?;

	UNSTRIPPED_ARTIFACT_DIRS.iter().find_map(|directory| {
		let candidate = target_dir.join(directory).join(file_name);
		let data = std::fs::read(&candidate).ok()?;
		let info = parse_elf(&data, &candidate).ok()?;
		let identical_text =
			info.text_vaddr == stripped.text_vaddr && info.text_bytes == stripped.text_bytes;

		(info.symbol_table == SymbolTable::Full && identical_text).then_some(info.symbols)
	})
}

/// Locate the instruction stream of a section-less ELF.
///
/// These are the sbpf-linker static-syscall artifacts: the whole program
/// lives in its program headers, and the executable `PT_LOAD` segment
/// carries the SBF instructions, so profiling reads the segment instead of
/// a `.text` section.
///
/// # Errors
///
/// Returns [`ProfileError::Elf`] when the executable segment cannot be read,
/// and [`ProfileError::NoTextSection`] when the ELF has no executable
/// segment.
fn executable_text_segment<'data>(
	obj: &object::File<'data, &'data [u8]>,
	path: &Path,
) -> Result<(u64, Vec<u8>), ProfileError> {
	for segment in obj.segments() {
		let executable = match segment.flags() {
			object::SegmentFlags::Elf { p_flags, .. } => p_flags.contains(object::elf::PF_X),
			_ => false,
		};
		if !executable {
			continue;
		}
		let bytes = segment.data().map_err(|e| {
			ProfileError::Elf {
				path: path.to_path_buf(),
				message: format!("Failed to read executable segment data: {e}"),
			}
		})?;
		return Ok((segment.address(), bytes.to_vec()));
	}

	Err(ProfileError::NoTextSection {
		path: path.to_path_buf(),
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
	fn parse_elf_rejects_a_non_elf_binary() {
		use object::write::Object;

		let mut writer = Object::new(
			object::BinaryFormat::MachO,
			object::Architecture::X86_64,
			object::Endianness::Little,
		);
		let section = writer.add_section(Vec::new(), b".text".to_vec(), object::SectionKind::Text);
		writer.set_section_data(section, vec![0u8; 8], 8);
		let bytes = writer.write().expect("write Mach-O binary");
		let result = parse_elf(&bytes, Path::new("macho.so"));
		let error = result.expect_err("a Mach-O binary should fail the format check");
		assert!(
			error.to_string().contains("expected ELF format"),
			"unexpected error: {error}"
		);
	}

	#[test]
	fn parse_elf_rejects_an_elf_with_the_wrong_architecture() {
		use object::write::Object;

		let mut writer = Object::new(
			object::BinaryFormat::Elf,
			object::Architecture::X86_64,
			object::Endianness::Little,
		);
		let section = writer.add_section(Vec::new(), b".text".to_vec(), object::SectionKind::Text);
		writer.set_section_data(section, vec![0u8; 8], 8);
		let bytes = writer.write().expect("write x86-64 ELF");
		let result = parse_elf(&bytes, Path::new("x86_64.so"));
		let error = result.expect_err("an x86-64 ELF should fail the architecture check");
		assert!(
			error.to_string().contains("expected SBF/BPF architecture"),
			"unexpected error: {error}"
		);
	}

	#[test]
	fn text_symbols_skips_empty_and_out_of_range_names() {
		use object::read::ObjectSymbol;
		use object::write::Object;

		let mut writer = Object::new(
			object::BinaryFormat::Elf,
			object::Architecture::Sbf,
			object::Endianness::Little,
		);
		let section = writer.add_section(Vec::new(), b".text".to_vec(), object::SectionKind::Text);
		writer.set_section_data(section, vec![0u8; 64], 8);
		for &(name, offset, size) in &[
			("", 0u64, 8u64),
			("in_range", 8, 8),
			("far_away", 0x100_0000, 8),
		] {
			writer.add_symbol(object::write::Symbol {
				name: name.as_bytes().to_vec(),
				value: offset,
				size,
				kind: object::SymbolKind::Text,
				scope: object::SymbolScope::Dynamic,
				weak: false,
				section: object::write::SymbolSection::Section(section),
				flags: object::SymbolFlags::None,
			});
		}
		let bytes = writer.write().expect("write ELF with edge-case symbols");
		let obj = object::File::parse(bytes.as_slice()).expect("parse ELF");
		let symbols = text_symbols(obj.symbols(), &(0..64));
		assert_eq!(
			symbols.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
			vec!["in_range"],
			"empty names and out-of-range addresses should be filtered"
		);
	}

	/// Build a minimal section-less little-endian SBPF ELF whose single
	/// `PT_LOAD` covers `text`, mirroring the sbpf-linker static-syscall
	/// artifact layout: program headers only, no section table.
	fn phdr_only_elf(p_flags: u32, text: &[u8]) -> Vec<u8> {
		let mut elf = Vec::new();
		// ELF64 header: ident, ET_EXEC, EM_BPF, EV_CURRENT.
		elf.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
		elf.extend_from_slice(&2u16.to_le_bytes());
		elf.extend_from_slice(&247u16.to_le_bytes());
		elf.extend_from_slice(&1u32.to_le_bytes());
		let entry = 0x1000u64;
		elf.extend_from_slice(&entry.to_le_bytes());
		elf.extend_from_slice(&64u64.to_le_bytes()); // e_phoff
		elf.extend_from_slice(&0u64.to_le_bytes()); // e_shoff
		elf.extend_from_slice(&0u32.to_le_bytes()); // e_flags
		elf.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize
		elf.extend_from_slice(&56u16.to_le_bytes()); // e_phentsize
		elf.extend_from_slice(&1u16.to_le_bytes()); // e_phnum
		elf.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize
		elf.extend_from_slice(&0u16.to_le_bytes()); // e_shnum
		elf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx
		assert_eq!(elf.len(), 64);
		// Program header: one PT_LOAD for the text bytes.
		elf.extend_from_slice(&1u32.to_le_bytes()); // PT_LOAD
		elf.extend_from_slice(&p_flags.to_le_bytes());
		elf.extend_from_slice(&(64u64 + 56).to_le_bytes()); // p_offset
		elf.extend_from_slice(&entry.to_le_bytes()); // p_vaddr
		elf.extend_from_slice(&entry.to_le_bytes()); // p_paddr
		elf.extend_from_slice(&(text.len() as u64).to_le_bytes());
		elf.extend_from_slice(&(text.len() as u64).to_le_bytes());
		elf.extend_from_slice(&8u64.to_le_bytes());
		elf.extend_from_slice(text);
		elf
	}

	#[test]
	fn parse_elf_reads_phdr_only_static_syscall_artifacts() {
		let text = [0xBFu8, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
		let elf = phdr_only_elf(1, &text); // PF_X
		let info = parse_elf(&elf, Path::new("static.so")).expect("parse PHDR-only ELF");
		assert_eq!(info.text_vaddr, 0x1000);
		assert_eq!(info.text_size, text.len() as u64);
		assert_eq!(info.text_bytes, text);
		assert!(info.symbols.is_empty());
	}

	#[test]
	fn parse_elf_rejects_phdr_only_without_executable_segment() {
		let elf = phdr_only_elf(4, &[0u8; 8]); // PF_R only
		let result = parse_elf(&elf, Path::new("readonly.so"));
		assert!(matches!(result, Err(ProfileError::NoTextSection { .. })));
	}

	#[test]
	fn parse_elf_still_rejects_sectioned_elves_without_text() {
		use object::Endianness;
		use object::SectionKind;
		use object::write::Object;

		let mut writer = Object::new(BinaryFormat::Elf, Architecture::Bpf, Endianness::Little);
		let section =
			writer.add_section(Vec::new(), b".rodata".to_vec(), SectionKind::ReadOnlyData);
		writer.set_section_data(section, vec![0u8; 8], 8);
		let bytes = writer.write().expect("write sectioned ELF");
		let result = parse_elf(&bytes, Path::new("rodata-only.so"));
		assert!(matches!(result, Err(ProfileError::NoTextSection { .. })));
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

	#[test]
	fn demangle_drops_the_legacy_hash_and_keeps_plain_names() {
		assert_eq!(
			demangle("_ZN4pina5impls15address_matches17hb8b7b38d9778b12dE"),
			"pina::impls::address_matches"
		);
		assert_eq!(
			demangle("_RNvCscR3e5J6VSLN_7___rustc17rust_begin_unwind"),
			"__rustc::rust_begin_unwind"
		);
		assert_eq!(demangle("entrypoint"), "entrypoint");
	}

	#[test]
	fn unstripped_symbols_needs_a_target_directory_above_the_artifact() {
		let stripped = ElfInfo {
			program_name: "demo".to_owned(),
			text_bytes: Vec::new(),
			text_vaddr: 0,
			text_size: 0,
			symbols: Vec::new(),
			symbol_table: SymbolTable::Dynamic,
		};

		assert!(unstripped_symbols(Path::new("demo.so"), &stripped).is_none());
		assert!(unstripped_symbols(Path::new("/"), &stripped).is_none());
	}
}
