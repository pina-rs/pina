# Security and Code Constraints

## Workspace-wide constraints

- `unsafe_code` is denied
- `unstable_features` is denied
- `clippy::correctness` is denied
- `clippy::wildcard_dependencies` is denied

## Error handling

- Do not use `Result::expect`.
- If a hard failure is required, prefer `unwrap_or_else` with an explicit panic message.

## Solana safety expectations

- When working with `AccountView`, prefer chained assertions like `assert_signer`, `assert_writable`, and `assert_owner` before state-dependent logic.
- On-chain code must remain `no_std` compatible where applicable.
