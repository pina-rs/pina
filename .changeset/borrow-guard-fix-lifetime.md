---
pina_lints: fix
---

# Preserve account locks during automatic lint fixes

Require manual review for all unused account borrow guard rewrites. A named guard can intentionally block incompatible account borrows until scope exit, so `pina lint --fix` must preserve its lifetime.
