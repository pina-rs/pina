---
pina_macros: breaking
pina: breaking
pina_lints: fix
pina_cli: fix
---

# Validate Stored Bumps at Runtime

The generated `assert_stored_bump` helper now validates the account owner and type and compares its supplied bump with the declared stored field before checking the PDA address. Transformed or fallback instruction bumps can no longer bypass that comparison through the lint's expression identities. The lint no longer claims to prove bump provenance.

Release mutable account-data borrows before calling `assert_stored_bump`: it now returns `ProgramError::AccountBorrowFailed` while such a borrow is held. The helper still accepts a matching stored noncanonical bump; use canonical creation or a checked PDA loader when namespace uniqueness is required.

Generated bump validation also preserves seed arguments named `bump` instead of shadowing them with the stored field.
