# 00: Signer Authorization

<br>

## The Vulnerability

<br>

On Solana, any account can be included in a transaction's account list. Without an explicit signer check, an attacker can submit a transaction that names a victim's account as the "authority" without actually having the victim's private key. The program then trusts the account and performs privileged operations on the attacker's behalf.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The `process` function modifies the vault state without calling `assert_signer()` on the authority account. An attacker can pass any address as the authority and the program will accept it.

## Why This Is Dangerous

<br>

An attacker can:

- Withdraw funds from any user's vault
- Modify any user's on-chain state
- Impersonate any authority without holding the private key

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The authority account is validated with `self.authority.assert_signer()?` before any state mutation.

## Pina API Reference

<br>

- `AccountInfoValidation::assert_signer()` — verifies `is_signer()` returns `true`, or returns `MissingRequiredSignature`
