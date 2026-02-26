# anchor_realloc

<br>

Pina parity port of Anchor's account reallocation safety checks.

## What this demonstrates

<br>

- Reallocation growth-limit enforcement.
- Duplicate realloc target detection.
- Controlled resize flows across one or multiple accounts.

## Differences From Anchor

<br>

- Realloc limits are explicit constants (`MAX_PERMITTED_DATA_INCREASE`) and helper guards.
- Duplicate-account prevention is explicit (`validate_distinct_realloc_targets`).
- Resizing is done with direct `AccountView::resize` calls after validations.

## Run

<br>

```sh
cargo test -p anchor_realloc
pina idl --path examples/anchor_realloc --output codama/idls/anchor_realloc.json
```
