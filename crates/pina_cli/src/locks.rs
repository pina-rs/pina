//! Static write-lock analysis for a parsed program.
//!
//! Solana's scheduler runs two transactions in parallel only when neither one
//! write-locks an account the other locks. Pina knows, before the program ever
//! runs, which accounts each instruction writes and which of them are PDAs. A
//! PDA whose seeds are all constants has the same address in every
//! transaction, so every instruction that writes it serializes all of that
//! instruction's traffic across the cluster. [`analyze`] turns a [`ProgramIr`]
//! into that lock picture: account nodes, per-instruction lock sets, the
//! instruction pairs that conflict, and the hotspots behind them.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use solana_address::Address;

use crate::doctor::escape_controls as escape;
use crate::error::IdlError;
use crate::ir::DefaultValueIr;
use crate::ir::InstructionAccountIr;
use crate::ir::InstructionIr;
use crate::ir::PdaIr;
use crate::ir::PdaSeedIr;
use crate::ir::ProgramIr;
use crate::project::Project;
use crate::project::ProjectError;

/// Version of the [`LockReport`] JSON document.
pub const LOCKS_SCHEMA_VERSION: u32 = 1;

/// The widest conflict matrix the text report draws, in columns. A wider
/// program gets a per-instruction list instead.
const MATRIX_MAX_WIDTH: usize = 120;

/// Where an account's address comes from, which decides whether two
/// instructions provably lock the same account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AddressClass {
	/// One address for every caller: a PDA whose seeds are all constants, or a
	/// known constant address such as a program or sysvar.
	Fixed,
	/// A PDA with variable seeds. Two transactions lock the same account only
	/// when they derive it from the same seed values.
	Keyed,
	/// An address the caller chooses. Pina cannot tell statically whether two
	/// transactions choose the same one, so caller accounts are never unified
	/// across instructions.
	Caller,
}

/// How certainly two instructions contend for a write lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
	/// They share a keyed PDA, or a fixed account one of them may omit, and at
	/// least one writes it: they wait for each other only when the addresses
	/// coincide at runtime.
	May,
	/// They share a fixed account and at least one writes it: they never run in
	/// parallel.
	Always,
}

/// One seed of a PDA account node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SeedSummary {
	/// Constant seed bytes, with their text when the bytes are printable ASCII.
	Constant { hex: String, text: Option<String> },
	/// A seed the caller supplies at runtime.
	Variable {
		name: String,
		#[serde(rename = "type")]
		rust_type: String,
	},
}

/// An account as the scheduler sees it, unified across instructions where the
/// address is provably shared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AccountNode {
	/// Stable key: `pda:<name>`, `address:<base58>`, or
	/// `caller:<instruction>.<slot>`.
	pub id: String,
	/// The PDA name, or the instruction slot that first names the account.
	pub name: String,
	pub class: AddressClass,
	/// The address every caller uses, when the class is `fixed` and the address
	/// is derivable.
	pub address: Option<String>,
	/// The PDA the account belongs to.
	pub pda: Option<String>,
	/// The PDA's seeds, in derivation order.
	pub seeds: Vec<SeedSummary>,
}

/// One instruction slot's lock on an account node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AccountLock {
	/// The [`AccountNode::id`] the slot locks.
	pub node: String,
	/// The instruction account slot.
	pub slot: String,
	pub signer: bool,
	/// Whether the caller may omit the account.
	pub optional: bool,
}

/// The accounts one instruction write-locks and read-locks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct InstructionLocks {
	pub name: String,
	pub writes: Vec<AccountLock>,
	pub reads: Vec<AccountLock>,
}

/// Two instructions that contend for at least one write lock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Conflict {
	/// The two instructions in declaration order. An instruction that
	/// contends with other transactions of itself is named twice.
	pub instructions: [String; 2],
	pub kind: ConflictKind,
	/// The account nodes that decide `kind`.
	pub nodes: Vec<String>,
}

/// A fixed account that at least one instruction writes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Hotspot {
	/// The [`AccountNode::id`] of the account.
	pub node: String,
	pub name: String,
	/// Instructions that write the account, in declaration order.
	pub writers: Vec<String>,
	/// Instructions that only read the account.
	pub readers: Vec<String>,
	/// Whether `[locks] allow` in `pina.toml` accepts the hotspot.
	pub allowed: bool,
}

/// The write-lock picture of one program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct LockReport {
	pub schema_version: u32,
	pub program: String,
	pub program_id: String,
	pub nodes: Vec<AccountNode>,
	pub instructions: Vec<InstructionLocks>,
	pub conflicts: Vec<Conflict>,
	pub hotspots: Vec<Hotspot>,
}

/// Errors produced while analysing a program's write locks.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LocksError {
	#[error(transparent)]
	Project(#[from] ProjectError),

	#[error(transparent)]
	Idl(#[from] IdlError),

	#[error("Program ID `{address}` is not a base58-encoded 32-byte address")]
	InvalidProgramId { address: String },

	#[error(
		"`[locks] allow` in pina.toml names `{name}`, which is not a hotspot of this program. \
		 Remove it, or name one of the hotspots: {hotspots}"
	)]
	UnknownAllowedAccount { name: String, hotspots: String },
}

