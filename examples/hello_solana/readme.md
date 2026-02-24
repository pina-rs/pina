# `hello_solana`

Minimal Pina program example.

## What it covers

- Basic instruction discriminator and parsing flow.
- `#[derive(Accounts)]` for account extraction.
- Signer validation plus on-chain log output.

## Run

```bash
cargo test -p hello_solana
pina idl --path examples/hello_solana --output codama/idls/hello_solana.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p hello_solana -Z build-std -F bpf-entrypoint
```
