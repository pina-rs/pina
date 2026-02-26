# `todo_program`

<br>

PDA-backed todo state example.

## What it covers

<br>

- Creating todo state accounts (`Initialize`).
- Toggling completion state (`ToggleCompleted`).
- Updating fixed-size digest data (`UpdateDigest`).

## Run

<br>

```bash
cargo test -p todo_program
pina idl --path examples/todo_program --output codama/idls/todo_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p todo_program -Z build-std -F bpf-entrypoint
```
