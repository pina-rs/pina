---
default: patch
pina: patch
---

Add 12 integration tests for `pina::introspection` module (previously 0% coverage).

Tests construct fake Instructions sysvar account data following the exact binary
layout that pinocchio's `Instructions` parser expects, then exercise each
introspection function end-to-end:

- `get_instruction_count`: single and multiple instructions
- `get_current_instruction_index`: correct index returned
- `assert_no_cpi`: passes for top-level, fails for CPI, checks correct index
- `has_instruction_before`: finds earlier programs, returns false when first
- `has_instruction_after`: finds later programs, returns false when last
- Instructions with account metas and data
- Wrong sysvar address rejection
