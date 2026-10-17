---
pina: patch
---

# Drop the unused `pastey` dependency

`pina` declared `pastey` as a direct dependency but never referenced it; it was left behind by an early manifest restructure. Removing it shrinks the published dependency graph and clears the `unused dependency` build warning.
