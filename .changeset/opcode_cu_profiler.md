---
default: minor
pina_profile: minor
---

Implement opcode-aware CU cost model for the static profiler.

The profiler now decodes each 8-byte SBF instruction's opcode and assigns costs based on the instruction class:

- Regular instructions (ALU, memory, branch): 1 CU each
- Syscall instructions (`call imm` with `src_reg=0`): 100 CU each

Per-function profiles now include `syscall_count` and the text output shows a Syscall column. The JSON output includes `total_syscalls` and per-function `syscall_count`.

This replaces the previous flat 1-CU-per-instruction model which could underestimate programs with heavy syscall usage by 10-100x.
