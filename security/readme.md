# Pina Security Guide

<br>

This guide covers the most common Solana smart contract vulnerabilities and how pina mitigates them. Each category includes:

- A **readme** explaining the vulnerability and pina's mitigations
- An **insecure** example — a compiling program with the vulnerable pattern
- A **secure** example — the correct pina pattern

Based on the [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks) taxonomy.

## Categories

<br>

| #                                    | Attack                     | Key Pina Mitigation                                 |
| ------------------------------------ | -------------------------- | --------------------------------------------------- |
| [00](00-signer-authorization/)       | Signer Authorization       | `assert_signer()`                                   |
| [01](01-account-data-matching/)      | Account Data Matching      | `assert_address()` on deserialized fields           |
| [02](02-owner-checks/)               | Owner Checks               | `assert_owner()` / `assert_owners()`                |
| [03](03-type-cosplay/)               | Type Cosplay               | `assert_type::<T>()` (discriminator + owner + size) |
| [04](04-initialization/)             | Initialization             | `assert_empty()` before `create_program_account`    |
| [05](05-arbitrary-cpi/)              | Arbitrary CPI              | `assert_address()` / `assert_program()`             |
| [06](06-duplicate-mutable-accounts/) | Duplicate Mutable Accounts | Address inequality check                            |
| [07](07-bump-seed-canonicalization/) | Bump Seed Canonicalization | `assert_seeds()` / `assert_canonical_bump()`        |
| [08](08-pda-sharing/)                | PDA Sharing                | Namespaced seeds + `assert_type::<T>()`             |
| [09](09-closing-accounts/)           | Closing Accounts           | `zeroed()` + `close_with_recipient()`               |
| [10](10-sysvar-address-checking/)    | Sysvar Address Checking    | `assert_sysvar()`                                   |

## How to Use

<br>

Each **secure** crate is a workspace member and compiles with `cargo build`. Each **insecure** crate is excluded from the workspace but can be built independently:

```sh
cargo build --manifest-path security/00-signer-authorization/insecure/Cargo.toml
```

Read each category's readme for a detailed explanation of the vulnerability and how to avoid it.
