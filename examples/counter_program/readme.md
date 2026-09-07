# `counter_program`

<br>

PDA-backed counter program.

## What it covers

<br>

- PDA-seeded accounts with `#[account]` / `#[pda]`.
- Atomic fixed-account initialization with `CreateProgramAccountWithBump::invoke_with`.
- Counter state mutation (`Initialize`, `Increment`).
- One-pass `CounterState::load_pda_mut` validation for the typed state, stored bump, and PDA address.

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
