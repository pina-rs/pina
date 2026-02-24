# `anchor_realloc`

Anchor parity example for realloc safety constraints.

## What it covers

- Maximum permitted account data increase checks.
- Duplicate realloc target detection.
- Multi-account realloc validation patterns.

## Run

```bash
cargo test -p anchor_realloc
pina idl --path examples/anchor_realloc --output codama/idls/anchor_realloc.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_realloc -Z build-std -F bpf-entrypoint
```
