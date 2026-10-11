---
pina_cli: feat
pina_profile: feat
---

# Add the experimental upstream sbpf-linker toolchain

`sbpf-linker` 0.2.3 is pinned in `[workspace.metadata.bin]`, and the `[target.bpfel-unknown-none]` rustflags in `.cargo/config.toml` now carry the static-syscall configuration the linker requires: without the two `--cfg` flags the pinocchio syscall externs fall back to relocation-based resolution that sbpf-linker mishandles, producing programs that load but silently corrupt data.

A `sbpf-linker = true` key under `[project]` in pina.toml routes every `pina build` through the upstream recipe (nightly `bpfel-unknown-none` with `-Z build-std`, static syscalls, LTO forced off) and retries with `--export=<symbol>` flags when the linker reports missing libcall symbols such as u128 division's `__udivti3`; removing the key reverts to the `cargo-build-sbf` driver. `pina init --sbpf-linker` records the key and scaffolds the matching `.cargo/config.toml`; `PINA_SBPF_LINKER` points the build at a specific sbpf-linker binary, for example the repository pin.

`examples/upstream_bpf_program` showcases the toolchain with a Surfpool suite that functionally gates both build paths, and `docs/sbpf-linker-evaluation.md` records the measured caveats and the comparison matrix against `cargo build-sbf`: binaries 6-31% smaller, runtime compute units at parity, fat LTO a measured regression (17-77%), u128 division needing export flags, and large entrypoint frames (the multisig example) not building upstream at all.

`pina profile` now also reads section-less program-header-only ELFs, the artifact layout sbpf-linker emits for static-syscall programs, so static compute-unit profiling works for both toolchains.
