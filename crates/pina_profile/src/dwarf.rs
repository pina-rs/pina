//! Source attribution for SBF program counters.
//!
//! A [`Symbolizer`] maps a program counter to the stack of source frames that
//! produced the instruction at it:
//!
//! - The physical function comes from the ELF symbol table (demangled), or
//!   from the DWARF subprogram when no symbol covers the address.
//! - Inlined frames come from `DW_TAG_inlined_subroutine` entries, so code
//!   that LTO inlined into `entrypoint` is still attributed to the function it
//!   was written in. Their names are DWARF's short names (for example
//!   `assert_writable` or `get<u8>`), because a `line-tables-only` build
//!   records no linkage names for them.
//! - Each frame's location is the call site of the frame inside it; the
//!   innermost frame's location is the line-table row for the address.
//!
//! Paths are reported exactly as the compiler recorded them. Cargo records
//! workspace members relative to the workspace root and other packages
//! relative to their own package root.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

use gimli::AttributeValue;
use gimli::EndianSlice;
use gimli::LittleEndian;
use object::Object;
use object::ObjectSection;
use serde::Deserialize;
use serde::Serialize;

use crate::ProfileError;
use crate::elf;
use crate::sbf::SBF_INSTRUCTION_SIZE;

type Slice<'data> = EndianSlice<'data, LittleEndian>;
type Dwarf<'data> = gimli::Dwarf<Slice<'data>>;
type Unit<'data> = gimli::Unit<Slice<'data>>;
type Entry<'data> = gimli::DebuggingInformationEntry<Slice<'data>>;

/// Abstract-origin and specification links followed before giving up on a name.
const MAX_ORIGIN_DEPTH: u8 = 8;

/// A source file and line.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceLocation {
	/// The path as recorded in the debug information.
	pub file: String,
	/// The 1-based line number.
	pub line: u32,
}

/// One logical frame of a program counter's source stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
	/// The function the frame executes.
	pub function: String,
	/// Where in that function the frame is: the call site of the next frame
	/// in, or the instruction itself for the innermost frame. `None` when the
	/// compiler recorded no line (line 0) or the program has no line table.
	pub location: Option<SourceLocation>,
}

/// A subprogram or inlined subroutine and the address ranges it covers.
#[derive(Debug)]
struct Scope {
	ranges: Vec<Range<u64>>,
	name: Option<String>,
	/// Where an inlined subroutine was inlined into its parent.
	call_site: Option<SourceLocation>,
	children: Vec<Scope>,
}

impl Scope {
	fn contains(&self, address: u64) -> bool {
		self.ranges.iter().any(|range| range.contains(&address))
	}
}

/// One row of the line table. `line == 0` and end-of-sequence rows carry no
/// location.
#[derive(Debug, Clone, Copy)]
struct LineRow {
	address: u64,
	file: usize,
	line: u32,
	end_sequence: bool,
}

/// Maps SBF program counters to functions and source lines.
#[derive(Debug)]
pub struct Symbolizer {
	text_address: u64,
	symbols: Vec<elf::Symbol>,
	scopes: Vec<Scope>,
	/// `(range, index into scopes)` sorted by range start.
	scope_index: Vec<(Range<u64>, usize)>,
	lines: Vec<LineRow>,
	files: Vec<String>,
}

impl Symbolizer {
	/// Build a symbolizer from an SBF ELF, reading DWARF when present.
	///
	/// A program without debug sections still resolves functions from its
	/// symbol table; [`Self::has_line_info`] reports whether lines resolve.
	///
	/// # Errors
	///
	/// Returns [`ProfileError::Elf`] when the ELF cannot be parsed or its
	/// DWARF is malformed, and [`ProfileError::NoTextSection`] when it has no
	/// `.text` section.
	pub fn new(data: &[u8], path: &Path) -> Result<Self, ProfileError> {
		let info = elf::parse_elf(data, path)?;
		let object = object::File::parse(data).map_err(|error| elf_error(path, error))?;
		let sections = gimli::DwarfSections::load(|id| load_section(&object, id))
			.map_err(|error| elf_error(path, error))?;
		let dwarf = sections.borrow(|section| EndianSlice::new(section, LittleEndian));
		let mut debug_info = DebugInfo::default();
		debug_info
			.read(&dwarf)
			.map_err(|error| elf_error(path, format!("malformed DWARF: {error}")))?;

		Ok(Self::from_parts(info.text_vaddr, info.symbols, debug_info))
	}

