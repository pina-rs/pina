---
pina_cli: fix
---

# Require the patched Tokio release

Require Tokio 1.53.3 or later to address RUSTSEC-2026-0386. Bound the deployment stdin regression test at the fake child's read so parallel Cargo discovery does not consume its timeout, while keeping the parent's stdin pipe open and draining command output.
