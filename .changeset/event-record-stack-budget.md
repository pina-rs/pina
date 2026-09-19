---
pina: feat
pina_macros: feat
---

# Reject event schemas that cannot fit the SBF stack

The generated `emit` function materializes a whole event record in one stack frame (`let mut record = [0u8; Self::SIZE];`) before calling into the runtime, so a large `#[event]` schema compiled cleanly and then exhausted the 4 KiB SBF stack at runtime — a self-inflicted denial of service with no compile-time signal.

`pina` gains `MAX_EVENT_RECORD_BYTES`, set to `4096 - 512`. The 512-byte reserve is headroom for the callee frame and the caller's own frame, the same reasoning `MAX_MIGRATION_WORKSPACE` already applies to the historical-normalization workspace. Because the constant is public, the bound a caller can read is the bound the macro enforces.

`pina_macros` emits a compile-time assertion in every `#[event]` expansion, so an oversized schema now fails the build with a message naming the struct and the remedy. A record exactly at the bound still compiles; the assertion is inclusive.
