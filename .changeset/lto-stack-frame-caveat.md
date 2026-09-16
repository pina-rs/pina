---
pina_cli: docs
---

# Warn about the entrypoint stack limit for cdylib-only

`pina build` advises moving to `crate-type = ["cdylib"]` for the 20-30% that LTO is worth, and the program size guide presents it as an unconditional win. It is not: LTO inlines every instruction handler into the entrypoint, the SBF runtime allows 4 KB of stack per frame, and `cargo-build-sbf` exits 0 while writing the `.so` even when it reports the frame overflowed. A build that looks successful can produce a program that faults at runtime.

Both the warning and the size guide now say to check the entrypoint's stack frame after switching, and note the two ways out: keep the `lib` crate type, or raise the limit with `--sbf-stack-size`.
