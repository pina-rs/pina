---
pina: note
---

Migrate `examples/pinocchio_bpf_starter` to `examples/pina_bpf` and convert the program to the `pina` API surface.

- Replace the starter implementation with `declare_id!`, `#[discriminator]`, `#[instruction]`, `parse_instruction`, and `nostd_entrypoint!`.
- Add a dedicated README for the example with explicit nightly build instructions using `-Z build-std=core,alloc`.
- Update workspace wiring (`Cargo.toml`, cargo aliases, docs, and CI scripts) to use `pina_bpf`.
- Add additional host tests and ignored BPF artifact verification tests, and run those artifact checks in `test:anchor-parity`.
