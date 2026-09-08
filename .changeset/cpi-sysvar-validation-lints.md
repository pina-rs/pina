---
pina_lints: breaking
pina_sdk_ids: docs
---

# Correct CPI and sysvar validation guidance

Require validation of the exact program account passed to explicitly unverified CPI calls. Static builders and Pinocchio Token's verified `invoke_with_program` variants do not require a preceding account assertion. Accept Pinocchio's checked typed sysvar loaders without a redundant `assert_sysvar()` call.

Migration: remove assertions added only before `invoke`, `invoke_signed`, `invoke_with_program`, or `invoke_signed_with_program`. Those APIs already fix or verify their target. Keep exact-target validation before direct `invoke_with_unverified_program` and `invoke_signed_with_unverified_program` calls.

The proof must be a resolved Pina assertion whose failure is enforced with `?`, `unwrap()`, or `expect()` on every continuing path. Failure-side `map_err()` and `inspect_err()` adapters are accepted before extraction, and the returned account value can be bound or chained. Discarded results, failure inspection, same-named methods, and one-branch checks no longer count. Success-side `map()`, `and_then()`, and `inspect()` adapters also do not establish proof because their callbacks can replace the validated binding before execution continues. Unverified CPI methods can no longer be stored as function values. Replace aliases, casts, containers, or closures with a direct method or UFCS call so the lint can see the exact target argument. A narrow lint allowance remains available for a reviewed abstraction that authenticates its target by another mechanism.

Checked `Clock`, `Rent`, `Instructions`, and `SlotHashes` account-view constructors remain accepted through ordinary Rust extraction, adapters, tuples, patterns, and control flow. Constructors that do not validate sysvar identity are now denied at their source. The affected APIs are the `Clock` and `Rent` byte constructors, `Instructions::new_unchecked`, and `SlotHashes::new` or `new_unchecked`. These constructors also cannot be stored as function values. Prefer a checked loader.

Before deliberate raw parsing, successfully call Pina's `assert_sysvar()` with the matching canonical `pina_sdk_ids::sysvar::<name>::ID` on every continuing path. The returned account value can be bound or chained, and failure-side adapters are accepted before extraction. Place a narrow, reviewed lint allowance directly on the unchecked constructor. Discarded results, failure inspection, same-named methods or ID constants, one-branch checks, and success-side callbacks no longer establish proof.