	fn from_parts(text_address: u64, symbols: Vec<elf::Symbol>, debug_info: DebugInfo) -> Self {
		let DebugInfo {
			scopes,
			mut lines,
			files,
			..
		} = debug_info;
		let mut scope_index: Vec<(Range<u64>, usize)> = scopes
			.iter()
			.enumerate()
			.flat_map(|(index, scope)| scope.ranges.iter().map(move |range| (range.clone(), index)))
			.collect();
		scope_index.sort_by_key(|(range, _)| range.start);
		// End-of-sequence markers sort before rows starting at the same
		// address, so a sequence that begins where another ends wins.
		lines.sort_by_key(|row| (row.address, !row.end_sequence));

		Self {
			text_address,
			symbols,
			scopes,
			scope_index,
			lines,
			files,
		}
	}

	/// Whether the program carries a DWARF line table.
	#[must_use]
	pub fn has_line_info(&self) -> bool {
		!self.lines.is_empty()
	}

	/// The source frames for the instruction at `pc`, outermost first.
	///
	/// `pc` is an instruction index into `.text`, as recorded in register
	/// traces. The result always has at least one frame.
	#[must_use]
	pub fn frames(&self, pc: u64) -> Vec<Frame> {
		let address = self
			.text_address
			.saturating_add(pc.saturating_mul(SBF_INSTRUCTION_SIZE));
		let chain = self.scope_chain(address);
		let physical = self
			.symbol_at(address)
			.map(str::to_owned)
			.or_else(|| chain.first().and_then(|scope| scope.name.clone()))
			.unwrap_or_else(|| {
				format!(
					"<unknown+0x{:x}>",
					address.saturating_sub(self.text_address)
				)
			});
		let names = std::iter::once(physical).chain(
			chain
				.iter()
				.skip(1)
				.map(|scope| scope.name.clone().unwrap_or_else(|| "<inlined>".to_owned())),
		);
		let mut locations: Vec<Option<SourceLocation>> = chain
			.iter()
			.skip(1)
			.map(|scope| scope.call_site.clone())
			.collect();
		locations.push(self.location_at(address));

		names
			.zip(locations)
			.map(|(function, location)| Frame { function, location })
			.collect()
	}

	/// The symbol covering `address`, if any.
	fn symbol_at(&self, address: u64) -> Option<&str> {
		let index = self
			.symbols
			.partition_point(|symbol| symbol.address <= address)
			.checked_sub(1)?;
		let symbol = &self.symbols[index];
		let end = if symbol.size > 0 {
			symbol.address.saturating_add(symbol.size)
		} else {
			self.symbols
				.get(index + 1)
				.map_or(u64::MAX, |next| next.address)
		};

		(address < end).then_some(symbol.name.as_str())
	}

	/// The subprogram containing `address` followed by each nested inlined
	/// subroutine that contains it, outermost first.
	fn scope_chain(&self, address: u64) -> Vec<&Scope> {
		let mut chain = Vec::new();
		let candidate = self
			.scope_index
			.partition_point(|(range, _)| range.start <= address)
			.checked_sub(1)
			.map(|index| &self.scope_index[index]);
		let Some((_, index)) = candidate.filter(|(range, _)| range.contains(&address)) else {
			return chain;
		};

		let mut scope = &self.scopes[*index];
		chain.push(scope);

		while let Some(child) = scope.children.iter().find(|child| child.contains(address)) {
			chain.push(child);
			scope = child;
		}

		chain
	}

	/// The line-table location of `address`.
	fn location_at(&self, address: u64) -> Option<SourceLocation> {
		let index = self
			.lines
			.partition_point(|row| row.address <= address)
			.checked_sub(1)?;
		let row = self.lines[index];

		if row.end_sequence || row.line == 0 {
			return None;
		}

		Some(SourceLocation {
			file: self.files[row.file].clone(),
			line: row.line,
		})
	}
}

fn elf_error(path: &Path, error: impl std::fmt::Display) -> ProfileError {
	ProfileError::Elf {
		path: path.to_path_buf(),
		message: error.to_string(),
	}
}

