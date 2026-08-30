# `escrow_program`

<br>

Token escrow program.

## What it covers

<br>

- Escrow lifecycle with `Make` and `Take` instructions.
- Vault PDA handling and seed validation.
- Token and Token-2022 program validation with ATA checks and the transfer flow.

## Run

<br>

```bash
cd examples/escrow_program
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p escrow_program -Z build-std -F bpf-entrypoint
```
