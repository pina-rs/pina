# 04: Initialization

<br>

## The Vulnerability

<br>

If a program doesn't check whether an account has already been initialized before writing initial state, an attacker can reinitialize accounts. This can reset balances, change authorities, or otherwise corrupt state.

The `#[account]` macro does **not** inject reinitialization protection automatically — you must explicitly call `assert_empty()` before creating accounts.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program creates and initializes an account without calling `assert_empty()` first. An attacker can call the initialize instruction again to overwrite existing state.

## Why This Is Dangerous

<br>

An attacker can:

- Reset an account's authority to their own address
- Reset balances or counters to zero
- Overwrite critical program state after it has been set

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `assert_empty()?.assert_writable()?` before creating the account, ensuring the account hasn't been initialized yet.

## Pina API Reference

<br>

- `AccountInfoValidation::assert_empty()` — verifies the account has no data, returns `AccountAlreadyInitialized` if non-empty
- `AccountInfoValidation::assert_not_empty()` — the inverse check for reading existing accounts