fn load_section<'data>(
	object: &object::File<'data>,
	id: gimli::SectionId,
) -> Result<Cow<'data, [u8]>, object::Error> {
	match object.section_by_name(id.name()) {
		Some(section) => section.uncompressed_data(),
		None => Ok(Cow::Borrowed(&[])),
	}
}

/// Scopes, line rows, and interned file paths read from every unit.
#[derive(Debug, Default)]
struct DebugInfo {
	scopes: Vec<Scope>,
	lines: Vec<LineRow>,
	files: Vec<String>,
	file_index: HashMap<String, usize>,
}

/// The DWARF sections and every unit, so references can cross units.
struct Sections<'a, 'data> {
	dwarf: &'a Dwarf<'data>,
	units: &'a [Unit<'data>],
}

impl DebugInfo {
	fn read(&mut self, dwarf: &Dwarf<'_>) -> gimli::Result<()> {
		let mut headers = dwarf.units();
		let mut units = Vec::new();

		while let Some(header) = headers.next()? {
			units.push(dwarf.unit(header)?);
		}

		let sections = Sections {
			dwarf,
			units: &units,
		};

		for unit in &units {
			self.read_lines(dwarf, unit)?;

			let mut tree = unit.entries_tree(None)?;
			read_scopes(&sections, unit, tree.root()?, &mut self.scopes)?;
		}

		Ok(())
	}

	fn intern(&mut self, path: String) -> usize {
		if let Some(index) = self.file_index.get(&path) {
			return *index;
		}

		let index = self.files.len();
		self.file_index.insert(path.clone(), index);
		self.files.push(path);
		index
	}

	fn read_lines<'data>(&mut self, dwarf: &Dwarf<'data>, unit: &Unit<'data>) -> gimli::Result<()> {
		let Some(program) = unit.line_program.clone() else {
			return Ok(());
		};
		let mut rows = program.rows();
		let mut unit_files: HashMap<u64, usize> = HashMap::new();

		while let Some((header, row)) = rows.next_row()? {
			if row.end_sequence() {
				self.lines.push(LineRow {
					address: row.address(),
					file: 0,
					line: 0,
					end_sequence: true,
				});
				continue;
			}

			let file = *unit_files.entry(row.file_index()).or_insert_with(|| {
				let path = header
					.file(row.file_index())
					.map_or_else(String::new, |entry| file_path(dwarf, unit, header, entry));
				self.intern(path)
			});
			let line = row
				.line()
				.map_or(0, |line| u32::try_from(line.get()).unwrap_or(u32::MAX));

			self.lines.push(LineRow {
				address: row.address(),
				file,
				line,
				end_sequence: false,
			});
		}

		Ok(())
	}
}

/// Collect the subprograms and inlined subroutines below `node`.
///
/// Other entries (namespaces, lexical blocks) are transparent: their scopes
/// attach to the nearest enclosing scope.
fn read_scopes<'data>(
	sections: &Sections<'_, 'data>,
	unit: &Unit<'data>,
	node: gimli::EntriesTreeNode<'_, '_, Slice<'data>>,
	scopes: &mut Vec<Scope>,
) -> gimli::Result<()> {
	let mut children = node.children();

	while let Some(child) = children.next()? {
		let entry = child.entry();
		let tag = entry.tag();

		if tag != gimli::DW_TAG_subprogram && tag != gimli::DW_TAG_inlined_subroutine {
			read_scopes(sections, unit, child, scopes)?;
			continue;
		}

		let mut scope = Scope {
			ranges: entry_ranges(sections.dwarf, unit, entry)?,
			name: entry_name(sections, unit, entry, 0),
			call_site: call_site(sections.dwarf, unit, entry),
			children: Vec::new(),
		};
		read_scopes(sections, unit, child, &mut scope.children)?;

		if scope.ranges.is_empty() {
			// Declarations and abstract instances own no code.
			scopes.append(&mut scope.children);
		} else {
			scopes.push(scope);
		}
	}

	Ok(())
}

