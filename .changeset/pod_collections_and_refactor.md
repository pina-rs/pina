---
pina: feat
---

# Re-export zeropod fixed-capacity collection types

Replace Pina's local collection implementation with re-exports of zeropod's allocation-free `PodOption`, `PodString`, and `PodVec` storage types for advanced direct zeropod integrations. Pina's macro-generated account, instruction, and event schemas deliberately reject string/vector collection fields because not every upstream construction path initializes inactive capacity. Macro schemas support semantic `Option<scalar>` fields through an exact audited `PodOption` mapping; use fully initialized fixed byte arrays plus checked helpers for bounded text and lists.
