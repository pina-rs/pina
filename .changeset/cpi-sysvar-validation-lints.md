---
pina_lints: fix
pina_sdk_ids: docs
---

# Correct CPI and sysvar validation guidance

Require validation of the exact program account passed to explicitly unverified CPI calls. Static builders and Pinocchio Token's verified `invoke_with_program` variants do not require a preceding account assertion. Accept Pinocchio's checked typed sysvar loaders without a redundant `assert_sysvar()` call.

Migration: remove assertions added only before `invoke`, `invoke_signed`, `invoke_with_program`, or `invoke_signed_with_program`; those APIs already fix or verify their target. Keep exact-target validation before direct `invoke_with_unverified_program` and `invoke_signed_with_unverified_program` calls. Unverified CPI methods can no longer be stored as function values: replace aliases, casts, containers, or closures with a direct method or UFCS call so the lint can see the exact target argument. A narrowly scoped lint allowance remains available for a reviewed abstraction that authenticates its target by another mechanism.

Checked `Clock`, `Rent`, `Instructions`, and `SlotHashes` account-view constructors remain accepted through ordinary Rust extraction, adapters, tuples, patterns, and control flow. Known unchecked `from_bytes` and `from_bytes_unchecked` constructors are now denied where they are called instead of relying on downstream value-provenance inference. Prefer a checked loader. Deliberate raw parsing must call `assert_sysvar()` before borrowing data and place a narrow, reviewed lint allowance directly on the unchecked constructor.
