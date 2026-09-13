---
pina_cpi_renderer: feat
---

# Render signed integer instruction arguments in CPI clients

The CPI renderer rejected every signed integer argument with `unsupported argument format I64`, so an instruction declaring a signed `i64` value, such as a Unix timestamp, could not generate a CPI client at all: the whole program failed to render, not just that instruction.

Scalar arguments now map signed formats to their native Rust type and the same little-endian `to_le_bytes()` write already used for unsigned integers, matching the packed-vector path in the same module and the `pina_codama_renderer` and Dart renderer, which have always supported signed numbers. Floating-point and compact `shortU16` numbers remain rejected so the renderer never emits a different wire format than the IDL declares.
