---
pina: feat
pina_macros: feat
---

# Add a read-only versioned account view

Generated migratable accounts now expose `<Account>::try_from_bytes_versioned(bytes)`, which validates the exact stored representation named by the version envelope and borrows it immutably through the generated `<Account>Versioned` enum (one variant per historical version plus `Current`). The accessor never rewrites, resizes, clears, or requires a writable borrow, and it fails closed for foreign discriminators, unknown or future versions, and malformed representations. This is the read path for read-heavy accounts whose one-time writable touch is genuinely hard to schedule; callers must handle every stored representation explicitly, so migration remains the recommended fix whenever a writable touch is schedulable.
