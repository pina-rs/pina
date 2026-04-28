# 09: Closing Accounts

<br>

## The Vulnerability

<br>

Closing a Solana account requires more than just zeroing its lamport balance. If the account data isn't zeroed and the account isn't properly closed, a within-transaction attacker can revive the account by adding lamports back before the runtime garbage-collects it. The stale data remains intact and can be reused.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program transfers lamports to the recipient but doesn't zero the account data or properly close the account. The account can be revived within the same transaction.

## Why This Is Dangerous

<br>

An attacker can:

- Revive a "closed" account and reuse its stale data
- Claim rewards or perform actions that should only happen once
- Bypass close guards by restoring lamports before the runtime GC pass

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The program invalidates the account bytes before closing so a revived account cannot reuse stale state in the same transaction.

## Closing guidance

<br>

<!-- {=pinaCloseAccountGuidance} -->

Closing guidance under Pinocchio 0.11:

- `close_with_recipient()` transfers lamports and closes the account handle, but it does not zero or resize account data for you.
- When stale bytes must be invalidated, use `close_account_zeroed()` or manually call `zeroed()` before `close_with_recipient()`.
- The `account-resize` feature only affects realloc helpers; it does not change close semantics.

<!-- {/pinaCloseAccountGuidance} -->

## Pina API Reference

<br>

- `CloseAccountWithRecipient::close_with_recipient()` — close after you have already invalidated any sensitive or authority-bearing state
- `CloseAccountWithRecipient::close_account_zeroed()` — zero the current raw account bytes, then close and return rent to the recipient
- Account data `zeroed()` method — explicit typed/raw-state invalidation before `close_with_recipient()` when you need custom close sequencing
