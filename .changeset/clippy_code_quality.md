---
pina: fix
pina_cli: fix
pina_macros: fix
---

# Remove Clippy warnings without changing public ergonomics

Clean up macro and code-generation helpers, remove dead test scaffolding, and pass Pina's pointer-sized `AccountView` handle by value inside private validators. Public account-validation and cursor APIs remain unchanged, and no account data is copied.
