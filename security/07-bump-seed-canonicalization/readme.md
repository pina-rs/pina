# 07: Bump Seed Canonicalization

## The Vulnerability

Program Derived Addresses (PDAs) are derived with a "bump seed" — the runtime tries bump values from 255 down to 0 and returns the first one that produces a valid PDA. This first-found bump is the "canonical" bump.

If a program accepts any user-provided bump without verifying it's canonical, an attacker can derive a different (non-canonical) PDA from the same seeds and create a parallel state account that the program treats as legitimate.

## Insecure Example

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The program accepts a user-provided bump and uses `assert_seeds_with_bump` without verifying canonicality. An attacker can use a non-canonical bump to create duplicate state.

## Why This Is Dangerous

An attacker can:

- Create multiple PDA accounts from the same logical seeds
- Bypass uniqueness invariants (e.g. one config per user)
- Create conflicting state that the program cannot distinguish

## Secure Example

See [`secure/src/lib.rs`](secure/src/lib.rs). The program uses `assert_seeds()` which internally calls `try_find_program_address` to find and verify the canonical bump.

## Pina API Reference

- `AccountInfoValidation::assert_seeds()` — finds the canonical bump via `try_find_program_address` and verifies the address matches
- `AccountInfoValidation::assert_canonical_bump()` — same as `assert_seeds()` but also returns the canonical bump value
- `AccountInfoValidation::assert_seeds_with_bump()` — accepts any bump; use only when you've stored and verified the bump yourself
