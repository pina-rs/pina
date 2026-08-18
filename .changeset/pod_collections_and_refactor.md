---
pina: feat
---

# Re-export zeropod fixed-capacity collection types

Replace Pina's local collection implementation with zeropod's allocation-free `Option<T>`, `String<N>`, and `Vec<T, N>` schema types. Zeropod generates the corresponding storage fields and validates tags, length prefixes, active elements, and UTF-8 before Pina returns a zero-copy view. Inactive capacity is not exposed as bytes.
