# 02: Owner Checks

## The Vulnerability

Every Solana account has an "owner" program. If a program doesn't verify account ownership before deserializing data, an attacker can create a fake account with the same data layout but owned by a different program. The program trusts the data and acts on it.

This is especially critical for token accounts: the `as_token_account()` and `as_token_mint()` methods perform layout casts without owner checks.

## Insecure Example

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program calls `as_token_account()` without first verifying that the account is owned by the SPL Token program. An attacker can craft a fake account with arbitrary token data.

## Why This Is Dangerous

An attacker can:

- Create a fake token account showing an inflated balance
- Bypass token program invariants (frozen accounts, authority checks)
- Drain the program by presenting spoofed token state

## Secure Example

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `assert_owners(&SPL_PROGRAM_IDS)?` before any token deserialization.

## Pina API Reference

- `AccountInfoValidation::assert_owner()` — verifies the account is owned by the given program
- `AccountInfoValidation::assert_owners()` — verifies the account is owned by one of the given programs (useful for SPL Token + Token-2022 compatibility)
