---
pina: feat
---

# Re-export zeropod fixed-capacity collection types

Replace Pina's local collection implementation with zeropod's allocation-free `PodOption<T>`, `PodString<N, PFX>`, and `PodVec<T, N, PFX>` types. Account and instruction boundaries now use `ZcElem` and `ZcValidate` to enforce alignment, layout, length-prefix, element, and UTF-8 invariants before safe access.
