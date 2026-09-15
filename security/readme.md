# Pina Security Guide

<br>

This guide covers the most common Solana smart contract vulnerabilities and how pina mitigates them. Each category includes:

- A **readme** explaining the vulnerability and pina's mitigations
- An **insecure** example — a compiling program with the vulnerable pattern
- A **secure** example — the correct pina pattern

Based on the [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks) taxonomy.

## Categories

<br>

| #                                    | Attack                     | Key Pina Mitigation                                               |
| ------------------------------------ | -------------------------- | ----------------------------------------------------------------- |
| [00](00-signer-authorization/)       | Signer Authorization       | `assert_signer()`                                                 |
| [01](01-account-data-matching/)      | Account Data Matching      | `assert_address()` on deserialized fields                         |
| [02](02-owner-checks/)               | Owner Checks               | `assert_owner()` / `assert_owners()`                              |
| [03](03-type-cosplay/)               | Type Cosplay               | Guard-backed `as_account::<T>()` typed loading                    |
| [04](04-initialization/)             | Initialization             | `assert_empty()` before `CreateProgramAccount`                    |
| [05](05-arbitrary-cpi/)              | Arbitrary CPI              | `assert_address()` / `assert_program()`                           |
| [06](06-duplicate-mutable-accounts/) | Duplicate Mutable Accounts | Address inequality check                                          |
| [07](07-bump-seed-canonicalization/) | Bump Seed Canonicalization | `assert_seeds()` / `assert_canonical_bump()`                      |
| [08](08-pda-sharing/)                | PDA Sharing                | Namespaced seeds + generated `load_pda*`                          |
| [09](09-closing-accounts/)           | Closing Accounts           | `close_account_zeroed()` or `zeroed()` + `close_with_recipient()` |
| [10](10-sysvar-address-checking/)    | Sysvar Address Checking    | `assert_sysvar()`                                                 |
| [11](11-admin-key-compromise/)       | Admin Key Compromise       | Guardian-gated pause, circuit breaker, two-phase rotation         |
| [12](12-oracle-integrity/)           | Oracle Integrity           | Feed pinning (`assert_address`) + Clock staleness bound           |

## How to Use

<br>

Each **secure** crate is a workspace member and compiles with `cargo build`. Each **insecure** crate is excluded from the workspace but can be built independently:

```sh
cargo build --manifest-path security/00-signer-authorization/insecure/Cargo.toml
```

Read each category's readme for a detailed explanation of the vulnerability and how to avoid it. Lessons 11 and 12 are grounded in real incidents from the last three years; see [incidents-2023-2026.md](incidents-2023-2026.md) for the full research.
