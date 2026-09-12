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

Opt migration-aware accounts, instruction payloads, and events into checked-in ABI history with recoverable pending deployments, immutable publication receipts that pin every published schema hash, and generated adjacent transitions. Programs can migrate fixed or compact account data on demand, old instruction payloads normalize into the current representation, compatible account lists may append optional slots, and pina_test can replay historical fixtures and verify rollback behavior. The IDL dispatch lint ignores generated runtime entrypoint helpers while continuing to check source handlers, including unsafe entrypoints.

**Behavior change for every Pina program:** the generated `Accounts` parser now treats a missing trailing optional account as `None` instead of failing with `NotEnoughAccountKeys`. Recompiled programs accept shortened account lists, so handlers must treat trailing optional accounts as untrusted and possibly absent in any request; keep program-address fillers in place when omitting optional accounts in the middle of a list.
