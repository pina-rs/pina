---
pina: docs
---

Improve documentation for guard-backed account access, cursor safety, inline_always allowance, and the log macro.

- **A1**: Add a "Guard lifetime" section to `AsAccount` explaining that `Ref`/`RefMut` guards block incompatible borrows while alive and should be dropped before later mutable access or CPIs.
- **A2**: Document that `AccountsCursor::next_mut` rejects aliases for individually parsed mutable fields, while `peek` performs no validation and `remaining_mut` deliberately preserves trailing-account aliases.
- **H2**: Annotate `#![allow(clippy::inline_always)]` with a rationale comment explaining CU optimization for on-chain programs.
- **H3**: Move the `log!` format-arg limitation into a dedicated `# Limitations` doc section.
