---
pina: fix
---

# Preflight fallible account mutations

Check active borrows and the per-instruction growth limit before moving rent or closing account balances, so expected validation failures do not leave helper callers with partially mutated in-memory state.
