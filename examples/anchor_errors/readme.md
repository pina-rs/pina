# `anchor_errors`

Anchor parity example for custom error behavior.

## What it covers

- Deterministic custom error code mapping.
- Guard helper behavior (`require_eq`, `require_neq`, `require_gt`, `require_gte`).
- Instruction variant to error-code parity checks.

## Run

```bash
cargo test -p anchor_errors
pina idl --path examples/anchor_errors --output codama/idls/anchor_errors.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_errors -Z build-std -F bpf-entrypoint
```
