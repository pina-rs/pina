# `staking_rewards_program`

<br>

Staking account and rewards-bookkeeping scaffold.

> **Not production-ready:** Deposit, withdraw, and claim update program-owned bookkeeping but do not transfer stake or reward tokens. Do not deploy or fork this example as a staking product without completing the custody and reward model described below.

## What it covers

<br>

- Pool initialization with stake and reward vault ATAs.
- Per-user position PDAs keyed by pool + owner.
- Deposit, withdraw, and claim validation and bookkeeping flows.
- Token-program validation and ATA creation (eager for pool vaults, idempotent for deposits).

The `tests/e2e.rs` file exercises the bookkeeping lifecycle through Mollusk. It does not assert token custody or reward payments. The tests report a skip when the SBF binary is missing, so build the binary first when using this suite as a deployment gate.

## Deliberately out of scope

- Transfers into and out of the PDA-controlled stake vault.
- Transfers from the reward vault to a claimant.
- Reward funding, solvency, emissions, time, precision, and settlement rules.
- Pause administration, position closure, pool shutdown, and recovery policy.

See the book's [Production Readiness](../../docs/src/production-readiness.md) checklist for the invariants and adversarial tests a real staking program needs.

## Run

<br>

```bash
cd examples/staking_rewards_program
pina test --unit
pina test
pina generate
```

The first command still runs useful native tests when the SBF artifact is absent; read its output and do not mistake a skipped E2E path for an executed program test.

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p staking_rewards_program -Z build-std -F bpf-entrypoint
```
