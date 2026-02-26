# `escrow_program`

<br>

Token escrow reference program built with Pina.

## What it covers

<br>

- Escrow lifecycle with `Make` and `Take` instructions.
- Vault PDA handling and seed validation.
- Token account checks and transfer flow.

## Run

<br>

```bash
cargo test -p escrow_program
pina idl --path examples/escrow_program --output codama/idls/escrow_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p escrow_program -Z build-std -F bpf-entrypoint
```
