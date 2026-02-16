# 10: Sysvar Address Checking

## The Vulnerability

Solana sysvars (Rent, Clock, etc.) are special accounts at well-known addresses. If a program reads a sysvar account without verifying both its address and owner, an attacker can create a fake account with spoofed sysvar data. This can manipulate rent calculations, clock readings, or other sysvar-dependent logic.

## Insecure Example

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program reads the rent sysvar account without verifying its address or owner. An attacker can pass a fake account with manipulated rent data.

## Why This Is Dangerous

An attacker can:

- Manipulate rent calculations to avoid paying rent
- Spoof the Clock sysvar to bypass time-based locks
- Provide false slot numbers or epoch data

## Secure Example

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `assert_sysvar(&Rent::ID)?` which checks both the owner (Sysvar program) and the address.

## Pina API Reference

- `AccountInfoValidation::assert_sysvar(sysvar_id)` — checks both the owner (Sysvar1111111111111111111111111111111111111) and the address match the expected sysvar
