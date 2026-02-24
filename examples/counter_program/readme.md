# `counter_program`

Reference counter example built with Pina.

## What it covers

- PDA-backed account creation.
- Counter state mutation (`Initialize`, `Increment`).
- Account validation chains and zero-copy state layout.

## Run

```bash
cargo test -p counter_program
pina idl --path examples/counter_program --output codama/idls/counter_program.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p counter_program -Z build-std -F bpf-entrypoint
```
