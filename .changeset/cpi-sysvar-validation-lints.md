---
pina_lints: fix
pina_sdk_ids: docs
---

# Correct CPI and sysvar validation guidance

Require validation of the exact program account passed to explicitly unverified CPI calls. Static builders and Pinocchio Token's verified `invoke_with_program` variants do not require a preceding account assertion. Accept Pinocchio's checked typed sysvar loaders without a redundant `assert_sysvar()` call.

Migration: remove assertions added only before `invoke`, `invoke_signed`, `invoke_with_program`, or `invoke_signed_with_program`; those APIs already fix or verify their target. Keep exact-target validation before `invoke_with_unverified_program` and `invoke_signed_with_unverified_program`. The lint follows function-item aliases through casts, assignments, `if`, and `match`, and only preserves alias provenance when every branch agrees.

Typed `Clock`, `Rent`, `Instructions`, and `SlotHashes` values are identified from their resolved Pinocchio type rather than a local name. Checked account-view constructors remain accepted. Values from unchecked byte constructors are rejected when used through methods, fields, inline expressions, or destructuring patterns.
