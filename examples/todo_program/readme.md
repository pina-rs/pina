# `todo_program`

<br>

PDA-backed todo state program.

## What it covers

<br>

- Creating todo state accounts (`Initialize`).
- Toggling completion state (`ToggleCompleted`).
- Updating fixed-size digest data (`UpdateDigest`).

## Run

<br>

```bash
cd examples/todo_program
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p todo_program -Z build-std -F bpf-entrypoint
```
