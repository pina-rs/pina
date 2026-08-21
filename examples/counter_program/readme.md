# `counter_program`

<br>

PDA-backed counter program.

## What it covers

<br>

- PDA-seeded accounts with `#[account]` / `#[pda]`.
- Counter state mutation (`Initialize`, `Increment`).
- Validation chains via `.assert_signer()?.assert_writable()?` and zeropod state views.

## Run

<br>

```bash
cargo test -p counter_program
pina idl --path examples/counter_program --output codama/idls/counter_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p counter_program -Z build-std -F bpf-entrypoint
```
