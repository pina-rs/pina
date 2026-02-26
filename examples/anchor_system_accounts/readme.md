# anchor_system_accounts

<br>

Pina parity port of Anchor's system-account ownership checks.

## What this demonstrates

<br>

- Signer validation for authorities.
- System-program ownership checks for wallet accounts.
- Minimal instruction dispatch for ownership constraints.

## Differences From Anchor

<br>

- Ownership and signer checks are explicit chained assertions on `AccountView`.
- The constraint logic is implemented directly in `ProcessAccountInfos`.
- Tests validate both acceptance and rejection paths for owner checks.

## Run

<br>

```sh
cargo test -p anchor_system_accounts
pina idl --path examples/anchor_system_accounts --output codama/idls/anchor_system_accounts.json
```
