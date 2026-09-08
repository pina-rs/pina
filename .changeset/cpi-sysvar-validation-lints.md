---
pina_lints: fix
pina_sdk_ids: docs
---

# Correct CPI and sysvar validation guidance

Require validation of the exact program account passed to dynamic CPI calls. Stop requiring unrelated program assertions for static CPI builders, and accept Pinocchio's checked typed sysvar loaders without a redundant `assert_sysvar()` call.