/// Where an inlined subroutine entry was inlined into its parent.
fn call_site<'data>(
	dwarf: &Dwarf<'data>,
	unit: &Unit<'data>,
	entry: &Entry<'data>,
) -> Option<SourceLocation> {
	let Some(AttributeValue::FileIndex(file_index)) = entry.attr_value(gimli::DW_AT_call_file)
	else {
		return None;
	};
	let line = entry.attr_value(gimli::DW_AT_call_line)?.udata_value()?;
	let header = unit.line_program.as_ref()?.header();

	Some(SourceLocation {
		file: file_path(dwarf, unit, header, header.file(file_index)?),
		line: u32::try_from(line).unwrap_or(u32::MAX),
	})
}

/// The non-empty address ranges of a scope entry.
fn entry_ranges<'data>(
	dwarf: &Dwarf<'data>,
	unit: &Unit<'data>,
	entry: &Entry<'data>,
) -> gimli::Result<Vec<Range<u64>>> {
	let mut ranges = Vec::new();
	let mut iter = dwarf.die_ranges(unit, entry)?;

	while let Some(range) = iter.next()? {
		ranges.push(range.begin..range.end);
	}

	// A `low_pc`/`high_pc` pair can describe an empty range, which would
	// shadow a real scope starting at the same address.
	ranges.retain(|range| !range.is_empty());
	Ok(ranges)
}

/// The name of a scope entry, following abstract origins and specifications.
///
/// A linkage name is preferred and demangled, since it carries the full path;
/// `line-tables-only` builds only record the short `DW_AT_name`. After LTO an
/// inlined subroutine's abstract origin is often in another unit.
fn entry_name<'data>(
	sections: &Sections<'_, 'data>,
	unit: &Unit<'data>,
	entry: &Entry<'data>,
	depth: u8,
) -> Option<String> {
	let dwarf = sections.dwarf;

	for attribute in [gimli::DW_AT_linkage_name, gimli::DW_AT_MIPS_linkage_name] {
		if let Some(name) = attribute_string(dwarf, unit, entry, attribute) {
			return Some(elf::demangle(&name));
		}
	}

	if let Some(name) = attribute_string(dwarf, unit, entry, gimli::DW_AT_name) {
		return Some(name);
	}

	// Origins chain at most a couple of levels; the bound stops a malformed
	// cycle from recursing forever.
	[gimli::DW_AT_abstract_origin, gimli::DW_AT_specification]
		.into_iter()
		.filter(|_| depth < MAX_ORIGIN_DEPTH)
		.find_map(|attribute| {
			let (origin_unit, offset) = match entry.attr_value(attribute)? {
				AttributeValue::UnitRef(offset) => (unit, offset),
				AttributeValue::DebugInfoRef(offset) => {
					sections.units.iter().find_map(|candidate| {
						Some((candidate, offset.to_unit_offset(&candidate.header)?))
					})?
				}
				_ => return None,
			};
			let origin = origin_unit.entry(offset).ok()?;
			entry_name(sections, origin_unit, &origin, depth + 1)
		})
}

fn attribute_string<'data>(
	dwarf: &Dwarf<'data>,
	unit: &Unit<'data>,
	entry: &Entry<'data>,
	attribute: gimli::DwAt,
) -> Option<String> {
	let value = entry.attr_value(attribute)?;
	let text = dwarf.attr_string(unit, value).ok()?;

	Some(text.to_string_lossy().into_owned())
}

/// Join a line-table file entry with its directory, as the compiler wrote it.
fn file_path<'data>(
	dwarf: &Dwarf<'data>,
	unit: &Unit<'data>,
	header: &gimli::LineProgramHeader<Slice<'data>>,
	entry: &gimli::FileEntry<Slice<'data>>,
) -> String {
	let name = dwarf
		.attr_string(unit, entry.path_name())
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let directory = entry
		.directory(header)
		.and_then(|directory| dwarf.attr_string(unit, directory).ok())
		.map(|directory| directory.to_string_lossy().into_owned())
		.unwrap_or_default();

	join_path(&directory, &name)
}