/// Discover the project at or above `start` and analyse its write locks,
/// honoring its `[locks] allow` list.
///
/// # Errors
///
/// Returns an error when the project cannot be discovered, its configuration
/// is invalid, its source cannot be parsed, or an allowed name is not one of
/// its hotspots.
pub fn analyze_project(start: &Path) -> Result<LockReport, LocksError> {
	let loaded = load_project(start)?;

	analyze(&loaded.ir, &loaded.allow)
}

/// A discovered project with its parsed program and `[locks] allow` list.
pub(crate) struct LoadedProject {
	pub(crate) project: Project,
	pub(crate) ir: ProgramIr,
	pub(crate) allow: Vec<String>,
}

/// Discover the project at or above `start` and parse its program.
pub(crate) fn load_project(start: &Path) -> Result<LoadedProject, LocksError> {
	let project = Project::discover(start)?;
	let allow = project.locks_config()?.allow;
	let ir = crate::parse::parse_program(&project.program_dir, None)?;

	Ok(LoadedProject { project, ir, allow })
}

/// Analyse the write locks of `ir`.
///
/// `allow` names hotspots the program accepts on purpose, such as an admin
/// configuration only admin instructions write. Allowed hotspots are still
/// reported, marked `allowed`.
///
/// # Errors
///
/// Returns an error when the program ID is not a valid address, or when an
/// `allow` entry names an account that is not a hotspot.
pub fn analyze(ir: &ProgramIr, allow: &[String]) -> Result<LockReport, LocksError> {
	let program_id = ir.public_key.parse::<Address>().map_err(|_| {
		LocksError::InvalidProgramId {
			address: ir.public_key.clone(),
		}
	})?;
	let mut nodes = NodeTable::new(&ir.pdas, program_id);
	let mut instructions = Vec::with_capacity(ir.instructions.len());
	let mut accesses = Vec::with_capacity(ir.instructions.len());

	for instruction in &ir.instructions {
		let (locks, access) = instruction_locks(instruction, &mut nodes);
		instructions.push(locks);
		accesses.push(access);
	}

	let hotspots = find_hotspots(&nodes.nodes, &accesses, allow)?;

	Ok(LockReport {
		schema_version: LOCKS_SCHEMA_VERSION,
		program: ir.name.clone(),
		program_id: ir.public_key.clone(),
		conflicts: find_conflicts(&accesses),
		nodes: nodes.nodes,
		instructions,
		hotspots,
	})
}

/// Account nodes in first-appearance order, with an index by id.
struct NodeTable<'ir> {
	pdas: &'ir [PdaIr],
	program_id: Address,
	nodes: Vec<AccountNode>,
	classes: HashMap<String, AddressClass>,
}

impl<'ir> NodeTable<'ir> {
	fn new(pdas: &'ir [PdaIr], program_id: Address) -> Self {
		Self {
			pdas,
			program_id,
			nodes: Vec::new(),
			classes: HashMap::new(),
		}
	}

	/// The node a slot locks, created on first use.
	fn node_for(
		&mut self,
		instruction: &InstructionIr,
		account: &InstructionAccountIr,
	) -> (String, AddressClass) {
		let known_address = account.default_value.as_ref().map(|value| {
			match value {
				DefaultValueIr::ProgramId(address) | DefaultValueIr::PublicKey(address) => {
					address.as_str()
				}
			}
		});
		let id = match (&account.pda_name, known_address) {
			(Some(pda_name), _) => format!("pda:{pda_name}"),
			(None, Some(address)) => format!("address:{address}"),
			(None, None) => format!("caller:{}.{}", instruction.name, account.name),
		};

		if let Some(class) = self.classes.get(&id) {
			return (id, *class);
		}

		let node = match &account.pda_name {
			Some(pda_name) => self.pda_node(id.clone(), pda_name),
			None => {
				AccountNode {
					id: id.clone(),
					name: account.name.clone(),
					class: if known_address.is_some() {
						AddressClass::Fixed
					} else {
						AddressClass::Caller
					},
					address: known_address.map(str::to_owned),
					pda: None,
					seeds: Vec::new(),
				}
			}
		};
		let class = node.class;
		self.classes.insert(id.clone(), class);
		self.nodes.push(node);

		(id, class)
	}

	fn pda_node(&self, id: String, pda_name: &str) -> AccountNode {
		let pda = self.pdas.iter().find(|pda| pda.name == pda_name);
		let seeds = pda.map_or_else(Vec::new, |pda| pda.seeds.iter().map(seed_summary).collect());
		let constant_seeds = pda.and_then(|pda| {
			pda.seeds
				.iter()
				.map(|seed| {
					match seed {
						PdaSeedIr::Constant { value } => Some(value.as_slice()),
						PdaSeedIr::Variable { .. } => None,
					}
				})
				.collect::<Option<Vec<_>>>()
		});
		let (class, address) = match constant_seeds {
			Some(seeds) => {
				(
					AddressClass::Fixed,
					Address::try_find_program_address(&seeds, &self.program_id)
						.map(|(address, _)| address.to_string()),
				)
			}
			None => (AddressClass::Keyed, None),
		};

		AccountNode {
			id,
			name: pda_name.to_owned(),
			class,
			address,
			pda: Some(pda_name.to_owned()),
			seeds,
		}
	}
}

