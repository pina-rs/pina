---
pina_cli: fix
---

# Preserve wide discriminator encodings

Parse the complete discriminator attribute grammar when generating Codama IDLs and preserve `u16`, `u32`, and `u64` discriminator widths instead of silently lowering them to `u8`.
