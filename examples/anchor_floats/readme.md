# `anchor_floats`

<br>

Pina parity port of Anchor's float account-data patterns.

## What this demonstrates

<br>

- Float storage in account data via `u32`/`u64` bit-pattern fields with `to_bits`/`from_bits` conversion.
- Authority-gated updates.
- Account initialization and mutation flows.

## Differences From Anchor

<br>

- Float values are explicitly converted with `to_bits`/`from_bits` for `Pod` safety.
- Authority checks and update rules are explicit in `apply_update`.
- Account creation uses the explicit `CreateAccount` CPI builder plus type validation calls.

## Run

<br>

```bash
cd examples/anchor_floats
pina test --unit
pina test
pina generate
```
