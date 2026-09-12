---
pina: feat
pina_macros: feat
pina_cli: fix
---

# Load stored-bump compact PDAs in one borrow

Generate `Type::with_pda` for compact accounts with a stored bump. The helper validates the owner, compact representation, canonical bump, and derived PDA address before it runs the caller's closure.

Avoid repeated account-property validation inside compact resize and update operations. The IDL parser now recognizes `with_pda` as PDA validation.
