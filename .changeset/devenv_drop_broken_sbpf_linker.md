---
default: note
---

Keep the prebuilt `custom.sbpf-linker` package available in `devenv` so BPF and binary-size jobs can find `sbpf-linker` on `PATH`, while disabling the package's Nix `installCheckPhase`. The upstream binary now requires linker inputs and `--output`, so invoking it with no arguments during the install check fails on Linux; on Darwin the same check also exposed a stale Homebrew LLVM load path. Skipping the install check unblocks `devenv shell` without removing the linker used by CI.
