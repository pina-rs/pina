---
default: note
pina: note
---

Re-enable the anchor parity BPF artifact checks in CI by building `sbpf-linker` with the Blueshift `upstream-gallery-21` LLVM toolchain.

This adds a cached `install:sbpf-gallery` devenv script and restores `cargo build-bpf` plus the ignored `pina_bpf` `bpf_build_` tests in `test:anchor-parity`.
