---
pina_lints: feat
pina_cli: fix
---

# make lint contracts executable and expand coverage

Add a typed lint for unchecked mutable remaining-account access, enforce explicit pass/fail outcomes in UI fixtures, and fix missed runtime bounds, compound asset arithmetic, canonical instruction dispatch, and missing root program IDs.

Key the installed lint driver by the exact rustc commit so incompatible nightly builds cannot reuse the same cached binary.
