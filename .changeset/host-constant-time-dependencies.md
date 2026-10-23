---
pina_cli: fix
---

# Update constant-time dependency to patched ctutils

Refresh the host CLI Cargo lockfile to ctutils 0.4.4, fixing RUSTSEC-2026-0385 in constant-time mask generation and u128 comparisons. On-chain program dependencies are unchanged.