fn seed_summary(seed: &PdaSeedIr) -> SeedSummary {
	match seed {
		PdaSeedIr::Constant { value } => {
			SeedSummary::Constant {
				hex: value.iter().fold(String::new(), |mut hex, byte| {
					let _ = write!(hex, "{byte:02x}");
					hex
				}),
				text: value
					.iter()
					.all(|byte| byte.is_ascii_graphic() || *byte == b' ')
					.then(|| String::from_utf8_lossy(value).into_owned()),
			}
		}
		PdaSeedIr::Variable { name, rust_type } => {
			SeedSummary::Variable {
				name: name.clone(),
				rust_type: rust_type.clone(),
			}
		}
	}
}

/// How one instruction locks one node, merged over every slot that names it.
#[derive(Clone, Copy, Debug)]
struct Access {
	class: AddressClass,
	writable: bool,
	/// Every slot naming the node is optional.
	optional: bool,
}

/// One instruction's merged locks, keyed by node id.
struct InstructionAccess {
	name: String,
	nodes: BTreeMap<String, Access>,
}

fn instruction_locks(
	instruction: &InstructionIr,
	nodes: &mut NodeTable<'_>,
) -> (InstructionLocks, InstructionAccess) {
	let mut writes = Vec::new();
	let mut reads = Vec::new();
	let mut access = BTreeMap::<String, Access>::new();

	for account in &instruction.accounts {
		let (node, class) = nodes.node_for(instruction, account);
		access
			.entry(node.clone())
			.and_modify(|merged| {
				merged.writable |= account.is_writable;
				merged.optional &= account.is_optional;
			})
			.or_insert(Access {
				class,
				writable: account.is_writable,
				optional: account.is_optional,
			});

		let lock = AccountLock {
			node,
			slot: account.name.clone(),
			signer: account.is_signer,
			optional: account.is_optional,
		};
		if account.is_writable {
			writes.push(lock);
		} else {
			reads.push(lock);
		}
	}

	(
		InstructionLocks {
			name: instruction.name.clone(),
			writes,
			reads,
		},
		InstructionAccess {
			name: instruction.name.clone(),
			nodes: access,
		},
	)
}

fn find_conflicts(accesses: &[InstructionAccess]) -> Vec<Conflict> {
	let mut conflicts = Vec::new();

	for (index, first) in accesses.iter().enumerate() {
		for second in &accesses[index..] {
			conflicts.extend(conflict_between(first, second));
		}
	}

	conflicts
}

fn conflict_between(first: &InstructionAccess, second: &InstructionAccess) -> Option<Conflict> {
	let mut causes = Vec::new();

	for (node, first_access) in &first.nodes {
		let Some(second_access) = second.nodes.get(node) else {
			continue;
		};

		if !first_access.writable && !second_access.writable {
			continue;
		}

		let kind = match first_access.class {
			AddressClass::Fixed if !first_access.optional && !second_access.optional => {
				ConflictKind::Always
			}
			AddressClass::Fixed | AddressClass::Keyed => ConflictKind::May,
			// Only the instruction itself shares its caller node, and two of its
			// transactions may pass different accounts.
			AddressClass::Caller => continue,
		};
		causes.push((kind, node));
	}

	let kind = causes.iter().map(|(kind, _)| *kind).max()?;

	Some(Conflict {
		instructions: [first.name.clone(), second.name.clone()],
		kind,
		nodes: causes
			.into_iter()
			.filter(|(cause, _)| *cause == kind)
			.map(|(_, node)| node.clone())
			.collect(),
	})
}

fn find_hotspots(
	nodes: &[AccountNode],
	accesses: &[InstructionAccess],
	allow: &[String],
) -> Result<Vec<Hotspot>, LocksError> {
	let hotspots = nodes
		.iter()
		.filter(|node| node.class == AddressClass::Fixed)
		.filter_map(|node| {
			let lockers = |writable: bool| {
				accesses
					.iter()
					.filter(|instruction| {
						instruction
							.nodes
							.get(&node.id)
							.is_some_and(|access| access.writable == writable)
					})
					.map(|instruction| instruction.name.clone())
					.collect::<Vec<_>>()
			};
			let writers = lockers(true);

			(!writers.is_empty()).then(|| {
				Hotspot {
					node: node.id.clone(),
					name: node.name.clone(),
					writers,
					readers: lockers(false),
					allowed: allow.contains(&node.name),
				}
			})
		})
		.collect::<Vec<_>>();

	if let Some(unknown) = allow
		.iter()
		.find(|name| !hotspots.iter().any(|hotspot| hotspot.name == **name))
	{
		let names = hotspots
			.iter()
			.map(|hotspot| hotspot.name.as_str())
			.collect::<Vec<_>>();

		return Err(LocksError::UnknownAllowedAccount {
			name: unknown.clone(),
			hotspots: if names.is_empty() {
				"none".to_owned()
			} else {
				names.join(", ")
			},
		});
	}

	Ok(hotspots)
}

