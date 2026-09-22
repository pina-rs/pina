---
pina_cpi_renderer: none
---

# Derive the scaffold release-line test from the crate version

`scaffold_declares_pina_from_the_configured_source` computes the expected `pina` pin from `CARGO_PKG_VERSION` the same way `ScaffoldDependency::Published` renders it, so the assertion follows the crate's release line instead of holding a hard-coded `0.19` that the next version bump would leave behind.
