---
pina: feat
pina_abi: feat
pina_cli: feat
pina_macros: feat
pina_test: feat
---

# Add first-class automatic ABI migrations

Opt migration-aware accounts, instruction payloads, and events into checked-in ABI history with immutable publication receipts and generated adjacent transitions. Programs can migrate fixed or compact account data on demand, old instruction payloads normalize into the current representation, compatible account lists may append optional slots, and pina_test can replay historical fixtures and verify rollback behavior.
