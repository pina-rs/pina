pub(crate) mod accounts;
pub(crate) mod discriminator;
pub(crate) mod errors;
pub(crate) mod helpers;
pub(crate) mod instructions;
pub(crate) mod mods;
pub(crate) mod scaffold;
pub(crate) mod seeds;
pub(crate) mod types;

// Items used directly by lib.rs production code.
pub(crate) use accounts::render_account_page;
pub(crate) use accounts::render_accounts_mod;
pub(crate) use errors::render_errors_mod;
pub(crate) use errors::render_errors_page;
pub(crate) use helpers::page;
pub(crate) use helpers::program_id_const_name;
pub(crate) use helpers::snake;
pub(crate) use instructions::render_instruction_page;
pub(crate) use instructions::render_instructions_mod;
pub(crate) use mods::render_programs_mod;
pub(crate) use mods::render_root_mod;
pub(crate) use scaffold::ensure_crate_scaffold;
pub(crate) use scaffold::write_files;
pub(crate) use types::render_defined_type_page;
pub(crate) use types::render_defined_types_mod;
