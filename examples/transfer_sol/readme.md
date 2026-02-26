# `transfer_sol`

<br>

SOL transfer example showing two transfer patterns.

## What it covers

<br>

- CPI transfer through the system program (`CpiTransfer`).
- Direct lamport mutation for program-owned accounts (`DirectTransfer`).
- Custom error handling for insufficient funds.

## Run

<br>

```bash
cargo test -p transfer_sol
pina idl --path examples/transfer_sol --output codama/idls/transfer_sol.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p transfer_sol -Z build-std -F bpf-entrypoint
```
