---
pina: patch
---

`close_with_recipient()` now uses `checked_add` for lamport arithmetic instead of unchecked addition, returning `ProgramError::ArithmeticOverflow` on overflow. While overflow was practically impossible due to total lamport supply constraints, this follows the defensive pattern used in `send()` and prevents undefined behavior in edge cases.
