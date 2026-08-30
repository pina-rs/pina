# `hello_solana`

<br>

Minimal Pina program.

## What it covers

<br>

- Basic instruction discriminator and parsing flow.
- `#[derive(Accounts)]` for account extraction.
- Signer validation plus on-chain log output.

## Run

<br>

```bash
cd examples/hello_solana
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p hello_solana -Z build-std -F bpf-entrypoint
```
