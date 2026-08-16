---
pina_macros: fix
---

Improve the `#[discriminator]` size-assertion error message: it now reports the primitive's byte width (e.g. `u128` (16 bytes)) and lists the supported primitives (`u8`, `u16`, `u32`, `u64`), instead of only naming the symbolic `MAX_DISCRIMINATOR_SPACE` constant.
