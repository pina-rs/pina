---
pina: feat
pina_macros: feat
---

# Match the Migrate trigger to the discriminator width

`#[discriminator]` reserves the all-ones value of the enum's own primitive, so a `u16` program reserves `0xffff` and a `u32` program reserves `0xffff_ffff`. The generated entrypoint guarded the reserved path with `is_migrate_instruction`, which only tests one byte, so for any program whose instruction enum used `primitive = u16`, `u32`, or `u64` the reserved discriminator never matched and `process_migrate` was unreachable.

`pina` adds width-specific helpers alongside the existing one: `is_migrate_instruction_u16`, `is_migrate_instruction_u32`, and `is_migrate_instruction_u64`. Each matches only its own width, so a one-byte `0xff` is not a two-byte `0xffff` and neither prefix-matches the other. They compare the decoded little-endian value rather than a slice equality, keeping the inline comparison the one-byte helper already had instead of the `memcmp` call `data == CONSTANT.to_le_bytes()` would emit on SBF.

`pina_macros` selects the helper from the enum's `primitive`, so `#[discriminator(entrypoint, migrations(...))]` on a wider discriminator now reaches the reserved path. The generated guard for a default `u8` program is byte-identical to before, so the common case takes no compute regression. The documentation on the reserved-discriminator constants and in `docs/src/migrations/flow.md` now names the width pairing.
