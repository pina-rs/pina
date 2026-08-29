---
pina_cli: fix
---

# Recover Native Lint Bundle Publication

Select lint libraries by the active Rust compiler host, omit impossible musl `cdylib` builds, and keep Windows archive paths local so automated releases can publish every supported lint bundle.
