pub(crate) mod accounts;
pub(crate) mod capacity;
pub(crate) mod discriminator;
pub(crate) mod errors;
pub(crate) mod events;
pub(crate) mod helpers;
pub(crate) mod instructions;
pub(crate) mod mods;
pub(crate) mod scaffold;
pub(crate) mod seeds;
pub(crate) mod types;

// Items used directly by lib.rs production code.
pub(crate) use accounts::is_compact_account;
pub(crate) use accounts::render_account_page;
pub(crate) use accounts::render_accounts_mod;
pub(crate) use capacity::CompactCapacityIndex;
pub(crate) use errors::render_errors_mod;
pub(crate) use errors::render_errors_page;
pub(crate) use events::render_event_page;
pub(crate) use events::render_events_mod;
pub(crate) use helpers::GENERATED_HEADER;
pub(crate) use helpers::page;
pub(crate) use helpers::program_id_const_name;
pub(crate) use helpers::snake;
pub(crate) use instructions::render_instruction_page;
pub(crate) use instructions::render_instructions_mod;
pub use instructions::render_migrate_instruction_page;
pub(crate) use mods::render_programs_mod;
pub(crate) use mods::render_root_mod;
pub(crate) use scaffold::ensure_crate_scaffold;
pub(crate) use scaffold::write_files;
pub(crate) use types::render_defined_type_page;
pub(crate) use types::render_defined_types_mod;

#[cfg(test)]
mod tests {
	use codama_nodes::EventNode;
	use codama_nodes::StructTypeNode;

	use super::*;

	#[test]
	fn events_barrel_reexports_and_lists_events_in_order() {
		let events = vec![
			EventNode::new("firstEvent", StructTypeNode::new(vec![])),
			EventNode::new("secondEvent", StructTypeNode::new(vec![])),
		];
		let barrel = render_events_mod(&events);

		assert!(barrel.contains("pub(crate) mod r#first_event;"), "{barrel}");
		assert!(
			barrel.contains("pub(crate) mod r#second_event;"),
			"{barrel}"
		);
		assert!(
			barrel.contains("pub use self::r#first_event::*;"),
			"{barrel}"
		);
		assert!(
			barrel.contains("pub use self::r#second_event::*;"),
			"{barrel}"
		);
	}
}
