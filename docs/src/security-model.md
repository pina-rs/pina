# Security Model

Pina's safety posture is built around explicit validation and predictable state transitions.

The [Security Lints](./security-lints.md) page maps these invariants to the compile-time checks applied to every repository example and secure security fixture.

## Core invariants

- Type correctness: account bytes must match expected discriminator and layout.
- Authority correctness: signer/owner checks must precede mutation.
- PDA correctness: seed and bump checks must gate PDA-bound operations.
- Value correctness: arithmetic and balance mutations must be checked.

See [ADR 0001](./adrs/0001-discriminator-first-layout.md), [ADR 0002](./adrs/0002-zero-copy-account-model.md), and [ADR 0003](./adrs/0003-guard-backed-typed-account-loaders.md) for the durable rationale behind these invariants.

## Version-safe binary layout and compatibility

The discriminator-first model makes byte layout part of protocol compatibility. Treat every `#[account]` struct as ABI:

- Do not reorder fields.
- Do not change existing discriminator values.
- Do not alter field types in-place without migration.
- If a struct grows, treat it as a new versioned shape and migrate state explicitly.

<!-- {=pinaDiscriminatorVersionCompatibility} -->

## Discriminators and ABI migrations

| Change                                                            | Compatibility impact                                                                      |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Add a new discriminator variant                                   | Backward-compatible; existing routes keep their identity                                  |
| Change an existing discriminator value                            | **Breaking** for every historical byte slice                                              |
| Change a migration-aware account or enveloped instruction payload | Compatible only when the checked-in history has an adjacent transition                    |
| Change a published versioned event schema                         | Compatible; a new version is appended and clients decode each version with its own schema |
| Change a published snapshot-only instruction payload              | **Breaking**; create a new instruction discriminator                                      |
| Append optional accounts to an instruction route                  | Compatible when the existing positional list remains an identical prefix                  |
| Reorder, remove, or escalate an instruction slot                  | **Breaking**; create a new instruction discriminator                                      |
| Change the migration version width after release                  | **Breaking** for every enveloped wire contract                                            |

Add `migrations` to an account, instruction, or event attribute to opt one contract into a framework-owned version field, or opt whole contract kinds in when you record the history:

```bash
pina migrations create --auto true # or --auto accounts,events,instructions
```

`--auto` accepts `true`, `false`, or a comma-separated list of `accounts`, `events`, and `instructions`. `pina migrations create` records the policy in `migrations/manifest.json`, its only home, and snapshots every contract of the listed kinds; a later run without the flag keeps the recorded policy. `pina.toml` holds no migration policy. Macros read the policy from the manifest, so a new struct still fails the build with "run `pina migrations create`" until it has a snapshot. Once a policy is recorded, `create` also scaffolds a `build.rs` emitting `cargo:rerun-if-changed=migrations/manifest.json`, so flipping the policy re-expands every contract without editing source. Add `migrations = false` to keep one contract out of an auto policy; removing an envelope the manifest already records is an error instead of a silent opt-out, because stripping an envelope is itself a wire-format change.

The policy gives accounts and events the version field. It records each instruction as a snapshot without one: the payload keeps its `[discriminator][payload]` wire format, and the build fails when the struct drifts from the snapshot. Add `migrations` to an `#[instruction]` to give that instruction the version field and adjacent transitions instead; the `#[discriminator(entrypoint)]` dispatcher then converts an older payload to the current layout before the handler runs.

Pina places the version field immediately after the discriminator. The accepted encodings are `u8`, `u16`, and `u32`; `u8` is the default and the recommended choice. Versions are tracked per contract, not per program: each account, instruction, and event owns an independent history that starts at version `0`, so `u8` gives every contract its own 255-version budget. Rewriting one contract 255 times is not a realistic outcome, and the narrower field costs one byte in every enveloped account. Choose a wider encoding with `pina migrations create --version-type u16` (or `u32`) before the first release only when you expect a single contract to exceed 255 versions. The width is program-wide, recorded as `versionType` in the manifest, and freezes at the first published release: after that the flag fails instead of widening it. Any other value, including `u64`, is rejected with an error naming the supported widths. Discriminator width is a separate setting, and that one does support `u64`.

Run `pina migrations create` before a release. Pina updates the replaceable draft when the current version is unpublished. After `pina deploy` records a non-local publication, the next schema change creates a new version, with an adjacent transition for an account or an enveloped instruction; the payload of a published snapshot-only instruction cannot change. Normal builds run `pina migrations check` and fail on drift, incomplete manual transitions, or changed published code.

