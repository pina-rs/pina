---
pina_cli: fix
pina_lints: docs
---

# Prevent Cargo environment from bypassing security lints

Pin the complete lint catalog and validated project lint levels through Cargo command-line configuration so forced or relative project environment entries cannot silently skip security linting. Apply the same policy to the repository's security lint gate.
