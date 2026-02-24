# `anchor_floats`

Anchor parity example for float account fields.

## What it covers

- Creating and updating account state with `f32`/`f64` values.
- Authority-gated updates.
- Bit-level conversion through `PodU32`/`PodU64`.

## Run

```bash
cargo test -p anchor_floats
pina idl --path examples/anchor_floats --output codama/idls/anchor_floats.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_floats -Z build-std -F bpf-entrypoint
```
