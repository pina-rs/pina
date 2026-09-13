---
pina_cpi_renderer: fix
---

# Render signed integer instruction arguments

The CPI renderer rejected `i8`, `i16`, `i32`, `i64`, and `i128` instruction arguments with `unsupported argument format`, so a program whose instruction carried a signed value (a timestamp, delta, or offset) could not generate a CPI client at all, even though Pina maps those fields to `PodI8`-`PodI128` and the Dart renderer already rendered them.

Signed formats now render as their native Rust types and share the two's-complement little-endian write used by the unsigned formats, so a negative value encodes to the bytes the program decodes. Floating point and `shortU16` arguments remain unsupported, and that rejection now names the supported formats. The Anchor fixture used by the CLI compile test carries an `i64` argument so the converter, renderer, and generated crate stay covered end to end.