impl LockReport {
	/// Hotspots that `[locks] allow` does not accept.
	pub fn denied_hotspots(&self) -> impl Iterator<Item = &Hotspot> {
		self.hotspots.iter().filter(|hotspot| !hotspot.allowed)
	}

	/// Render the report as color-free text: hotspots first, then the
	/// instruction conflict matrix, then a legend.
	#[must_use]
	pub fn render_text(&self) -> String {
		let mut output = String::new();
		let allowed = self.hotspots.len() - self.denied_hotspots().count();
		let count =
			|class: AddressClass| self.nodes.iter().filter(|node| node.class == class).count();
		let _ = writeln!(output, "{} ({})", escape(&self.program), self.program_id);
		let _ = writeln!(
			output,
			"{}; accounts: {} fixed, {} keyed, {} caller-chosen; {} ({allowed} allowed)",
			counted(self.instructions.len(), "instruction"),
			count(AddressClass::Fixed),
			count(AddressClass::Keyed),
			count(AddressClass::Caller),
			counted(self.hotspots.len(), "hotspot"),
		);

		self.write_hotspots(&mut output);
		self.write_conflicts(&mut output);
		write_legend(&mut output);

		output
	}

	fn write_hotspots(&self, output: &mut String) {
		let _ = writeln!(output, "\nHotspots");

		if self.hotspots.is_empty() {
			let _ = writeln!(
				output,
				"  None. No instruction writes an account with a fixed address, so unrelated\n  \
				 users never queue behind each other."
			);
			return;
		}

		let _ = writeln!(
			output,
			"  A fixed account has one address for every caller, so every instruction that\n  \
			 writes it takes the same write lock."
		);

		for hotspot in &self.hotspots {
			let node = self.nodes.iter().find(|node| node.id == hotspot.node);
			let allowed = if hotspot.allowed {
				"  (allowed in pina.toml)"
			} else {
				""
			};
			let address = node
				.and_then(|node| node.address.as_deref())
				.unwrap_or("not derivable from its seeds");
			let _ = writeln!(output, "\n  {}{allowed}", escape(&hotspot.name));
			let _ = writeln!(output, "    address  {address}");

			if let Some(node) = node.filter(|node| !node.seeds.is_empty()) {
				let seeds = node.seeds.iter().map(seed_text).collect::<Vec<_>>();
				let _ = writeln!(output, "    seeds    {}", seeds.join(", "));
			}

			let _ = writeln!(output, "    writers  {}", hotspot.writers.join(", "));
			if !hotspot.readers.is_empty() {
				let _ = writeln!(output, "    readers  {}", hotspot.readers.join(", "));
			}
			let _ = writeln!(output, "    {}", consequence(hotspot));
		}
	}

	fn write_conflicts(&self, output: &mut String) {
		let _ = writeln!(output, "\nConflicts");

		if self.instructions.is_empty() {
			let _ = writeln!(output, "  The program declares no instructions.");
			return;
		}

		let kinds = self.conflict_kinds();
		let names = self
			.instructions
			.iter()
			.map(|instruction| escape(&instruction.name))
			.collect::<Vec<_>>();
		let number_width = names.len().to_string().len();
		let name_width = names
			.iter()
			.map(|name| name.chars().count())
			.max()
			.unwrap_or_default();
		let label_width = 2 + number_width + 1 + name_width;

		if label_width + 3 * names.len() > MATRIX_MAX_WIDTH {
			write_conflict_list(output, &names, &kinds);
			return;
		}

		let mut header = " ".repeat(label_width);
		for column in 1..=names.len() {
			let _ = write!(header, "{column:>3}");
		}
		let _ = writeln!(output, "{header}");

		for (row, name) in names.iter().enumerate() {
			let mut line = format!("  {:>number_width$} {name:<name_width$}", row + 1);
			for column in 0..names.len() {
				let _ = write!(line, "{:>3}", symbol(kinds.get(&ordered(row, column))));
			}
			let _ = writeln!(output, "{line}");
		}
	}

	/// Conflict kinds by instruction index pair, lower index first.
	fn conflict_kinds(&self) -> HashMap<(usize, usize), ConflictKind> {
		let index = self
			.instructions
			.iter()
			.enumerate()
			.map(|(index, instruction)| (instruction.name.as_str(), index))
			.collect::<HashMap<_, _>>();

		self.conflicts
			.iter()
			.filter_map(|conflict| {
				let [first, second] = &conflict.instructions;
				Some((
					ordered(*index.get(first.as_str())?, *index.get(second.as_str())?),
					conflict.kind,
				))
			})
			.collect()
	}
}

