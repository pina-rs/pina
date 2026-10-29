---
pina_cli: fix
pina_cpi_renderer: fix
---

# Keep IDL Import Provenance Writes Inside the Generated Crate

Reject linked import destinations before rendering and write provenance through the renderer's pinned directory using no-follow reads and atomic replacement. External symlink and hard-link targets remain untouched.
