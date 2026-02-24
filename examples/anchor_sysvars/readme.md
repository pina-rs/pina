# `anchor_sysvars`

Anchor parity example for sysvar account validation.

## What it covers

- Clock, rent, and stake-history sysvar address checks.
- Instruction dispatch and program-id mismatch checks.

## Run

```bash
cargo test -p anchor_sysvars
pina idl --path examples/anchor_sysvars --output codama/idls/anchor_sysvars.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_sysvars -Z build-std -F bpf-entrypoint
```
