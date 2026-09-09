---
pina_lints: breaking
pina_cli: breaking
---

# make lint contracts executable and expand coverage

Add a typed lint for unchecked mutable remaining-account access, including UFCS, function-item, and macro-expanded call shapes. Enforce explicit pass/fail outcomes in UI fixtures, and fix missed runtime bounds, stale or aliased remaining-account proofs, bounded iterator-adapter false positives, primitive-integer and nested asset arithmetic classification, mutable-borrow closure and alias gaps, canonical instruction dispatch, missing root program IDs, syntax-based or destructured borrow-guard misclassification, the no-op `let _ = guard` false negative, closure invocation ordering around CPI, and loop-carried invalidation of remaining-account bounds.

Key the installed lint driver by the complete `rustc -vV` report, including its commit when available, so distinct reported compiler identities cannot reuse the same cached binary and source-built compilers without commit metadata remain supported. Use a SHA-256 content identity so Cargo notices an in-place driver rebuild.

See the hardened security-lint migration guide for the duplicate-account and bound-check rewrites required by the new deny-level behavior.
