---
pina: fix
pina_cli: docs
pina_macros: fix
pina_skill: docs
---

# Preserve Declarative Validation at Mutation Boundaries

Zero fixed, instruction, event, and compact-creation storage when initialization or application validation fails. Compact builders now accept the account's generated patch type, including wrappers that implement `PinaCompactPatch`, so structural and application validation cannot be bypassed by an unrelated safe `PinaPodPatch` implementation. Callers must propagate post-patch errors so Solana rolls back rejected bytes and any associated rent movement.