fn write_conflict_list(
	output: &mut String,
	names: &[String],
	kinds: &HashMap<(usize, usize), ConflictKind>,
) {
	for (row, name) in names.iter().enumerate() {
		let partners = |kind: ConflictKind| {
			names
				.iter()
				.enumerate()
				.filter(|(column, _)| kinds.get(&ordered(row, *column)) == Some(&kind))
				.map(|(_, partner)| partner.as_str())
				.collect::<Vec<_>>()
		};
		let always = partners(ConflictKind::Always);
		let may = partners(ConflictKind::May);
		let _ = writeln!(output, "  {name}");

		if always.is_empty() && may.is_empty() {
			let _ = writeln!(output, "    {} none", symbol(None));
		}
		if !always.is_empty() {
			let _ = writeln!(
				output,
				"    {} always  {}",
				symbol(Some(&ConflictKind::Always)),
				always.join(", ")
			);
		}
		if !may.is_empty() {
			let _ = writeln!(
				output,
				"    {} may     {}",
				symbol(Some(&ConflictKind::May)),
				may.join(", ")
			);
		}
	}
}

fn write_legend(output: &mut String) {
	let _ = writeln!(output, "\nLegend");
	let _ = writeln!(
		output,
		"  {}  always  both lock a fixed account and at least one writes it: never in parallel",
		symbol(Some(&ConflictKind::Always))
	);
	let _ = writeln!(
		output,
		"  {}  may     both lock a keyed PDA, or a fixed account one of them may omit, and",
		symbol(Some(&ConflictKind::May))
	);
	let _ = writeln!(
		output,
		"             at least one writes it: they wait only when the addresses match"
	);
	let _ = writeln!(
		output,
		"  {}  none    no shared write lock Pina can prove; caller-chosen accounts are never \
		 compared",
		symbol(None)
	);
}

/// `count` followed by `noun`, pluralized with `s` when the count is not one.
fn counted(count: usize, noun: &str) -> String {
	if count == 1 {
		format!("1 {noun}")
	} else {
		format!("{count} {noun}s")
	}
}

fn ordered(first: usize, second: usize) -> (usize, usize) {
	(first.min(second), first.max(second))
}

fn symbol(kind: Option<&ConflictKind>) -> char {
	match kind {
		Some(ConflictKind::Always) => '●',
		Some(ConflictKind::May) => '◐',
		None => '·',
	}
}

fn seed_text(seed: &SeedSummary) -> String {
	match seed {
		SeedSummary::Constant {
			text: Some(text), ..
		} => format!("{text:?}"),
		SeedSummary::Constant { hex, text: None } => format!("0x{hex}"),
		SeedSummary::Variable { name, rust_type } => {
			format!("<{}: {}>", escape(name), escape(rust_type))
		}
	}
}

/// The one-line cost of a hotspot, in terms of the instructions it serializes.
fn consequence(hotspot: &Hotspot) -> String {
	let writers = hotspot
		.writers
		.iter()
		.map(|writer| format!("`{}`", escape(writer)))
		.collect::<Vec<_>>();
	let mut sentence = format!(
		"Every {} in the cluster runs one at a time",
		join_with_and(&writers)
	);

	match hotspot.readers.as_slice() {
		[] => {}
		[reader] => {
			let _ = write!(sentence, "; `{}` waits for each one", escape(reader));
		}
		readers => {
			let _ = write!(
				sentence,
				"; the {} instructions that read it wait for each one",
				readers.len()
			);
		}
	}
	sentence.push('.');

	sentence
}

