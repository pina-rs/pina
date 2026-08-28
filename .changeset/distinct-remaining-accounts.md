---
pina: feat
pina_macros: feat
---

# Add strict mutable trailing accounts

Add opt-in `#[pina(remaining, distinct)]` parsing for programs whose mutable trailing accounts must have unique addresses. Existing `#[pina(remaining)]` pass-through behavior remains unchanged for instructions that intentionally accept aliases.
