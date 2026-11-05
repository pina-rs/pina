---
pina: fix
pina_macros: fix
pina_cli: docs
pina_skill: docs
---

# Require an explicit budget for migration funding

An omitted migration lamport budget now permits zero payer transfers. Rent-free migrations still run, while funded growth requires an explicit cap shared across the reserved instruction's account slots. This prevents a small allocation increase from silently charging the payer for an existing account's entire rent deficit.
