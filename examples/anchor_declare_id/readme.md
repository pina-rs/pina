# `anchor_declare_id`

Anchor parity example for the `declare-id` test case.

## What it covers

- Program ID verification through `parse_instruction`.
- Accepting matching `program_id` and rejecting mismatches.

## Run

```bash
cargo test -p anchor_declare_id
pina idl --path examples/anchor_declare_id --output codama/idls/anchor_declare_id.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_declare_id -Z build-std -F bpf-entrypoint
```
