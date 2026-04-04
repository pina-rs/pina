# `vesting_program`

<br>

Token vesting and lockup scaffold built with Pina.

## What it covers

<br>

- Vesting schedule initialization with a PDA-owned state account.
- Vault ATA creation for the schedule account.
- Claim and cancel flows with explicit validation chains.
- Token-program account validation and ATA scaffolding.

## Run

<br>

```bash
cargo test -p vesting_program
pina idl --path examples/vesting_program --output codama/idls/vesting_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p vesting_program -Z build-std -F bpf-entrypoint
```
