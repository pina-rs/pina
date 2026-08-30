# `vesting_program`

<br>

Vesting schedule-state and account-validation scaffold.

> **Not production-ready:** Claim and cancel mutate schedule bookkeeping but do not use the clock or transfer tokens. Do not deploy or fork this example as a vesting product without completing the entitlement and custody model described below.

## What it covers

<br>

- Vesting schedule initialization with a PDA-owned state account.
- Vault ATA creation for the schedule account.
- Claim and cancel bookkeeping with explicit validation chains.
- Token-program account validation and ATA scaffolding.

The `tests/e2e.rs` file exercises the schedule-state lifecycle through Mollusk. It does not prove cliff enforcement, vested-amount calculation, token release, or cancellation refunds. The tests report a skip when the SBF binary is missing, so build the binary first when using this suite as a deployment gate.

## Deliberately out of scope

- Clock-sysvar validation and cliff or linear-unlock calculations.
- Funding the vault and transferring vested tokens to the beneficiary.
- Returning or preserving tokens after cancellation.
- Rounding, amendment, closure, recovery, and insolvency policy.

See the book's [Production Readiness](../../docs/src/production-readiness.md) checklist for the invariants and adversarial tests a real vesting program needs.

## Run

<br>

```bash
cd examples/vesting_program
pina test --unit
pina test
pina generate
```

The first command still runs useful native tests when the SBF artifact is absent; read its output and do not mistake a skipped E2E path for an executed program test.

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p vesting_program -Z build-std -F bpf-entrypoint
```
