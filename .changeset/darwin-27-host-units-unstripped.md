---
pina_cli: patch
---

# Build SBF programs on Darwin 27

`pina build` failed on macOS 27 for every program that depends on a proc macro, the scaffolded program included: rustc reported `can't find crate for <macro crate>` as soon as a dependency used a derive. A release build strips the build scripts and proc macros it compiles for the host, and on macOS rustc strips them with the llvm-objcopy bundled in its toolchain. Before LLVM 23 that tool leaves `__LINKEDIT` entries misaligned, and Darwin 27's dyld now refuses to load such a library ("mis-aligned LINKEDIT string pool"). Agave's platform-tools v1.54 ship LLVM 20, so every `cargo build-sbf` run was affected.

Both the `cargo-build-sbf` and the sbpf-linker builds now set `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false`, which keeps host units unstripped. Host units never reach the deployed program, so the published `.so` is unchanged; the only cost is a slightly larger target directory and a one-time rebuild of host dependencies.
