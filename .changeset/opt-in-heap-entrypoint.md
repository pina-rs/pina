---
pina: feat
---

# Add an opt-in heap allocation entrypoint

`nostd_entrypoint!` installs `pinocchio::no_allocator!`, so a Pina program cannot use the heap at all: any dynamic allocation aborts at runtime. That stays the default, but programs that want allocation no longer have to hand-roll the entrypoint through Pina's `pinocchio` re-export.

`nostd_entrypoint_alloc!` is the same macro with one substitution — `pinocchio::default_allocator!` instead of `no_allocator!` — so a program that declares `extern crate alloc` can use `Box`, `Vec`, and the rest of `alloc` while keeping Pina's entrypoint wiring. `nostd_entrypoint!` and its expansion are unchanged; the heap path is purely opt-in and per program.

The macro's rustdoc documents the costs that `no_allocator!` currently makes impossible, because each is a failure mode an opted-in program can hit: the runtime grants a 32 KiB heap frame at 0 CU and anything larger needs a caller-sent `request_heap_frame` (multiple of 1024, at most 256 KiB, 8 CU per extra 32 KiB page); the bump allocator's `dealloc` is a no-op, so a transaction's peak heap use is the sum of every allocation it makes; an allocation the frame cannot satisfy aborts instead of returning a `ProgramError`; and the allocator stores its position in the first word of the region, so the usable bytes are the granted frame minus one word.

`heap_alloc_program` exercises the new macro end to end. Its mollusk suite holds the heap frame at a fixed size and shows a fill inside the frame completing, a fill past it aborting, and the same fill succeeding once the frame is raised. Its Surfpool suite runs the same artifact on a real runtime, where `Box` round-trips through the bump allocator. Three doc sites that described allocator-freeness as an unconditional Pina property now say "by default".