/// Join a recorded directory and file name with `/`, keeping absolute names.
fn join_path(directory: &str, name: &str) -> String {
	let absolute = name.starts_with('/') || name.as_bytes().get(1) == Some(&b':');

	if directory.is_empty() || absolute {
		return name.to_owned();
	}

	format!("{}/{name}", directory.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn symbol(name: &str, address: u64, size: u64) -> elf::Symbol {
		elf::Symbol {
			name: name.to_owned(),
			address,
			size,
		}
	}

	fn location(file: &str, line: u32) -> Option<SourceLocation> {
		Some(SourceLocation {
			file: file.to_owned(),
			line,
		})
	}

	fn row(address: u64, file: usize, line: u32) -> LineRow {
		LineRow {
			address,
			file,
			line,
			end_sequence: false,
		}
	}

	/// `.text` at 0x100: `outer` (0x100..0x140) inlines `middle`
	/// (0x110..0x130), which inlines `inner` (0x118..0x120).
	fn nested() -> Symbolizer {
		let inner = Scope {
			ranges: vec![0x118..0x120],
			name: Some("inner".to_owned()),
			call_site: location("src/middle.rs", 7),
			children: Vec::new(),
		};
		let middle = Scope {
			ranges: vec![0x110..0x130],
			name: None,
			call_site: location("src/outer.rs", 3),
			children: vec![inner],
		};
		let outer = Scope {
			ranges: vec![0x100..0x140],
			name: Some("outer_dwarf".to_owned()),
			call_site: None,
			children: vec![middle],
		};
		let debug_info = DebugInfo {
			scopes: vec![outer],
			lines: vec![
				row(0x100, 0, 1),
				row(0x110, 1, 5),
				row(0x118, 2, 9),
				row(0x120, 0, 0),
				LineRow {
					address: 0x140,
					file: 0,
					line: 0,
					end_sequence: true,
				},
			],
			files: vec![
				"src/outer.rs".to_owned(),
				"src/middle.rs".to_owned(),
				"src/inner.rs".to_owned(),
			],
			file_index: HashMap::new(),
		};

		Symbolizer::from_parts(0x100, vec![symbol("outer", 0x100, 0x40)], debug_info)
	}

	#[test]
	fn frames_expand_inlined_scopes_with_call_sites() {
		let frames = nested().frames(3); // 0x118

		assert_eq!(
			frames,
			vec![
				Frame {
					function: "outer".to_owned(),
					location: location("src/outer.rs", 3),
				},
				Frame {
					function: "<inlined>".to_owned(),
					location: location("src/middle.rs", 7),
				},
				Frame {
					function: "inner".to_owned(),
					location: location("src/inner.rs", 9),
				},
			]
		);
	}

	#[test]
	fn frames_report_missing_lines_and_unknown_addresses() {
		let symbolizer = nested();

		// Line 0 at 0x120 is compiler-generated code without a location.
		let generated = symbolizer.frames(4);
		assert_eq!(generated.len(), 2);
		assert_eq!(generated[1].location, None);

		// 0x140 is past every scope, symbol, and sequence.
		let outside = symbolizer.frames(8);
		assert_eq!(
			outside,
			vec![Frame {
				function: "<unknown+0x40>".to_owned(),
				location: None,
			}]
		);
		assert!(symbolizer.has_line_info());
	}

	#[test]
	fn dwarf_names_cover_addresses_without_symbols() {
		let symbolizer = Symbolizer::from_parts(
			0x100,
			Vec::new(),
			DebugInfo {
				scopes: vec![Scope {
					ranges: vec![0x100..0x108],
					name: Some("from_dwarf".to_owned()),
					call_site: None,
					children: Vec::new(),
				}],
				..DebugInfo::default()
			},
		);

		assert_eq!(symbolizer.frames(0)[0].function, "from_dwarf");
		assert_eq!(symbolizer.frames(1)[0].function, "<unknown+0x8>");
		assert!(!symbolizer.has_line_info());
	}

	#[test]
	fn zero_sized_symbols_extend_to_the_next_symbol() {
		let symbolizer = Symbolizer::from_parts(
			0,
			vec![symbol("first", 0, 0), symbol("second", 16, 0)],
			DebugInfo::default(),
		);

		assert_eq!(symbolizer.symbol_at(8), Some("first"));
		assert_eq!(symbolizer.symbol_at(16), Some("second"));
		assert_eq!(symbolizer.symbol_at(u64::MAX - 1), Some("second"));
	}

	/// An SBF ELF with `.text` at address 0 and the given extra sections,
	/// each optionally marked `SHF_COMPRESSED`.
	fn elf_with_sections(sections: &[(&str, Vec<u8>, bool)]) -> Vec<u8> {
		use object::write::Object;

		let mut elf = Object::new(
			object::BinaryFormat::Elf,
			object::Architecture::Sbf,
			object::Endianness::Little,
		);
		let text = elf.add_section(Vec::new(), b".text".to_vec(), object::SectionKind::Text);
		elf.set_section_data(text, vec![0x95; 0x50], 8);

		for (name, data, compressed) in sections {
			let id = elf.add_section(
				Vec::new(),
				name.as_bytes().to_vec(),
				object::SectionKind::Debug,
			);
			elf.set_section_data(id, data.clone(), 1);

			if *compressed {
				elf.section_mut(id).flags = object::SectionFlags::Elf {
					sh_type: object::elf::SHT_PROGBITS,
					sh_flags: object::elf::SHF_COMPRESSED,
				};
			}
		}

		let written = elf.write();
		assert!(written.is_ok(), "{written:?}");
		written.unwrap_or_default()
	}

	/// DWARF covering the shapes full debug information adds to what
	/// `line-tables-only` emits: a namespace and a lexical block around the
	/// scopes, a linkage name, an inlined subroutine whose origin uses an
	/// unsupported reference form, and a second unit without a line program
	/// whose scope refers to the first unit's declaration.
	fn synthetic_dwarf() -> Vec<(&'static str, Vec<u8>, bool)> {
		use gimli::write::Address;
		use gimli::write::AttributeValue as Value;
		use gimli::write::DebugInfoRef;
		use gimli::write::Dwarf as WriteDwarf;
		use gimli::write::EndianVec;
		use gimli::write::LineProgram;
		use gimli::write::LineString;
		use gimli::write::Range as WriteRange;
		use gimli::write::RangeList;
		use gimli::write::Sections;
		use gimli::write::Unit as WriteUnit;

		let encoding = gimli::Encoding {
			format: gimli::Format::Dwarf32,
			version: 4,
			address_size: 8,
		};
		let mut program = LineProgram::new(
			encoding,
			gimli::LineEncoding::default(),
			LineString::String(b"/work".to_vec()),
			None,
			LineString::String(b"src/lib.rs".to_vec()),
			None,
		);
		let directory = program.add_directory(LineString::String(b"src".to_vec()));
		let file = program.add_file(LineString::String(b"lib.rs".to_vec()), directory, None);
		program.begin_sequence(Some(Address::Constant(0)));
		for (offset, line) in [(0, 1), (0x10, 7), (0x30, 9)] {
			let row = program.row();
			row.address_offset = offset;
			row.file = file;
			row.line = line;
			program.generate_row();
		}
		program.end_sequence(0x40);

		let mut dwarf = WriteDwarf::new();
		let lines_unit = dwarf.units.add(WriteUnit::new(encoding, program));
		let unit = dwarf.units.get_mut(lines_unit);
		let root = unit.root();
		let helper = unit.add(root, gimli::DW_TAG_subprogram);
		unit.get_mut(helper)
			.set(gimli::DW_AT_name, Value::String(b"helper".to_vec()));
		let namespace = unit.add(root, gimli::DW_TAG_namespace);
		let outer = unit.add(namespace, gimli::DW_TAG_subprogram);
		let outer_entry = unit.get_mut(outer);
		outer_entry.set(
			gimli::DW_AT_linkage_name,
			Value::String(b"_ZN4demo5outer17h0123456789abcdefE".to_vec()),
		);
		outer_entry.set(gimli::DW_AT_low_pc, Value::Address(Address::Constant(0)));
		outer_entry.set(gimli::DW_AT_high_pc, Value::Udata(0x40));
		let block = unit.add(outer, gimli::DW_TAG_lexical_block);
		let ranges = unit.ranges.add(RangeList(vec![WriteRange::StartLength {
			begin: Address::Constant(0x10),
			length: 0x10,
		}]));
		let inlined = unit.add(block, gimli::DW_TAG_inlined_subroutine);
		let inlined_entry = unit.get_mut(inlined);
		inlined_entry.set(gimli::DW_AT_abstract_origin, Value::UnitRef(helper));
		inlined_entry.set(gimli::DW_AT_ranges, Value::RangeListRef(ranges));
		inlined_entry.set(gimli::DW_AT_call_file, Value::FileIndex(Some(file)));
		inlined_entry.set(gimli::DW_AT_call_line, Value::Udata(3));
		let unnamed = unit.add(block, gimli::DW_TAG_inlined_subroutine);
		let unnamed_entry = unit.get_mut(unnamed);
		unnamed_entry.set(
			gimli::DW_AT_abstract_origin,
			Value::DebugInfoRefSup(gimli::DebugInfoOffset(0)),
		);
		unnamed_entry.set(gimli::DW_AT_low_pc, Value::Address(Address::Constant(0x30)));
		unnamed_entry.set(gimli::DW_AT_high_pc, Value::Udata(0x8));

		let other_unit = dwarf
			.units
			.add(WriteUnit::new(encoding, LineProgram::none()));
		let other = dwarf.units.get_mut(other_unit);
		let other_root = other.root();
		let cross = other.add(other_root, gimli::DW_TAG_subprogram);
		let cross_entry = other.get_mut(cross);
		cross_entry.set(
			gimli::DW_AT_abstract_origin,
			Value::DebugInfoRef(DebugInfoRef::Entry(lines_unit, helper)),
		);
		cross_entry.set(gimli::DW_AT_low_pc, Value::Address(Address::Constant(0x40)));
		cross_entry.set(gimli::DW_AT_high_pc, Value::Udata(0x10));

		let mut sections = Sections::new(EndianVec::new(gimli::LittleEndian));
		let written = dwarf.write(&mut sections);
		assert!(written.is_ok(), "{written:?}");
		let mut debug = Vec::new();
		let collected = sections.for_each(|id, data| {
			if !data.slice().is_empty() {
				debug.push((id.name(), data.slice().to_vec(), false));
			}
			Ok::<(), gimli::write::Error>(())
		});
		assert!(collected.is_ok(), "{collected:?}");
		debug
	}

	#[test]
	fn reads_namespaces_blocks_linkage_names_and_cross_unit_origins() {
		let elf = elf_with_sections(&synthetic_dwarf());
		let symbolizer = Symbolizer::new(&elf, Path::new("synthetic.so"));
		assert!(symbolizer.is_ok(), "{symbolizer:?}");
		let symbolizer = symbolizer.unwrap_or_else(|_| unreachable!());

		assert_eq!(
			symbolizer.frames(2),
			vec![
				Frame {
					function: "demo::outer".to_owned(),
					location: location("src/lib.rs", 3),
				},
				Frame {
					function: "helper".to_owned(),
					location: location("src/lib.rs", 7),
				},
			]
		);
		assert_eq!(symbolizer.frames(6)[1].function, "<inlined>");
		assert_eq!(
			symbolizer.frames(8),
			vec![Frame {
				function: "helper".to_owned(),
				location: None,
			}]
		);
	}

	#[test]
	fn malformed_and_undecodable_debug_sections_are_rejected() {
		let malformed = elf_with_sections(&[(".debug_info", vec![0xff; 16], false)]);
		let error = Symbolizer::new(&malformed, Path::new("malformed.so"));
		assert!(
			matches!(&error, Err(ProfileError::Elf { message, .. }) if message.starts_with("malformed DWARF")),
			"{error:?}"
		);

		let compressed = elf_with_sections(&[(".debug_line", vec![1, 2, 3], true)]);
		let error = Symbolizer::new(&compressed, Path::new("compressed.so"));
		assert!(matches!(error, Err(ProfileError::Elf { .. })), "{error:?}");
	}

	#[test]
	fn join_path_keeps_absolute_names() {
		assert_eq!(join_path("src", "lib.rs"), "src/lib.rs");
		assert_eq!(join_path("src/", "lib.rs"), "src/lib.rs");
		assert_eq!(join_path("", "lib.rs"), "lib.rs");
		assert_eq!(join_path("src", "/abs/lib.rs"), "/abs/lib.rs");
		assert_eq!(join_path("src", "C:\\lib.rs"), "C:\\lib.rs");
	}

	#[test]
	fn intern_reuses_paths() {
		let mut debug_info = DebugInfo::default();

		assert_eq!(debug_info.intern("a.rs".to_owned()), 0);
		assert_eq!(debug_info.intern("b.rs".to_owned()), 1);
		assert_eq!(debug_info.intern("a.rs".to_owned()), 0);
	}
}
