# Testing and SBF Builds

## Building for SBF

Programs are compiled with Agave's `cargo-build-sbf`, which owns its SBF target and linker toolchain.

Example:

```sh
cargo build-escrow-program
```

This expands to:

```sh
cargo build-sbf --manifest-path examples/escrow_program/Cargo.toml --sbf-out-dir target/deploy --features bpf-entrypoint
```

The `build-pina-bpf-program` alias uses the same `cargo-build-sbf` path for the standalone `pina_bpf_program` example.

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
