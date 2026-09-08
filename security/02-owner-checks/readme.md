# 02: Owner Checks

<br>

## The Vulnerability

<br>

Every Solana account has an "owner" program. If a program doesn't verify account ownership before deserializing data, an attacker can create a fake account with the same data layout but owned by a different program. The program trusts the data and acts on it.

This is especially critical for token accounts. Pina's token loaders delegate account-view ownership and layout validation to the corresponding checked upstream parser. A caller can bypass that boundary by parsing detached bytes with an API that has no runtime account owner to inspect.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program calls Token-2022's `StateWithExtensions::from_bytes()`. That API validates the byte layout, but it cannot verify the owner because it receives only a byte slice. An attacker can provide matching token bytes in an account owned by another program.

## Why This Is Dangerous

<br>

An attacker can:

- Create a fake token account showing an inflated balance
- Bypass token program invariants (frozen accounts, authority checks)
- Drain the program by presenting spoofed token state

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `as_token_2022_account()`, which delegates owner and layout checks to Token-2022's checked account-view parser before it returns guard-backed typed data. Use `as_token_account_for_program()` when an instruction accepts either SPL Token or Token-2022.

## Pina API Reference

<br>

- `AccountInfoValidation::assert_owner()` — verifies the account is owned by the given program
- `AccountInfoValidation::assert_owners()` — verifies the account is owned by one of the given programs (useful for SPL Token + Token-2022 compatibility)
- `AsTokenAccount::as_token_account()` — validates the original SPL Token owner and account layout together through the checked upstream parser
- `AsTokenAccount::as_token_2022_account()` — validates Token-2022 ownership, layout, and extension storage together
- `AsTokenAccount::as_token_account_for_program()` — selects and validates either canonical token program
