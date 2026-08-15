---
pina: docs
---

Improve documentation for guard-backed account access, cursor safety, inline_always allowance, and the log macro.

- **A1**: Add a "Guard lifetime" section to `AsAccount` showing that the `Ref`/`RefMut` guard must be held across the full use site; documents the match-arm reborrow pitfall.
- **A2**: Document that `AccountsCursor::peek` bypasses `track_mutable_account` and that `next_mut` is the only safe path for mutable access; add a doc header to `track_mutable_account` itself.
- **H2**: Annotate `#![allow(clippy::inline_always)]` with a rationale comment explaining CU optimization for on-chain programs.
- **H3**: Move the `log!` format-arg limitation into a dedicated `# Limitations` doc section.