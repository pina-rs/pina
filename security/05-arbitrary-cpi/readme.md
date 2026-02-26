# 05: Arbitrary CPI

<br>

## The Vulnerability

<br>

Cross-Program Invocations (CPI) execute instructions on other programs. If a program doesn't verify the address of the target program before invoking it, an attacker can substitute a malicious program. The malicious program can then perform arbitrary actions with the authority and accounts passed to the CPI.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program invokes whatever program the caller passes without verifying its address. An attacker can substitute a malicious program.

## Why This Is Dangerous

<br>

An attacker can:

- Replace the system program with a malicious program that steals funds
- Replace the token program with one that mints unlimited tokens
- Execute arbitrary logic with the signing authority of the calling program

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `assert_address(&system::ID)?` on the system program account before the CPI.

## Pina API Reference

<br>

- `AccountInfoValidation::assert_address()` — verifies exact address match
- `AccountInfoValidation::assert_program()` — verifies address + executable flag in one call
- `AccountInfoValidation::assert_addresses()` — verifies address is one of a known set (useful for token program compatibility)
