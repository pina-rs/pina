---
pina: breaking
pina_lints: breaking
---

# prefer guard-backed fixed-account loading

Fixed-account size mismatches now return `InvalidAccountSize` from `assert_type`. Examples and documentation avoid redundant `assert_type` calls before `as_account*` or generated `load_pda*` methods. The deny-by-default raw zero-copy cast lint now rejects direct `bytemuck` account casts because a prior validation-only assertion cannot bind a later cast to the validated borrow.
