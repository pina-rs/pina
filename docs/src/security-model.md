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

| Change                                           | Compatibility impact                                                     |
| ------------------------------------------------ | ------------------------------------------------------------------------ |
| Add a new discriminator variant                  | Backward-compatible; existing routes keep their identity                 |
| Change an existing discriminator value           | **Breaking** for every historical byte slice                             |
| Change a migration-aware account or payload      | Compatible only when the checked-in history has an adjacent transition   |
| Append optional accounts to an instruction route | Compatible when the existing positional list remains an identical prefix |
| Reorder, remove, or escalate an instruction slot | **Breaking**; create a new instruction discriminator                     |
| Change the migration version width after release | **Breaking** for every migration-aware wire contract                     |

Add `migrations` to an account, instruction, or event attribute to opt into a framework-owned version field. Pina places the field immediately after the discriminator. Set its width once for the program:

```toml
[migrations]
version-type = "u8"
```

Run `pina migrations make` before a release. Pina updates the replaceable draft when the current version is unpublished. After `pina deploy` records a non-local publication, the next schema change creates a new version and adjacent transition. Normal builds run `pina migrations check` and fail on drift, incomplete manual transitions, or changed published code.

An old instruction can omit only newly appended optional accounts. Pina does not synthesize signers, writable privileges, PDAs, or required accounts. Any change to an existing process slot requires a new discriminator.

Historical events are immutable. Generated event decoders validate their exact released shape, project them into current-shape scratch bytes, and retain the source version so consumers can distinguish an absent historical field from an emitted default value.

<!-- {/pinaDiscriminatorVersionCompatibility} -->

## High-priority guardrails

- Prefer checked arithmetic (`checked_add`, `checked_sub`) for all user-facing or balance-affecting values.
- Ensure all token account types used by helper traits implement `AccountValidation`.
- Keep close/transfer helpers conservation-safe (no temporary double-crediting).

## Closing accounts safely

<!-- {=pinaCloseAccountGuidance} -->

Closing guidance under Pinocchio 0.11:

- `close_with_recipient(&ID, recipient)` verifies that `ID` owns the account, transfers its lamports, and closes the account handle. It does not zero or resize account data.
- When stale bytes must be invalidated, use `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()` or manually call `zeroed()` before `close_with_recipient(&ID, recipient)`.
- The `account-resize` feature only affects realloc helpers; it does not change close semantics.

<!-- {/pinaCloseAccountGuidance} -->

## Best practices

<!-- {=pinaSecurityBestPractices} -->

- **Always call `assert_signer()`** before trusting authority accounts
- **Use Pina's token loaders directly** because they delegate canonical owner and layout validation to the corresponding checked upstream parser before returning typed state
- **Use `as_associated_token_account()`** when reading a canonical ATA because it validates the runtime owner, derived address, stored current authority, and stored mint together; enforce state, delegate, close-authority, and extension policy separately
- **Create accounts through the typed creation builders**, which reject targets whose storage is not zeroed with `AccountAlreadyInitialized`
- **Use `invoke_with` or `invoke_signed_with`** when fixed-account creation must establish nonzero values before final PinaPod validation
- **Use generated `load_pda` or `load_pda_mut`** when a fixed stored-bump PDA handler needs a typed guard, so recursive content and the PDA address are validated once
- **Use generated `with_pda`** when a compact stored-bump PDA handler needs a compact view, so the layout, canonical bump, and PDA address are validated during the same borrow
- **Validate dynamic program accounts** with Pina's `assert_program()` before explicitly unverified CPI invocations; static and self-verifying CPI APIs need no redundant assertion
- **Use `as_account::<T>()` or `as_account_mut::<T>()`** when a handler needs fixed-account fields; these guard-backed loaders check the owner, discriminator, exact size, and nested values
- **Reserve `assert_type::<T>()` for validation-only paths** that do not need typed fields, and never treat it as proof for a later raw cast
- **Use `send_owned(&ID, amount, recipient)`** for direct lamport debits; it verifies that the program owns the sender before mutation
- **Use `CloseAccountZeroed { account, recipient, program_id: &ID }.invoke()` or `zeroed()` + `close_with_recipient(&ID, recipient)`** when stale account bytes must be invalidated before close
- **Use `CreateProgramAccount` or `CreateCompactProgramAccount` for canonical PDA creation**; their explicit-bump variants also reject noncanonical bumps without separate seed assertions
- **Keep `assert_seeds()` / `assert_canonical_bump()` for validation-only paths** that are not immediately followed by a checked creation builder
- **Give each account type its own seed namespace** so PDAs cannot collide across account types

<!-- {/pinaSecurityBestPractices} -->

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
