---
pina: feat
---

# Add a checked discriminator write

`IntoDiscriminator::write_discriminator` returns `()` and drops the write when the destination slice is shorter than the discriminator: a `debug_assert!` fires in debug builds, but a release build silently leaves the buffer untouched. Generated code always sizes its buffers from `Self::BYTES`, so this only bit hand-written implementations, where a missing discriminator produces an account that looks initialized but decodes as an unknown type.

The new `IntoDiscriminator::try_write_discriminator` reports the undersized buffer with `PinaProgramError::DataTooShort` (0xFFFF_FFFA, "Account or instruction data is shorter than the expected minimum") instead. That is the variant `HasMigrationVersion::write_le` and the migration decoders already return for a destination that cannot hold the value, so a caller sees one error for every short-buffer case.

The method has a default implementation that length-checks and then delegates to `write_discriminator`, so a manual implementer inherits the checked path without writing anything; the primitive implementations (`u8`, `u16`, `u32`, `u64`) and `into_discriminator!` override it to write through `get_mut` in one step. `write_discriminator` is unchanged, and its doc comment now points new manual implementers at the checked form.

Tests cover short buffers of every length up to `BYTES` (including a short inner slice of a long array), exact and oversized buffers, and that the checked write produces the same bytes as the unchecked one.
