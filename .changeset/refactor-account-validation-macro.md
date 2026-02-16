---
pina: patch
---

Replaced four duplicate `AccountValidation` trait implementations for SPL token types with a single `impl_account_validation!` macro, reducing code duplication while preserving identical behavior. Also fixed the inverted condition in the `Mint` impl's `assert` method which incorrectly returned `Ok` when the condition was `false`.
