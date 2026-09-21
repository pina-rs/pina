---
pina: fix
---

# Adopt the Pinapod 0.4.2 compact-writer hardening

Restore Pinapod 0.4.2 after verifying that its generated-code changes are intentional hardening rather than a runtime regression. Compact writers now validate their buffer before commit-time offset and pointer work, and a preflight/commit length disagreement returns `InvalidLength` in release builds instead of relying on a debug assertion.

The only Pina-side behavior change is the expected macro expansion. `update` keeps its existing commit-then-inline-write ordering because inline fields occupy the fixed-size header and cannot change the encoded length. `try_initialize` writes inline fields before commit-time validation so a valid non-zero representation, such as a one-based enum, is present when validation runs. Pina's complete compact account suite passes against the published 0.4.2 pair in both debug and release builds.
