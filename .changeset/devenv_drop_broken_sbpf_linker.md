---
default: note
---

Drop the prebuilt `custom.sbpf-linker` package from the devenv shell. Its Mach-O load commands referenced a Homebrew LLVM dylib (`/opt/homebrew/opt/llvm/lib/libLLVM.dylib`) that is not present on all machines, which crashed the Nix `installCheckPhase` and broke `devenv shell`. BPF builds continue to source `sbpf-linker` from the `install:sbpf-gallery` script, which builds it from source against the Blueshift `upstream-gallery-21` LLVM.
