---
pina: patch
---

# Link host binaries with Apple's toolchain on Darwin

Inside `devenv shell`, Rust linked host test binaries through the nix `cc`-wrapper chain, which ends at `cctools-binutils-darwin`'s `ld64` rather than Apple's linker. Binaries it produced could not initialise their unwinder, so every `panic!` aborted the whole test binary with `fatal runtime error: failed to initiate panic, error 5` — a failed assertion was indistinguishable from a crash, `abort_after_mutation` subprocess tests could not report, and the `lint:push` pre-push hook, which re-enters devenv, could never pass on macOS.

`devenv.nix` now sets `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER` and `CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER` to `/usr/bin/cc` alongside the existing `HOST_CC`/`HOST_CXX` overrides. The variables are target-scoped, so the `bpfel-unknown-none` SBF builds keep their `sbpf-linker` and the Kani profile is unaffected. With the fix, `cargo test --workspace` completes inside devenv, the pre-push hook passes, and `coverage:all` runs to completion locally.
