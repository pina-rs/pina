---
pina_cli: fix
---

# Contain project initialization writes

Reject symbolic links, reparse points, and non-regular scaffold destinations, including dangling links, before initializing a project. Pin directory handles and exclusively create scaffold files to prevent redirected writes. With `--force`, replace existing regular entries without modifying other hard links to the original files.
