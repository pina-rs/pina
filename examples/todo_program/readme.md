# `todo_program`

PDA-backed todo state example.

## What it covers

- Creating todo state accounts (`Initialize`).
- Toggling completion state (`ToggleCompleted`).
- Updating fixed-size digest data (`UpdateDigest`).

## Run

```bash
cargo test -p todo_program
pina idl --path examples/todo_program --output codama/idls/todo_program.json
```

## Optional SBF build

```bash
cargo build --release --target bpfel-unknown-none -p todo_program -Z build-std -F bpf-entrypoint
```