An old instruction can omit only newly appended optional accounts. Pina does not synthesize signers, writable privileges, PDAs, or required accounts. Any change to an existing process slot requires a new discriminator.

Historical events are immutable, so Pina versions events instead of migrating them. The program emits only the current version. Generated clients decode each earlier version with its own schema, as a separate `<Event>V<n>` event, and a log record whose version no generated event describes fails instead of being misread.

<!-- {/pinaDiscriminatorVersionCompatibility} -->

## High-priority guardrails

- Prefer checked arithmetic (`checked_add`, `checked_sub`) for all user-facing or balance-affecting values.
- Ensure all token account types used by helper traits implement `AccountValidation`.
- Keep close/transfer helpers conservation-safe (no temporary double-crediting).

## Closing accounts safely

<!-- {=pinaCloseAccountGuidance} -->

Closing guidance under Pinocchio 0.11:

- `close_with_recipient(&ID, recipient)` verifies that `ID` owns the account, transfers its lamports, and closes the account handle. It does not zero or resize account data.
- When stale bytes must be invalidated, use `close_account_zeroed(&ID, recipient)` or `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()`. When the close must stay separate, clear the whole data buffer with `account.try_borrow_mut()?.fill(0);` before `close_with_recipient(&ID, recipient)`.
- The `account-resize` feature only affects realloc helpers; it does not change close semantics.

<!-- {/pinaCloseAccountGuidance} -->

## Best practices

<!-- {=pinaSecurityBestPractices} -->

- **Always call `assert_signer()`** before trusting authority accounts
- **Use Pina's token loaders directly** because they delegate canonical owner and layout validation to the corresponding checked upstream parser before returning typed state
- **Use `as_associated_token_account()`** when reading a canonical ATA because it validates the runtime owner, derived address, stored current authority, and stored mint together; enforce state, delegate, close-authority, and extension policy separately
- **Skip `assert_associated_token_address()` when the same account feeds an `associated_token_account::instructions::Create` or `CreateIdempotent` CPI**, because that program derives the identical seeds and returns `InvalidSeeds` on a mismatch; reserve the assertion for validation-only paths that never reach an ATA instruction
- **Create accounts through the typed creation builders**, which reject targets whose storage is not zeroed with `AccountAlreadyInitialized`
- **Use `invoke_with` or `invoke_signed_with`** when fixed-account creation must establish nonzero values before final PinaPod validation
- **Use generated `load_pda` or `load_pda_mut`** when a fixed stored-bump PDA handler needs a typed guard, so recursive content and the PDA address are validated once
- **Use generated `with_stored_bump_pda`** (the pre-split name `with_pda` still works but is deprecated) when a compact stored-bump PDA handler needs a compact view and the address is already established, so the layout and stored-bump PDA address are validated during the same borrow
- **Use generated `with_checked_pda` instead of `with_stored_bump_pda`** when an untrusted caller chooses which account the handler loads; it searches for the canonical bump and so rejects a shadow account created at a noncanonical bump, which a stored-bump check alone cannot detect
- **Validate dynamic program accounts** with Pina's `assert_program()` before explicitly unverified CPI invocations; static and self-verifying CPI APIs need no redundant assertion
- **Use `as_account::<T>()` or `as_account_mut::<T>()`** when a handler needs fixed-account fields; these guard-backed loaders check the owner, discriminator, exact size, and nested values
- **Reserve `assert_type::<T>()` for validation-only paths** that do not need typed fields, and never treat it as proof for a later raw cast
- **Use `send_owned(&ID, amount, recipient)`** for direct lamport debits; it verifies that the program owns the sender before mutation
- **Use `close_account_zeroed(&ID, recipient)` or `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()`** when stale account bytes must be invalidated before close; when the close must stay separate, clear the whole buffer with `account.try_borrow_mut()?.fill(0);` before `close_with_recipient(&ID, recipient)`
- **Use `CreateProgramAccount` or `CreateCompactProgramAccount` for canonical PDA creation**; their explicit-bump variants also reject noncanonical bumps without separate seed assertions
- **Reserve `CreateProgramAccountWithUncheckedBump` for seed namespaces that already bind uniqueness**; it checks the supplied bump derives the account's address but does not prove it canonical
- **Keep `assert_seeds()` / `assert_canonical_bump()` for validation-only paths** that are not immediately followed by a checked creation builder
- **Give each account type its own seed namespace** so PDAs cannot collide across account types

