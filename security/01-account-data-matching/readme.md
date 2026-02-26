# 01: Account Data Matching

<br>

## The Vulnerability

<br>

A program may store references to other accounts (e.g. a maker's address in an escrow state). If the program doesn't verify that the accounts passed in the transaction match the values stored on-chain, an attacker can substitute different accounts and steal funds.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program reads the escrow state but doesn't verify that the `maker` account passed in the transaction matches the `maker` field stored in the escrow.

## Why This Is Dangerous

<br>

An attacker can:

- Pass their own account as the maker to receive funds intended for someone else
- Substitute a different mint to drain the vault with a worthless token

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). After deserializing the escrow state, the program calls `self.maker.assert_address(&escrow.maker)?` to verify the account matches.

## Pina API Reference

<br>

- `AccountInfoValidation::assert_address()` — verifies the account's address matches the expected value
- `AccountInfoValidation::assert_addresses()` — verifies the account's address is in a set of expected values
