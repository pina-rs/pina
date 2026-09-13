---
pina_cli: none
---

# Clarify the supported migration version widths

The migration version envelope accepts `u8`, `u16`, and `u32` only. Documentation now separates that setting from discriminator width, which does support `u64`, and a configuration test pins the rejection of `version-type = "u64"` with an error naming the supported widths.
