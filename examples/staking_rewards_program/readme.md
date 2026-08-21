# `staking_rewards_program`

<br>

Staking and rewards distribution scaffold.

## What it covers

<br>

- Pool initialization with stake and reward vault ATAs.
- Per-user position PDAs keyed by pool + owner.
- Deposit, withdraw, and claim bookkeeping flows.
- Token-program validation and ATA creation (eager for pool vaults, idempotent for deposits).

The `tests/e2e.rs` file exercises the staking lifecycle through Mollusk.

## Run

<br>

```bash
cargo test -p staking_rewards_program
pina idl --path examples/staking_rewards_program --output codama/idls/staking_rewards_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p staking_rewards_program -Z build-std -F bpf-entrypoint
```
