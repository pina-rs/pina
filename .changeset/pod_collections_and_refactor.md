---
pina_pod_primitives: minor
---

Add `PodOption<T>`, `PodString<N, PFX>`, and `PodVec<T, N, PFX>` fixed-capacity collection types for zero-copy Solana account layouts. Split the monolithic `lib.rs` into a multi-file module structure for maintainability. Add kani proof harnesses for collection types.
