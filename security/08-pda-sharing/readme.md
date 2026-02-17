# 08: PDA Sharing

## The Vulnerability

If two different account types use the same PDA seeds, they share the same address space. An attacker can create one type of account and then use it where the other type is expected. This is similar to type cosplay but at the PDA derivation level.

## Insecure Example

See [`insecure/src/lib.rs`](insecure/src/lib.rs). Both `UserConfig` and `UserVault` use the identical seeds `&[b"state", user_address]`, meaning they would derive to the same PDA.

## Why This Is Dangerous

An attacker can:

- Create one account type that has the same PDA as another
- Use an account in a context it wasn't intended for
- Bypass type-specific invariants by sharing PDAs across types

## Secure Example

See [`secure/src/lib.rs`](secure/src/lib.rs). Each account type uses type-specific seed prefixes: `&[b"config", user]` vs `&[b"vault", user]`. This ensures each type has its own PDA namespace.

## Pina API Reference

- Use distinct seed prefixes for each account type (e.g. `b"config"`, `b"vault"`)
- `AccountInfoValidation::assert_type::<T>()` — additional defense that checks the discriminator even if PDAs happen to collide
- `AccountInfoValidation::assert_seeds()` — verify the PDA matches the expected seed derivation
