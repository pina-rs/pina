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
cd examples/counter_program
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p counter_program -Z build-std -F bpf-entrypoint
```
