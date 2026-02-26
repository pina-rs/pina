# pina_bpf

<br>

A minimal BPF-targeted example migrated from `pinocchio-bpf-starter` to `pina`.

## What Changed

<br>

Compared to the original pinocchio starter style:

- Uses `pina` macros and types (`declare_id!`, `#[discriminator]`, `#[instruction]`, `parse_instruction`, `nostd_entrypoint!`).
- Separates the on-chain entrypoint behind a `bpf-entrypoint` feature.
- Includes host unit tests for instruction parsing and process logic.
- Includes ignored BPF integration tests that validate:
  - the BPF artifact exists after build
  - the generated artifact is a valid ELF binary

## Build Requirements

<br>

This example must be built with nightly Rust and `build-std` for `core,alloc`.

```bash
rustup component add rust-src --toolchain nightly-2025-10-15
cargo +nightly-2025-10-15 build --release \
  --target bpfel-unknown-none \
  -p pina_bpf \
  -F bpf-entrypoint \
  -Z build-std=core,alloc
```

The workspace provides an alias for the same command:

```bash
cargo +nightly-2025-10-15 build-bpf
```

## Tests

<br>

Run normal unit tests:

```bash
cargo test -p pina_bpf
```

Run BPF artifact verification tests after building:

```bash
cargo test -p pina_bpf bpf_build_ -- --ignored
```
