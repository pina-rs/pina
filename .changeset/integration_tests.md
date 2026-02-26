---
pina: patch
---

Add comprehensive end-to-end integration tests for the pina crate. The test suite covers:

- Full account lifecycle (create, write state, read/validate, update, close with rent return)
- Multi-instruction flows (Initialize then Update, verify state after each step)
- Error handling (invalid signer, wrong owner, discriminator mismatch, data length mismatch, invalid instruction discriminator, empty instruction data, wrong program ID, insufficient accounts, non-writable account, empty account rejection)
- Lamport transfer operations (send, insufficient funds, same-account rejection, close with recipient)
- PDA seed verification (derive and verify roundtrip, canonical bump assertion, assert_seeds_with_bump on AccountView)
- AccountView validation chains (chained assertions, short-circuit behavior)
- Discriminator dispatch across all instruction variants
- TryFromAccountInfos derive mapping and rejection of excess accounts
- Address assertion (single address and multi-address matching)

Tests use raw SVM input buffer construction to create AccountView instances without requiring compiled BPF programs, following the same memory layout as the pinocchio entrypoint deserializer.
