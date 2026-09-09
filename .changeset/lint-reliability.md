---
pina_lints: breaking
pina_cli: breaking
---

# make lint contracts executable and expand coverage

Add a typed lint for unchecked mutable remaining-account access, including UFCS, function-item, and macro-expanded call shapes. Enforce explicit pass/fail outcomes in UI fixtures, and fix missed runtime bounds, stale or aliased remaining-account proofs, compound asset arithmetic, canonical instruction dispatch, missing root program IDs, and syntax-based borrow-guard misclassification.

Key the installed lint driver by the exact rustc commit so incompatible nightly builds cannot reuse the same cached binary.

See the hardened security-lint migration guide for the duplicate-account and bound-check rewrites required by the new deny-level behavior.
