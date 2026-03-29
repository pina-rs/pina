# Testing and SBF Builds

## Building for SBF

Programs are compiled to `bpfel-unknown-none` using `sbpf-linker`.

Example:

```sh
cargo build-escrow-program
```

This expands to:

```sh
cargo build --release --target bpfel-unknown-none -p escrow_program -Z build-std -F bpf-entrypoint
```

Linker flags are configured in `.cargo/config.toml`.

## `bpf-entrypoint` feature

The `bpf-entrypoint` feature separates:

- the on-chain entrypoint used for SBF builds
- the library code used in tests

## Testing Solana programs

Use `mollusk-svm` for Solana VM simulation in tests.

Programs are typically tested as regular Rust libraries without the `bpf-entrypoint` feature.

Related utilities are available from `test_utils_solana`.