/// Join items as prose: `a`, `a and b`, or `a, b, and c`.
fn join_with_and(items: &[String]) -> String {
	match items {
		[first, second] => format!("{first} and {second}"),
		[rest @ .., last] if rest.len() > 1 => format!("{}, and {last}", rest.join(", ")),
		_ => items.concat(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ir::DiscriminatorIr;

	const PROGRAM_ID: &str = "GJQcuWrT2f3f4KNuJcXhhwUa1ZQTYbxzzJ1hotzKu8hS";
	const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

	/// A read-only, required, caller-chosen slot.
	fn slot(name: &str) -> InstructionAccountIr {
		InstructionAccountIr {
			name: name.to_owned(),
			is_writable: false,
			is_signer: false,
			is_optional: false,
			default_value: None,
			is_pda: false,
			pda_name: None,
			constraints: Vec::new(),
			docs: Vec::new(),
		}
	}

	fn writable(account: InstructionAccountIr) -> InstructionAccountIr {
		InstructionAccountIr {
			is_writable: true,
			..account
		}
	}

	fn optional(account: InstructionAccountIr) -> InstructionAccountIr {
		InstructionAccountIr {
			is_optional: true,
			..account
		}
	}

	fn signer(account: InstructionAccountIr) -> InstructionAccountIr {
		InstructionAccountIr {
			is_signer: true,
			..account
		}
	}

	fn pda(name: &str, pda_name: &str) -> InstructionAccountIr {
		InstructionAccountIr {
			is_pda: true,
			pda_name: Some(pda_name.to_owned()),
			..slot(name)
		}
	}

	fn known(name: &str, address: &str) -> InstructionAccountIr {
		InstructionAccountIr {
			default_value: Some(DefaultValueIr::PublicKey(address.to_owned())),
			..slot(name)
		}
	}

	fn instruction(name: &str, accounts: Vec<InstructionAccountIr>) -> InstructionIr {
		InstructionIr {
			name: name.to_owned(),
			rust_name: name.to_owned(),
			accounts,
			arguments: Vec::new(),
			discriminator: DiscriminatorIr {
				value: 0,
				repr_size: 1,
			},
			docs: Vec::new(),
		}
	}

	fn constant(value: &[u8]) -> PdaSeedIr {
		PdaSeedIr::Constant {
			value: value.to_vec(),
		}
	}

	fn program(instructions: Vec<InstructionIr>) -> ProgramIr {
		ProgramIr {
			name: "locks_program".to_owned(),
			public_key: PROGRAM_ID.to_owned(),
			pinapod_enums: Vec::new(),
			accounts: Vec::new(),
			instructions,
			events: Vec::new(),
			errors: Vec::new(),
			pdas: vec![
				PdaIr {
					name: "config".to_owned(),
					seeds: vec![constant(b"config")],
				},
				PdaIr {
					name: "position".to_owned(),
					seeds: vec![
						constant(b"position"),
						PdaSeedIr::Variable {
							name: "owner".to_owned(),
							rust_type: "Address".to_owned(),
						},
					],
				},
				PdaIr {
					name: "oversized".to_owned(),
					seeds: vec![constant(&[0; 33])],
				},
				PdaIr {
					name: "binary".to_owned(),
					seeds: vec![constant(&[0x00, 0xff])],
				},
			],
		}
	}

	fn analyze_ok(ir: &ProgramIr, allow: &[&str]) -> LockReport {
		let allow = allow
			.iter()
			.map(|name| (*name).to_owned())
			.collect::<Vec<_>>();

		analyze(ir, &allow).unwrap_or_else(|error| panic!("analysis failed: {error}"))
	}

	/// Every conflict as `(first, second, kind)`.
	fn conflicts(report: &LockReport) -> Vec<(&str, &str, ConflictKind)> {
		report
			.conflicts
			.iter()
			.map(|conflict| {
				let [first, second] = &conflict.instructions;
				(first.as_str(), second.as_str(), conflict.kind)
			})
			.collect()
	}

	fn node<'report>(report: &'report LockReport, id: &str) -> &'report AccountNode {
		let found = report.nodes.iter().find(|node| node.id == id);

		found.unwrap_or_else(|| panic!("no node `{id}`"))
	}

	#[test]
	fn fixed_pdas_derive_their_address_and_serialize_every_writer() {
		let ir = program(vec![
			instruction(
				"initialize",
				vec![signer(slot("admin")), writable(pda("config", "config"))],
			),
			instruction("update", vec![writable(pda("config", "config"))]),
			instruction("read", vec![pda("config", "config")]),
			instruction("peek", vec![pda("settings", "config")]),
		]);
		let report = analyze_ok(&ir, &[]);
		let config = node(&report, "pda:config");

		assert_eq!(config.class, AddressClass::Fixed);
		// Derived independently with `@solana/kit`'s `getProgramDerivedAddress`.
		assert_eq!(
			config.address.as_deref(),
			Some("9K52hxhxWeLv1mjTm6cePqA4tpEfXKspu7LJMhQ88eYS")
		);
		assert_eq!(
			conflicts(&report),
			[
				("initialize", "initialize", ConflictKind::Always),
				("initialize", "update", ConflictKind::Always),
				("initialize", "read", ConflictKind::Always),
				("initialize", "peek", ConflictKind::Always),
				("update", "update", ConflictKind::Always),
				("update", "read", ConflictKind::Always),
				("update", "peek", ConflictKind::Always),
			]
		);
		assert_eq!(report.conflicts[0].nodes, ["pda:config"]);
		assert_eq!(report.hotspots.len(), 1);
		assert_eq!(report.hotspots[0].writers, ["initialize", "update"]);
		assert_eq!(report.hotspots[0].readers, ["read", "peek"]);
		assert!(!report.hotspots[0].allowed);
		assert_eq!(report.denied_hotspots().count(), 1);
		assert_eq!(report.instructions[0].reads[0].slot, "admin");
		assert!(report.instructions[0].reads[0].signer);
		assert_eq!(report.instructions[0].writes[0].node, "pda:config");
	}

	#[test]
	fn keyed_pdas_may_conflict_and_caller_accounts_never_do() {
		let ir = program(vec![
			instruction(
				"open",
				vec![
					writable(slot("payer")),
					writable(pda("position", "position")),
				],
			),
			instruction("close", vec![writable(pda("position", "position"))]),
			instruction("inspect", vec![pda("position", "position")]),
			instruction("pay", vec![writable(slot("payer"))]),
		]);
		let report = analyze_ok(&ir, &[]);
		let position = node(&report, "pda:position");

		assert_eq!(position.class, AddressClass::Keyed);
		assert_eq!(position.address, None);
		assert_eq!(
			position.seeds,
			[
				SeedSummary::Constant {
					hex: "706f736974696f6e".to_owned(),
					text: Some("position".to_owned()),
				},
				SeedSummary::Variable {
					name: "owner".to_owned(),
					rust_type: "Address".to_owned(),
				},
			]
		);
		assert_eq!(
			node(&report, "caller:open.payer").class,
			AddressClass::Caller
		);
		assert_eq!(
			node(&report, "caller:pay.payer").class,
			AddressClass::Caller
		);
		assert_eq!(
			conflicts(&report),
			[
				("open", "open", ConflictKind::May),
				("open", "close", ConflictKind::May),
				("open", "inspect", ConflictKind::May),
				("close", "close", ConflictKind::May),
				("close", "inspect", ConflictKind::May),
			]
		);
		assert!(report.hotspots.is_empty());
	}

	#[test]
	fn known_addresses_unify_and_optional_fixed_accounts_may_conflict() {
		let ir = program(vec![
			instruction("first", vec![known("system_program", SYSTEM_PROGRAM)]),
			instruction("second", vec![known("system", SYSTEM_PROGRAM)]),
			instruction("maybe", vec![optional(writable(pda("config", "config")))]),
			instruction("always", vec![writable(pda("config", "config"))]),
			instruction(
				"twice",
				vec![
					optional(pda("config", "config")),
					writable(pda("config_again", "config")),
				],
			),
		]);
		let report = analyze_ok(&ir, &[]);
		let system = node(&report, &format!("address:{SYSTEM_PROGRAM}"));

		assert_eq!(system.class, AddressClass::Fixed);
		assert_eq!(system.name, "system_program");
		assert_eq!(system.address.as_deref(), Some(SYSTEM_PROGRAM));
		assert_eq!(
			conflicts(&report),
			[
				("maybe", "maybe", ConflictKind::May),
				("maybe", "always", ConflictKind::May),
				("maybe", "twice", ConflictKind::May),
				("always", "always", ConflictKind::Always),
				("always", "twice", ConflictKind::Always),
				("twice", "twice", ConflictKind::Always),
			]
		);
		assert_eq!(report.instructions[4].reads.len(), 1);
		assert!(report.instructions[4].reads[0].optional);
		assert_eq!(report.instructions[4].writes.len(), 1);
	}

	#[test]
	fn written_known_addresses_are_hotspots() {
		let ir = program(vec![instruction(
			"collect",
			vec![writable(known("fee_vault", SYSTEM_PROGRAM))],
		)]);
		let report = analyze_ok(&ir, &["fee_vault"]);

		assert_eq!(report.hotspots[0].name, "fee_vault");
		assert!(report.hotspots[0].allowed);
		assert_eq!(report.denied_hotspots().count(), 0);

		let text = report.render_text();
		assert!(text.contains("fee_vault  (allowed in pina.toml)"), "{text}");
		assert!(!text.contains("seeds "), "{text}");
		assert!(
			text.contains("Every `collect` in the cluster runs one at a time."),
			"{text}"
		);
	}

	#[test]
	fn allow_entries_must_name_hotspots() {
		let ir = program(vec![instruction(
			"update",
			vec![writable(pda("config", "config"))],
		)]);
		let allow = vec!["confg".to_owned()];
		let error = analyze(&ir, &allow).expect_err("a misspelled hotspot must be rejected");
		let message = error.to_string();

		assert!(message.contains("`confg`"), "{message}");
		assert!(message.contains("hotspots: config"), "{message}");

		let quiet = program(vec![instruction("read", vec![pda("config", "config")])]);
		let error = analyze(&quiet, &allow).expect_err("a program without hotspots allows none");

		assert!(error.to_string().contains("hotspots: none"), "{error}");
	}

	#[test]
	fn rejects_a_program_id_that_is_not_an_address() {
		let mut ir = program(Vec::new());
		ir.public_key = "not-base58!".to_owned();
		let error = analyze(&ir, &[]).expect_err("an invalid program id must be rejected");

		assert!(matches!(error, LocksError::InvalidProgramId { .. }));
		assert!(error.to_string().contains("not-base58!"));
	}

	#[test]
	fn underivable_binary_and_missing_seeds_are_reported_honestly() {
		let ir = program(vec![instruction(
			"stuff",
			vec![
				writable(pda("big", "oversized")),
				writable(pda("raw", "binary")),
				pda("ghost", "missing"),
			],
		)]);
		let report = analyze_ok(&ir, &[]);
		let oversized = node(&report, "pda:oversized");
		let missing = node(&report, "pda:missing");

		assert_eq!(oversized.class, AddressClass::Fixed);
		assert_eq!(oversized.address, None);
		assert_eq!(missing.class, AddressClass::Keyed);
		assert!(missing.seeds.is_empty());

		let text = report.render_text();
		assert!(
			text.contains("address  not derivable from its seeds"),
			"{text}"
		);
		assert!(text.contains("seeds    0x00ff"), "{text}");
	}

	#[test]
	fn renders_hotspots_with_readers_and_a_conflict_matrix() {
		let ir = program(vec![
			instruction("initialize", vec![writable(pda("config", "config"))]),
			instruction(
				"deposit",
				vec![
					writable(pda("config", "config")),
					writable(pda("position", "position")),
				],
			),
			instruction("view", vec![pda("config", "config")]),
			instruction("audit", vec![pda("config", "config")]),
			instruction("withdraw", vec![pda("position", "position")]),
		]);
		let text = analyze_ok(&ir, &[]).render_text();

		assert!(
			text.starts_with(&format!("locks_program ({PROGRAM_ID})\n")),
			"{text}"
		);
		assert!(
			text.contains(
				"5 instructions; accounts: 1 fixed, 1 keyed, 0 caller-chosen; 1 hotspot (0 \
				 allowed)"
			),
			"{text}"
		);
		assert!(text.contains("    seeds    \"config\"\n"), "{text}");
		assert!(text.contains("    readers  view, audit\n"), "{text}");
		assert!(
			text.contains(
				"Every `initialize` and `deposit` in the cluster runs one at a time; the 2 \
				 instructions that read it wait for each one."
			),
			"{text}"
		);
		assert!(text.contains("                1  2  3  4  5\n"), "{text}");
		assert!(text.contains("  2 deposit     ●  ●  ●  ●  ◐\n"), "{text}");
		assert!(text.contains("  5 withdraw    ·  ◐  ·  ·  ·\n"), "{text}");
		assert!(text.contains("Legend"), "{text}");

		let single_reader = program(vec![
			instruction("a", vec![writable(pda("config", "config"))]),
			instruction("b", vec![writable(pda("config", "config"))]),
			instruction("c", vec![writable(pda("config", "config"))]),
			instruction("d", vec![pda("config", "config")]),
		]);
		let text = analyze_ok(&single_reader, &[]).render_text();

		assert!(
			text.contains(
				"Every `a`, `b`, and `c` in the cluster runs one at a time; `d` waits for each \
				 one."
			),
			"{text}"
		);
	}

	#[test]
	fn renders_a_conflict_list_when_the_matrix_is_too_wide() {
		let mut instructions = (0..30)
			.map(|index| {
				instruction(
					&format!("instruction_with_a_long_name_{index:02}"),
					vec![writable(pda("config", "config"))],
				)
			})
			.collect::<Vec<_>>();
		instructions.push(instruction(
			"open_position",
			vec![writable(pda("position", "position"))],
		));
		instructions.push(instruction("idle", vec![slot("nothing")]));
		let text = analyze_ok(&program(instructions), &[]).render_text();

		assert!(!text.contains("  1  2  3"), "{text}");
		assert!(
			text.contains(
				"  instruction_with_a_long_name_00\n    ● always  instruction_with_a_long_name_00"
			),
			"{text}"
		);
		assert!(
			text.contains("  open_position\n    ◐ may     open_position\n"),
			"{text}"
		);
		assert!(text.contains("  idle\n    · none\n"), "{text}");
	}

	#[test]
	fn renders_programs_without_instructions() {
		let text = analyze_ok(&program(Vec::new()), &[]).render_text();

		assert!(text.contains("0 instructions;"), "{text}");
		assert!(
			text.contains("  None. No instruction writes an account with a fixed address"),
			"{text}"
		);
		assert!(
			text.contains("  The program declares no instructions."),
			"{text}"
		);
	}

	#[test]
	fn seeds_render_as_text_hex_or_typed_slots() {
		let rendered = [
			SeedSummary::Constant {
				hex: "6869".to_owned(),
				text: Some("hi".to_owned()),
			},
			SeedSummary::Constant {
				hex: "00ff".to_owned(),
				text: None,
			},
			SeedSummary::Variable {
				name: "owner".to_owned(),
				rust_type: "Address".to_owned(),
			},
		]
		.iter()
		.map(seed_text)
		.collect::<Vec<_>>();

		assert_eq!(rendered, ["\"hi\"", "0x00ff", "<owner: Address>"]);
	}

	#[test]
	fn json_uses_the_documented_schema() {
		let ir = program(vec![instruction(
			"update",
			vec![writable(pda("config", "config"))],
		)]);
		let json = serde_json::to_value(analyze_ok(&ir, &[]));
		let json = json.unwrap_or_else(|error| panic!("report must serialize: {error}"));

		assert_eq!(json["schemaVersion"], LOCKS_SCHEMA_VERSION);
		assert_eq!(json["programId"], PROGRAM_ID);
		assert_eq!(json["nodes"][0]["class"], "fixed");
		assert_eq!(json["nodes"][0]["seeds"][0]["kind"], "constant");
		assert_eq!(json["nodes"][0]["seeds"][0]["text"], "config");
		assert_eq!(json["instructions"][0]["writes"][0]["slot"], "config");
		assert_eq!(json["conflicts"][0]["instructions"][1], "update");
		assert_eq!(json["conflicts"][0]["kind"], "always");
		assert_eq!(json["hotspots"][0]["allowed"], false);
	}
}
