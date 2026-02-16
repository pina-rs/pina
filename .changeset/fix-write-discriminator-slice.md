---
pina: patch
---

Fixed `write_discriminator` to correctly slice the destination buffer to `Self::BYTES` before copying. Previously, if the destination buffer was larger than the discriminator size, `copy_from_slice` would panic due to length mismatch.
