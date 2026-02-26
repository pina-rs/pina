# 06: Duplicate Mutable Accounts

<br>

## The Vulnerability

<br>

If a program expects two distinct writable accounts (e.g. source and destination) but doesn't check that they are different, an attacker can pass the same account for both. When the program debits one and credits the other, both operations hit the same account, potentially creating or destroying value.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program transfers value between source and destination without checking they are different accounts. Passing the same account as both source and dest could cause unexpected behavior.

## Why This Is Dangerous

<br>

An attacker can:

- Create value out of nothing if the program credits before debiting
- Cause integer overflow/underflow in balance calculations
- Bypass program invariants that assume distinct accounts

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The program checks `source.address() != dest.address()` before processing the transfer.

## Pina API Reference

<br>

- `AccountView::address()` — returns the account's address for comparison
- Compare addresses directly: `source.address() != dest.address()`
