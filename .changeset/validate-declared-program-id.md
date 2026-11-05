---
pina_cli: fix
---

# Validate declared program addresses

Reject invalid `declare_id!` values during IDL extraction so upgrade-only deployment cannot interpret a declared keypair path as a program address.
