---
pina: major
---

**BREAKING**: `assert_writable()` now returns `ProgramError::InvalidAccountData` instead of `ProgramError::MissingRequiredSignature`. The previous error type was misleading — a writability check is unrelated to signatures. Code that matches on `MissingRequiredSignature` from `assert_writable()` must update to match `InvalidAccountData`.
