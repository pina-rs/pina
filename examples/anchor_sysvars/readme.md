# anchor_sysvars

<br>

Pina parity port of Anchor's sysvar-address validation checks.

## What this demonstrates

<br>

- Verifying Clock, Rent, and Stake History sysvar addresses.
- Routing a dedicated sysvar-check instruction.
- Deterministic sysvar mismatch failures.

## Differences From Anchor

<br>

- Sysvar IDs are explicit constants in program code.
- Validation uses direct `assert_sysvar` checks instead of declarative constraints.
- Unit tests include direct checks for expected constant separation and parser behavior.

## Run

<br>

```sh
cargo test -p anchor_sysvars
pina idl --path examples/anchor_sysvars --output codama/idls/anchor_sysvars.json
```
