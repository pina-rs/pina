---
pina: breaking
pina_macros: breaking
---

# Reject duplicate mutable remaining accounts by default

Make mutable `#[pina(remaining)]` fields reject duplicate account addresses without requiring an additional attribute. Programs that intentionally accept aliases can restore the previous behavior with `#[pina(remaining, distinct = false)]` and a field doc comment explaining the invariant that makes duplicate writable accounts safe.

The new `require_reason_for_duplicate_remaining_accounts` Dylint rule enforces that documentation requirement.
