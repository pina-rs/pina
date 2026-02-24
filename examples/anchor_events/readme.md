# `anchor_events`

Anchor parity example for event modeling.

## What it covers

- Event discriminator definitions.
- Deterministic event payload construction.
- Event serialization round-trip checks.

## Run

```bash
cargo test -p anchor_events
pina idl --path examples/anchor_events --output codama/idls/anchor_events.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_events -Z build-std -F bpf-entrypoint
```
