---
pina: fix
---

# Enforce migration versions in `assert_type`

Make `assert_type` apply the account type's migration-envelope validation after checking its discriminator. Stale migration-backed accounts now fail with `MigrationRequired`, matching the behavior of the typed account loaders.
