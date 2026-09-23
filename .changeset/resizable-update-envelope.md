---
pina: fix
---

# Enforce the migration envelope in resizable updates

`UpdateResizableAccount` applied patches through a raw writer when the `validation` feature was off — the default feature set — so neither the discriminator nor `require_current_migration_version` was checked before bytes were written. A compact account holding a stale envelope (version `n`, layout `L_n`) could be patched as if its bytes were the current layout, silently corrupting state and leaving the stale version byte in place for the next read to replay.

Both the preflight and the write now route through the generated `T::updated_len`/`T::update`, which enforce the storage length, the discriminator, and the migration envelope before any byte changes, and write the current version afterwards. Only the application-level re-validation those methods append is feature-gated, so the check is now identical in every build. Behavior for current-envelope accounts is unchanged; 45 `cpi_helpers` tests pass with and without `validation`.
