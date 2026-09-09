---
pina: feat
pina_abi: feat
pina_cli: feat
pina_codama_renderer: feat
pina_cpi_renderer: feat
pina_lints: fix
pina_macros: feat
pina_test: feat
---

# Add first-class automatic ABI migrations

Opt migration-aware accounts, instruction payloads, and events into checked-in ABI history with recoverable pending deployments, immutable publication receipts, and generated adjacent transitions. Programs can migrate fixed or compact account data on demand, old instruction payloads normalize into the current representation, compatible account lists may append optional slots, and pina_test can replay historical fixtures and verify rollback behavior. The IDL dispatch lint ignores generated runtime entrypoint helpers while continuing to check source handlers, including unsafe entrypoints.
