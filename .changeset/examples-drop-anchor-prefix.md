---
pina_cli: none
pina_codama_renderer: none
pina_cpi_renderer: none
pina_lints: none
---

# Give every example a consistent, descriptive name

The Anchor parity examples were mislabeled with an `anchor_` prefix even though they are Pina programs, and the remaining examples mixed suffixed and unsuffixed naming. Every example now ends with `_program` and its name describes what it demonstrates:

- `anchor_declare_id` → `declare_id_program`
- `anchor_declare_program` → `declare_program` (already followed the convention)
- `anchor_duplicate_mutable_accounts` → `duplicate_mutable_accounts_program`
- `anchor_errors` → `custom_errors_program`
- `anchor_events` → `events_program`
- `anchor_floats` → `float_accounts_program`
- `anchor_realloc` → `account_realloc_program`
- `anchor_system_accounts` → `system_accounts_program`
- `anchor_sysvars` → `sysvar_checks_program`
- `hello_solana` → `hello_solana_program`
- `transfer_sol` → `transfer_sol_program`
- `pina_bpf` → `pina_bpf_program`
- `compact_accounts` → `compact_accounts_program`

Regenerated the Codama IDL fixtures and the Rust, CPI, JavaScript, and Dart clients from the renamed example crates, and updated the workspace members, devenv targets, the compute-unit policy (including baseline aliases for profiling older revisions), Surfpool and Dart contract tests, CLI snapshots, and documentation to match. Instruction names and wire formats are unchanged.
