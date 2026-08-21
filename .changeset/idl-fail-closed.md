---
pina: fix
pina_cli: fix
pina_macros: fix
---

# Make IDL extraction fail closed

Reject incomplete or ambiguous program source graphs instead of silently emitting partial Codama metadata. IDL generation now resolves explicit module paths, distinguishes missing conditional modules from missing required files, validates package names and entrypoint ownership, and requires PDA attributes and inferred PDA account links to resolve exactly.

Keep account and PDA naming on one canonical path, including structs named exactly `State`. Constant-only `#[pda]` declarations now generate valid typed seed helpers, allowing data-free signer PDAs to use the same `seeds().with_bump(...)` API and generated client defaults as state-bearing PDAs.
