# `anchor_declare_program`

Anchor parity example for `declare-program` behavior.

## What it covers

- Validating an external program account against an expected ID.
- Requiring the external program account to be executable.

## Run

```bash
cargo test -p anchor_declare_program
pina idl --path examples/anchor_declare_program --output codama/idls/anchor_declare_program.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p anchor_declare_program -Z build-std -F bpf-entrypoint
```
