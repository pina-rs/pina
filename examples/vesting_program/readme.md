# `vesting_program`

<br>

Vesting schedule with a cliff, linear unlock, token release, and cancellation refund.

> **Review before deploying:** the entitlement and custody model is complete, but the example still omits the policy a specific product needs (amendment, recovery, insolvency, and multi-schedule administration). See the production-readiness checklist below.

## What it covers

<br>

- Vesting schedule initialization with a PDA-owned state account.
- Vault ATA creation for the schedule account.
- Claim with a Clock-sysvar check: a claim before `cliff_ts` is rejected, and the vested entitlement grows linearly to `end_ts`, rounding down so the beneficiary can never be released more than the curve supports.
- Token release from the vault signed by the vesting PDA, with the vault and beneficiary balances asserted by the Surfpool suite.
- Cancel: refund of the unclaimed remainder to the admin and closure of the vault, so no value is stranded.

The `tests/surfpool` suite funds the vault through a real SPL mint and asserts the exact balances around every step, which is what proves the release and the refund actually move tokens. It reports a skip when the SBF binary is missing, so build the binary first when using this suite as a deployment gate.

## Deliberately out of scope

- Amendment, early termination, or re-issuance of an existing schedule.
- Recovery when the mint is frozen or the vault is short of the entitlement.
- Multiple concurrent schedules per `(admin, beneficiary, mint)` pair.

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
