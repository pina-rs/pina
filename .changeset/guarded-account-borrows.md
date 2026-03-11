---
pina: major
---

Keep typed account and token loaders borrow-guarded so runtime borrow checks remain active
for the lifetime of returned views. Add regression tests for overlapping typed borrows,
validate `taker_ata_b` in the escrow example, and enforce custom security dylints in the
standard lint/CI workflow.
