# `transfer_sol`

<br>

Two SOL transfer patterns: CPI and direct lamport mutation.

## What it covers

<br>

- CPI transfer through the system program (`CpiTransfer`).
- Direct lamport mutation for program-owned accounts (`DirectTransfer`).
- Custom error handling for insufficient funds.

## Run

<br>

```bash
cd examples/transfer_sol
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p transfer_sol -Z build-std -F bpf-entrypoint
```
