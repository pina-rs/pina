---
pina: fix
pina_lints: fix
---

# prefer guard-backed fixed-account loading

Fixed-account size mismatches now return `InvalidAccountSize` from `assert_type`. Examples and documentation avoid redundant `assert_type` calls before `as_account*` or generated `load_pda*` methods. The raw zero-copy cast lint no longer treats a prior validation-only assertion as proof that an unrelated later cast is safe.
