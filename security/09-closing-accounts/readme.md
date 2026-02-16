# 09: Closing Accounts

## The Vulnerability

Closing a Solana account requires more than just zeroing its lamport balance. If the account data isn't zeroed and the account isn't properly closed, a within-transaction attacker can revive the account by adding lamports back before the runtime garbage-collects it. The stale data remains intact and can be reused.

## Insecure Example

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program transfers lamports to the recipient but doesn't zero the account data or properly close the account. The account can be revived within the same transaction.

## Why This Is Dangerous

An attacker can:

- Revive a "closed" account and reuse its stale data
- Claim rewards or perform actions that should only happen once
- Bypass close guards by restoring lamports before the runtime GC pass

## Secure Example

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `zeroed()` on the account data to clear it, then calls `close_with_recipient()` which zeros lamports, resizes to 0, and closes the account.

## Pina API Reference

- `CloseAccountWithRecipient::close_with_recipient()` — transfers lamports to recipient, resizes data to 0, and closes the account
- Account data `zeroed()` method — zeros all account data before closing to prevent stale data reuse
