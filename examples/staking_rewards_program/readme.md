# `staking_rewards_program`

<br>

Staking pools with check-pointed reward accrual and reward release.

> **Review before deploying:** rewards accrue and release correctly, but the example still omits the emission policy a specific product needs (funding schedules, solvency guarantees, and shutdown rules). See the production-readiness checklist below.

## What it covers

<br>

- Pool initialization with stake and reward vault ATAs.
- Per-user position PDAs keyed by pool + owner.
- Deposit, withdraw, and claim validation and bookkeeping flows.
- Stake custody: `Deposit` transfers the deposited stake tokens into the pool's stake vault and `Withdraw` transfers the principal back out, so the vault balance always equals the pool's `total_staked`.
- Reward accrual through a monotone pool `reward_index` (rewards per staked token, scaled by `REWARD_INDEX_SCALE`) with a per-position `reward_debt` checkpoint. `SetRewardIndex` is the authority's drip and may only raise the index; deposit and withdraw bank the position's accrued rewards before the stake changes, so a new deposit cannot claim rewards earned before it arrived.
- Reward release: `Claim` transfers the accrued amount out of the pool's reward vault, signed by the pool PDA.

The `tests/surfpool` suite funds both vaults through real mints and asserts the balances around every step, including that a deposit from a wallet without stake tokens is refused, that the stake vault balance tracks `total_staked` through deposits and withdrawals, that a second claim without a new drip is refused, and that a regressed reward index is rejected. It reports a skip when the SBF binary is missing, so build the binary first when using this suite as a deployment gate.

## Deliberately out of scope

- Reward funding schedules, emissions curves, and solvency guarantees: the authority decides when and by how much the index moves, and a claim fails if the reward vault cannot cover it.
- Pause administration, position closure, pool shutdown, and recovery policy.

- Pool-creation policy: initialization is permissionless per `(stake_mint, reward_mint)` pair and the first initializer becomes the pool admin (who controls `SetRewardIndex`). If you announce a pool ahead of deploying, initialize it in the same transaction as the announcement lands, or front-run your own users — the first-initializer capture is the Audius init-front-run class.

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
