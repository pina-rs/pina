---
pina_cli: fix
---

# Contain project initialization writes

Reject symbolic links, reparse points, and non-regular scaffold destinations, including dangling links, before initializing a project. Pin directory handles and use no-follow file opens to prevent scaffold writes through symbolic links, including when `--force` is enabled.
