---
pina_cli: fix
---

# Preserve concurrent publication updates

Acquire the migration lock before reading the manifest and publication ledger so reconciliation cannot overwrite a receipt or pending deployment committed by another command.
