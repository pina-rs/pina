# `anchor_system_accounts`

Anchor parity example for system-owned account checks.

## What it covers

- Signer validation for authority accounts.
- Owner validation for system wallet accounts.

## Run

```bash
cargo test -p anchor_system_accounts
pina idl --path examples/anchor_system_accounts --output codama/idls/anchor_system_accounts.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_system_accounts -Z build-std -F bpf-entrypoint
```
