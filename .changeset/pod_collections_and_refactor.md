---
pina: feat
---

# Add fixed-capacity Pod collection types

Add `PodOption<T>`, `PodString<N, PFX>`, and `PodVec<T, N, PFX>` fixed-capacity collection types for zero-copy Solana account layouts. Split the monolithic `lib.rs` into a multi-file module structure for maintainability. Add kani proof harnesses for collection types.

`PodString` deliberately does not implement `Deref<Target = str>` or `AsRef<str>`: because it is `bytemuck::Pod`, bytes loaded from untrusted account data may not be valid UTF-8, so string access is only available through the validating `try_as_str()` or the explicit unsafe `as_str_unchecked()`.
