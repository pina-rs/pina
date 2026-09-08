# 02: Owner Checks

<br>

## The Vulnerability

<br>

Every Solana account has an "owner" program. If a program doesn't verify account ownership before deserializing data, an attacker can create a fake account with the same data layout but owned by a different program. The program trusts the data and acts on it.

This is especially critical for token accounts. Pina's token loaders enforce canonical program ownership, but a caller can still bypass that boundary by invoking an upstream layout parser directly.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program calls the upstream token layout parser directly without first verifying that the account is owned by the SPL Token program. An attacker can craft a fake account with arbitrary token data.

## Why This Is Dangerous

<br>

An attacker can:

- Create a fake token account showing an inflated balance
- Bypass token program invariants (frozen accounts, authority checks)
- Drain the program by presenting spoofed token state

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `as_token_account()`, which checks the original SPL Token program owner before it returns typed data. Use `as_token_account_for_program()` when an instruction accepts either SPL Token or Token-2022.

## Pina API Reference

<br>

- `AccountInfoValidation::assert_owner()` — verifies the account is owned by the given program
- `AccountInfoValidation::assert_owners()` — verifies the account is owned by one of the given programs (useful for SPL Token + Token-2022 compatibility)
- `AsTokenAccount::as_token_account()` — validates the original SPL Token owner and account layout together
- `AsTokenAccount::as_token_account_for_program()` — selects and validates either canonical token program
