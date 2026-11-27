---
pina: none
---

# Pin the 64-slot clamp in opt-mut aliasing tests

Add adversarial regression tests and host benchmark coverage pinning the 64-slot clamp in `validate_opt_mut_aliasing_64` against `wrapping_shl` wrap-around on slices with 65 or more accounts. Test-only; no version bump.