<!-- {/pinaSecurityBestPractices} -->

## Stored-bump PDA verification

`load_pda`, `load_pda_mut`, and `with_stored_bump_pda` re-derive the account's address from the bump stored in its data with `sha256` (`pina::is_derived_address`) instead of the `sol_create_program_address` syscall, which saves about 1,350 compute units per load. The syscall adds one check the hash does not: that the result lies off the ed25519 curve. That check is redundant for any account these loaders accept.

- **Checks run in order.** Each loader first requires the program to own the account and its data to pass the type's discriminator and layout checks, so the program itself initialized the account.
- **Only a valid program address can get that far.** The program can only create an account at a seed-derived address through `invoke_signed`, and the runtime signs only for an off-curve address. Pina's creation builders cannot adopt an account someone else assigned to the program, because the system program's `allocate` and `assign` refuse an account the system program does not own. So the stored bump already derived a valid program address.
- **The skipped check adds nothing here.** The only address it would additionally reject is an on-curve address equal to the hash, for which no one can derive a private key.
- **Seed lengths are still checked.** `sha256` concatenates the seeds, so a 33-byte seed hashes exactly like a 32-byte seed followed by a 1-byte one, which can be a valid program address. `create_program_address` rejects any seed longer than `MAX_SEED_LEN`, and `is_derived_address` does the same before hashing. A seed of constant or fixed-size length, which every generated loader passes, folds the check away.

The PDA creation builders check the target address the same way before the create-account CPI. There the runtime performs the curve check itself: the allocation signs for the address through `invoke_signed` with the same seeds and bump, and the runtime refuses to sign for an on-curve address, failing the instruction with "Could not create program address with signer seeds". `counter_program`'s Surfpool suite proves it with a bump whose derived address is on the curve: the program's check accepts that address, the runtime rejects the signature, and no account is created. The builders still reject a seed longer than `MAX_SEED_LEN` themselves, with `InvalidSeeds`, as they did when they called `create_program_address`.

Keep `create_program_address` (through `assert_seeds_with_bump`) for a bump a caller supplies when no `invoke_signed` follows, and the canonical loaders and builders (`load_checked_pda`, `load_checked_pda_mut`, `with_checked_pda`, `CreateProgramAccount`) when the bump must be the highest valid one. Both still check the curve.

## Content validation with PinaPod

Pina's zero-copy account model is built on PinaPod. Its generated storage view makes validation load-bearing: `PinaAccount::try_from_bytes` and `as_account` reject noncanonical booleans, invalid UTF-8, overlength vector prefixes, invalid option tags, invalid active nested values, and invalid enum discriminants before returning a reference.

The `#[account]` macro uses the native struct only as a schema and derives `PinaPod`. For `Account`, PinaPod generates `AccountZc`; loaders return that companion, not a reference to the native schema. `PinaAccount::validate_account_data` checks the discriminator, exact size, and every fixed field. Compact loaders also check every tail offset and active length.

### Unit enums

Unit enums with explicit discriminants can derive `PinaPod`. PinaPod generates an `EnumZc` companion that stores raw bytes and validates the discriminant before converting it to the native enum. Pina's audited `#[account]` grammar does not accept arbitrary custom enums, so this form applies to direct PinaPod schemas and advanced manual `PinaAccount` implementations.

### Inactive capacity

PinaPod initializes the full capacity of fixed strings, vectors, and options. Shortening or clearing a value also zeroes the removed payload, so stale application data does not remain in inactive capacity. Pina still exposes validated field accessors rather than a byte slice over an in-memory schema or storage view.

Compact patches clear bytes removed by a tail replacement. Typed creation and update builders accept the account's exact generated patch through `PinaCompactPatch`; an unrelated `PinaPodPatch` implementation cannot bypass that boundary. `UpdateResizableAccount` preflights the structural patch and target size before moving rent or changing account bytes. A failed preflight leaves both data and lamports unchanged. With the `validation` feature, application rules run on the completed representation after the patch is written. Propagate every update error so Solana rolls back the bytes and any earlier rent movement.

## Testing strategy

- Unit tests for negative validation cases.
- Regression tests for every previously fixed bug class.
- Integration tests for cross-account invariants where mutation order matters.

These framework guarantees do not validate an application's economic design. Use the [Production Readiness](./production-readiness.md) gate before deploying an asset-bearing program.
