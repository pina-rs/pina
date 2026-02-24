# `anchor_duplicate_mutable_accounts`

Anchor parity example for duplicate mutable account checks.

## What it covers

- Explicit duplicate mutable account detection in program logic.
- Separate behavior for duplicate read-only accounts.
- Custom error mapping (`ConstraintDuplicateMutableAccount`).

## Run

```bash
cargo test -p anchor_duplicate_mutable_accounts
pina idl --path examples/anchor_duplicate_mutable_accounts --output codama/idls/anchor_duplicate_mutable_accounts.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_duplicate_mutable_accounts -Z build-std -F bpf-entrypoint
```
