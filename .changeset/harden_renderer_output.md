---
pina_codama_renderer: fix
---

# Harden generated output boundaries

Reject unsafe names and literals before rendering, validate generated Rust before replacing existing output, and constrain cleanup to managed files within the requested destination.
