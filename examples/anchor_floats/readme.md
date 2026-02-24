# anchor_floats

Pina parity port of Anchor's float account-data patterns.

## What this demonstrates

- Float storage in account data using `PodU32`/`PodU64` bit patterns.
- Authority-gated updates.
- Account initialization and mutation flows.

## Differences From Anchor

- Float values are explicitly converted with `to_bits`/`from_bits` for `Pod` safety.
- Authority checks and update rules are explicit in `apply_update`.
- Account creation is performed with explicit `create_account` + type validation calls.

## Run

```sh
cargo test -p anchor_floats
pina idl --path examples/anchor_floats --output codama/idls/anchor_floats.json
```
