---
default: note
---

Keep the prebuilt `custom.sbpf-linker` package available on Linux CI, where BPF and binary-size jobs rely on it being on `PATH`, while disabling its Nix `installCheckPhase` on Darwin. The Darwin package currently crashes during `sbpf-linker --help` because its Mach-O load commands reference `/opt/homebrew/opt/llvm/lib/libLLVM.dylib`; skipping that install check unblocks local `devenv shell` without removing the Linux linker used by CI.
