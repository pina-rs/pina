---
pina_cli: feat
pina_skill: feat
---

# Explain failed transactions with `pina explain`

A failed Pina check returns a bare error code, and several checks share one: `writable`, `address`, `executable`, `data_len`, and `distinct_from` all return `InvalidAccountData`. A distinct code per check would cost every program size and compute units, so `pina explain` reconstructs the failure off-chain instead:

```sh
pina explain <SIGNATURE>
pina explain <SIGNATURE> --network devnet --json
pina explain --transaction-file ./failed.json
```

The command reads the transaction's account flags, error, and logs and matches them against the program's `#[derive(Accounts)]` structs and processors. It names the failing instruction, decodes the error (built-in, `PinaProgramError`, or the program's `#[error]` variant with its documentation), and ranks the checks that could have returned it, each with its field, rule, `path:line`, and a confidence class: `confirmed` from the transaction alone (signer and writable flags, account counts, duplicate mutable keys, `distinct_from`, known addresses), `checked_against_current_state` for owner, executable, and data checks read from current account state, or `possible` for checks that need runtime values. A check that runs after one the transaction proves failing is left out, and a candidate whose Pina log message appears is ranked first. The report also lists where the program constructs the error, the account table, and the failing instruction's logs, and it attributes a failure inside a CPI to the callee.

`--transaction-file` accepts a saved `getTransaction` result or JSON-RPC response and works offline. The default network is localnet; mainnet is queried only when named. `--rpc-url` rejects credentials, queries, and fragments, accepts plaintext HTTP only for loopback hosts, and redirects are never followed. Each run makes at most one `getTransaction` and one `getMultipleAccounts` request and retries neither. `--json` emits a versioned document with `schemaVersion: 1`.
