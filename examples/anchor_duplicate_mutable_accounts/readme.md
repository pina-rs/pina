# anchor_duplicate_mutable_accounts

<br>

Pina parity port of Anchor's duplicate mutable account checks.

## What this demonstrates

<br>

- Detecting duplicate mutable accounts.
- Returning deterministic custom error codes.
- Allowing duplicate readonly accounts where appropriate.

## Differences From Anchor

<br>

- Duplicate mutable detection is explicit (`ensure_distinct`) instead of implicit parser behavior.
- Validation is coded per instruction path so allowed/disallowed cases are clear.
- Error mapping uses `#[error]` enums and explicit `ProgramError::Custom` values.

## Run

<br>

```sh
cargo test -p anchor_duplicate_mutable_accounts
pina idl --path examples/anchor_duplicate_mutable_accounts --output codama/idls/anchor_duplicate_mutable_accounts.json
```
