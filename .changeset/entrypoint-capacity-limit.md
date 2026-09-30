---
pina: fix
pina_macros: fix
---

# Size the entrypoint account array by each route's limit

`ENTRYPOINT_ACCOUNT_CAPACITY` was derived from each route's `ACCOUNT_BOUND`, which counts a `#[pina(remaining)]` slice as one slot. A route with one positional account and a trailing slice got a capacity of three, so an instruction sent five accounts ran with only two in the slice, and a writable account duplicated past the third slot was never compared against.

`ParseAccounts::ACCOUNT_LIMIT` now declares the most accounts a parser can accept: `#[derive(Accounts)]` counts positional and optional fields plus nested limits, and declares `UNBOUNDED` for a struct with a trailing slice, directly or through a nested group. Hand-written parsers default to `UNBOUNDED`. The capacity is one more than the largest limit, or the transaction maximum when any route is unbounded. `ACCOUNT_BOUND` and `MAX_INSTRUCTION_ACCOUNTS` keep counting declared slots.

The reserved `Migrate` route now reads only its declared slots and fails with `TooManyAccountKeys` when an account follows the last one, so its slot count is a limit too. Before, `MigrateContext` validated accounts past the slots only when the entrypoint array happened to hold them. Omitted trailing slots still migrate only the accounts that are present.
