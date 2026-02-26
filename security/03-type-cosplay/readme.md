# 03: Type Cosplay

<br>

## The Vulnerability

<br>

If a program has multiple account types with similar sizes, an attacker can pass one account type where another is expected. Without discriminator validation, the program reinterprets the data as the wrong type, potentially granting unauthorized access.

For example, if a "User" account and an "Admin" account have the same byte size, an attacker could pass their User account where an Admin account is expected.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program deserializes account data using raw `bytemuck::try_from_bytes` without checking the discriminator. Any account of the right size is accepted.

## Why This Is Dangerous

<br>

An attacker can:

- Escalate privileges by passing a lower-privilege account as a higher-privilege one
- Bypass authorization by cosplaying as a different account type
- Corrupt state by writing to accounts with the wrong expected layout

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The program calls `assert_type::<T>(&ID)?` which checks the discriminator, owner, and data size before deserialization.

## Pina API Reference

<br>

- `AccountInfoValidation::assert_type::<T>(program_id)` — checks owner, discriminator, and data length in one call
- `AsAccount::as_account::<T>(program_id)` — checks owner and discriminator during deserialization
