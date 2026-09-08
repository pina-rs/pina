---
pina_cli: none
pina_codama_renderer: none
pina_cpi_renderer: none
pina_lints: none
---

# Drop the `anchor_` prefix from example names

Renamed the Anchor parity examples so their names describe what they demonstrate instead of implying they are Anchor programs: `anchor_declare_id` → `declare_id`, `anchor_declare_program` → `declare_program`, `anchor_duplicate_mutable_accounts` → `duplicate_mutable_accounts`, `anchor_errors` → `custom_errors`, `anchor_events` → `events`, `anchor_floats` → `float_accounts`, `anchor_realloc` → `account_realloc`, `anchor_system_accounts` → `system_accounts`, and `anchor_sysvars` → `sysvar_checks`.

Regenerated the Codama IDL fixtures and the Rust, CPI, JavaScript, and Dart clients from the renamed example crates, and updated the workspace members, compute-unit policy, Surfpool tests, and documentation to match. `pina codama generate` is unaffected as an interface; only the example inventory names it emits changed.
