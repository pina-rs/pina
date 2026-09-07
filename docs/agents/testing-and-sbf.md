# Testing and SBF Builds

## Building for SBF

Programs are compiled to `bpfel-unknown-none` using `sbpf-linker`.

Example:

```sh
cargo build-escrow-program
```

This expands to:

```sh
cargo build-sbf --manifest-path examples/escrow_program/Cargo.toml --sbf-out-dir target/deploy --features bpf-entrypoint
```

Linker flags and the `bpfel-unknown-none` target are configured in `.cargo/config.toml`. The `build-bpf` alias builds the standalone `pina_bpf` crate with `build-std` instead.

## `bpf-entrypoint` feature

The `bpf-entrypoint` feature separates:

- the on-chain entrypoint used for SBF builds
- the library code used in tests

## Testing Solana programs

Use `mollusk-svm` for Solana VM simulation in tests.

Programs are typically tested as regular Rust libraries without the `bpf-entrypoint` feature.

## See also

- [Build and tooling](./build-and-tooling.md) for the full toolchain and Kani/Miri setup.
- [CI and releases](../src/ci-and-releases.md) for every verification lane CI runs (feature matrix, fuzz, mutants, IDL drift).

The workspace `solana-*` crates (`solana-account`, `solana-instruction`, `solana-pubkey`, etc.) are available as dev-dependencies for building host-side fixtures.
