# Lint reference

Every lint Pina ships, with the contract it enforces, why violating it is a vulnerability, and how to bless an intentional exception.

`pina lint --explain <LINT>` prints the same entry for one lint without leaving the terminal.

## How to read this page

A lint is a claim about your program, and a finding is the compiler telling you it could not prove that claim. Two things follow from that:

- **Fix the finding, do not silence it.** Every entry names the API or restructure that satisfies the contract. That is the intended resolution.
- **Blessing is a documented decision, not a suppression.** Where an entry describes an `#[allow]`, it is scoped to the smallest item and the entry says what to write in the comment. A crate-wide `allow` in `pina.toml` removes the signal for everyone, including the next person who adds a genuine violation to the same module.

### Configuring levels

Default levels are set per lint. Override them in the `[lints]` table of `pina.toml`:

```toml
[lints]
# Raise a heuristically-noisy warn to a hard requirement.
deny_heap_allocations_in_onchain_instruction_handlers = "deny"
# Accept a documented, deliberate exception at crate scope.
require_explicit_discriminators_and_seed_namespaces = "allow"
```

Prefer an item-scoped `#[allow]` over a crate-scoped `allow`, with a comment naming the invariant that makes the exemption safe.

## Default levels

A `deny` lint is a security property: the build fails, and there is no supported way to ship without addressing it. A `warn` lint is heuristically detected or advisory, so a false positive is expected occasionally and the blessing guidance is what matters.

| Lint                                                                                                            | Default |
| --------------------------------------------------------------------------------------------------------------- | ------- |
| [deny_account_borrows_across_cpi](#deny_account_borrows_across_cpi)                                             | `deny`  |
| [deny_colliding_account_discriminators](#deny_colliding_account_discriminators)                                 | `deny`  |
| [deny_heap_allocations_in_onchain_instruction_handlers](#deny_heap_allocations_in_onchain_instruction_handlers) | `warn`  |
| [deny_unchecked_remaining_mut](#deny_unchecked_remaining_mut)                                                   | `deny`  |
| [deny_unused_account_borrow_guards](#deny_unused_account_borrow_guards)                                         | `warn`  |
| [require_bounded_remaining_accounts](#require_bounded_remaining_accounts)                                       | `deny`  |
| [require_canonical_bump_before_pda_write](#require_canonical_bump_before_pda_write)                             | `deny`  |
| [require_canonical_instruction_dispatch_for_idl](#require_canonical_instruction_dispatch_for_idl)               | `warn`  |
| [require_checked_asset_arithmetic](#require_checked_asset_arithmetic)                                           | `deny`  |
| [require_consistent_token_program](#require_consistent_token_program)                                           | `deny`  |
| [require_explicit_discriminators_and_seed_namespaces](#require_explicit_discriminators_and_seed_namespaces)     | `warn`  |
| [require_explicit_token_2022_extension_policy](#require_explicit_token_2022_extension_policy)                   | `deny`  |
| [require_guarded_full_balance_drain](#require_guarded_full_balance_drain)                                       | `warn`  |
| [require_idl_root_to_define_one_program_id](#require_idl_root_to_define_one_program_id)                         | `warn`  |
| [require_post_cpi_balance_reload](#require_post_cpi_balance_reload)                                             | `deny`  |
| [require_program_check_before_cpi](#require_program_check_before_cpi)                                           | `deny`  |
| [require_reason_for_duplicate_remaining_accounts](#require_reason_for_duplicate_remaining_accounts)             | `deny`  |
| [require_sysvar_assert_before_sysvar_use](#require_sysvar_assert_before_sysvar_use)                             | `deny`  |
| [require_type_assert_before_zero_copy_cast](#require_type_assert_before_zero_copy_cast)                         | `deny`  |
| [require_writable_before_account_resize](#require_writable_before_account_resize)                               | `deny`  |
| [require_zeroed_before_close](#require_zeroed_before_close)                                                     | `deny`  |

## deny_account_borrows_across_cpi

Default level: `deny`

**Contract.** Drop every mutable account-data borrow before invoking another program.

**Why this matters.** The invoked program may need the same account. Holding a `RefMut` across the CPI makes the invocation fail at runtime and hides the re-entrancy boundary the borrow was documenting.

**Blessing an exception.** Call `drop(guard)` before the CPI, or narrow the borrow so it ends before the invocation. Prefer restructuring over an `#[allow]`: the attribute silences the check without ending the borrow, so the runtime failure returns.

## deny_colliding_account_discriminators

Default level: `deny`

**Contract.** Every account type's `HasDiscriminator::VALUE` carries a numeric value no other account type in the program also claims at the same discriminator width.

**Why this matters.** Pina discriminators are author-chosen integers, and rustc only rejects duplicates within one enum. Two account types behind different enums that agree on the value and the serialized width pass every typed loader check — owner, discriminator, exact size — so either account deserializes as the other: the sealevel-attacks type-cosplay class.

**Blessing an exception.** There is no sound exception; two account types at one value is a latent vulnerability. Give the colliding variant a fresh value (wire values are part of the ABI, so ship it as a migration), or consolidate all accounts behind one discriminator enum, where rustc makes the collision impossible.

## deny_heap_allocations_in_onchain_instruction_handlers

Default level: `warn`

**Contract.** Avoid heap allocation in instruction handlers.

**Why this matters.** On-chain code is charged for every allocated byte and for the code that manages it. Borrowed slices, stack buffers, and fixed-size POD types keep both compute units and deployed size down.

**Blessing an exception.** Warns by default and is heuristic: it matches allocation-prone method names in handler-like functions, so a non-allocating `Clone` implementation or a foreign API can trip it. Scope `#[allow]` to the individual item and say which call is known not to allocate.

## deny_unchecked_remaining_mut

Default level: `deny`

**Contract.** Reach remaining accounts through `remaining_mut_distinct()` or the derive attribute rather than `AccountsCursor::remaining_mut()`.

**Why this matters.** `remaining_mut()` validates writability but preserves duplicate addresses. Mutating through two aliases to one account applies a single logical update twice, which is the duplicate-mutable-account vulnerability class.

**Blessing an exception.** An instruction that genuinely must accept a repeated address — a self-transfer, for example — should use `#[pina(remaining, distinct = false)]`, which documents the exception at the field that takes it. That is preferred over a raw `remaining_mut()` call, which the lint cannot distinguish from an oversight.

## deny_unused_account_borrow_guards

Default level: `warn`

**Contract.** Read or discard an account borrow guard immediately instead of binding it to a local.

**Why this matters.** The guard keeps the account-data borrow alive until the end of the enclosing scope. Binding and never reading it holds the borrow open for nothing and turns a later borrow of the same data into a runtime panic.

**Blessing an exception.** Bind the guard as `_guard` if the borrow must outlive the statement, or call `drop(guard)` where it should end. Both state the intent; an `#[allow]` does not, and the borrow outlives the attribute either way.

## require_bounded_remaining_accounts

Default level: `deny`

**Contract.** Bound every loop over remaining accounts with a constant `.take(MAX)` or a dominating constant-bound length check.

**Why this matters.** The caller controls how many accounts arrive. Linear per-account work over an unbounded count turns into compute exhaustion, which is a denial of service against the instruction.

**Blessing an exception.** Define the maximum as a `const MAX: usize` and apply it with `.take(MAX)`, or reject oversized input first with a constant-bound length check. A caller-derived bound does not satisfy the contract: the lint requires a constant so the worst-case cost is auditable from the source.

## require_canonical_bump_before_pda_write

Default level: `deny`

**Contract.** Prove a PDA bump is canonical with `assert_canonical_bump()` before accepting a PDA through `assert_seeds_with_bump()`. `assert_stored_bump()` is the generated counterpart of `assert_seeds()`: it reuses a bump the handler parsed from the same account, and this lint requires that provenance.

**Why this matters.** A program-derived address has one canonical bump. Accepting any valid bump lets one seed namespace resolve to several addresses, breaking the uniqueness the seeds were chosen to provide. `assert_stored_bump()` names the one legitimate source for an explicit bump — the account's own stored field, read in this instruction — so the provenance is checked rather than assumed.

**Blessing an exception.** `CreateProgramAccount` and `CreateProgramAccountWithBump` validate canonicality internally and need no assertion. Where several addresses per namespace are genuinely intended, use `CreateProgramAccountWithUncheckedBump`, which names the decision. `assert_stored_bump()` passes only when its bump argument resolves to a parse of the same account; a bump from instruction data or a different account fails. Reach for `#[allow]` only on a validation-only path that accepts non-canonical bumps by design, and name that invariant in the comment.

## require_canonical_instruction_dispatch_for_idl

Default level: `warn`

**Contract.** Match directly on the parsed instruction enum in the entrypoint.

**Why this matters.** IDL extraction starts from the entrypoint. An explicit `match` over the instruction enum is what lets the extractor resolve every accounts struct, so hidden dispatch means a program whose IDL is incomplete or wrong.

**Blessing an exception.** Restructure the dispatch. If the indirection is required, scope an `#[allow]` to the entrypoint function and note which construct the extractor cannot follow.

## require_checked_asset_arithmetic

Default level: `deny`

**Contract.** Use checked arithmetic for values that carry an economic quantity: balances, amounts, prices, rewards, stakes, supply, or lamports.

**Why this matters.** Silent overflow, underflow, or saturation corrupts an economic invariant without failing. The corruption is often permanent and can be worth real tokens, so the arithmetic must fail loudly.

**Blessing an exception.** Use the checked operation and map the error into the program's error enum. An `#[allow]` is appropriate only where the invariant is proven by the surrounding code, and the comment should state the bound that makes it safe.

## require_consistent_token_program

Default level: `deny`

**Contract.** Use one token-program identity for parsing, ATA derivation, and dynamic token CPI within one instruction.

**Why this matters.** Mixing identities can validate an account under one token program and then invoke another. The ownership and address assumptions that justified the validation no longer hold for the program that acts.

**Blessing an exception.** Decide the token program once and thread that single value through: read it from the `TokenAccountRef` or `TokenMintRef` you already resolved, whose `from_account_view` validated that the account belongs to `ID` or `crate::token_2022::ID`. Where a program supports both in one instruction, branch on the identity first and keep each branch internally consistent.

## require_explicit_discriminators_and_seed_namespaces

Default level: `warn`

**Contract.** Give seed-based code an explicit byte-string namespace and make discriminator markers visible.

**Why this matters.** Explicit namespaces keep seed derivation auditable and let the IDL extractor follow the program's account layout. Without them, two account roles can share a namespace and collide.

**Blessing an exception.** Declare the namespace as a named `const` byte string, for example `const SEED_CONFIG: &[u8] = b"config";`. The lint checks that a namespace is visible, not that all namespaces differ; compare them across account roles yourself.

## require_explicit_token_2022_extension_policy

Default level: `deny`

**Contract.** State which Token-2022 extensions an instruction accepts before it reads a Token-2022-capable mint's fields.

**Why this matters.** Extensions change transfer and authority semantics — transfer fees, permanent delegates, transfer hooks. Reading only the legacy base fields silently treats those semantics as irrelevant, and accounting computed from them is wrong.

**Blessing an exception.** Assert the policy explicitly on the mint: `assert_extensions_allowed(&[...])` for the extensions the program handles, or `assert_no_extensions()` when it handles none. Do that rather than allowing the lint, because the policy is the thing the lint is asking for.

## require_guarded_full_balance_drain

Default level: `warn`

**Contract.** Gate an instruction that can sweep an account's entire balance behind a pause, circuit breaker, or withdrawal cap.

**Why this matters.** An ungated full-balance drain is the shape real key-compromise exploits use. Once the sweep authority leaks, nothing on-chain slows the drain; a pause switch plus a per-window cap bounds the blast radius.

**Blessing an exception.** Add the guard, or express the operation as a close when that is the intent, since a close states where the remaining lamports go. The guard must behave like one: its name states the pause or cap check (or it is a local wrapper returning `Result` that enforces such a guard before any early success return), its receiver or an argument is derived from the handler's parameters, and it stops the handler on the failing value with `?`, `unwrap`, or a branch that returns `Err` or panics; the polarity of `is_err`, `is_ok`, and `assert_eq!` is checked. A discarded result, a branch that returns `Ok`, a zero-argument or literal-only call, a closure, and a local callee that can only return a literal success do not count. Where a drain is intended and bounded elsewhere, scope `#[allow]` to the handler and name the compensating control.

## require_idl_root_to_define_one_program_id

Default level: `warn`

**Contract.** Define exactly one program ID at the crate root.

**Why this matters.** IDL extraction starts from the crate root and expects a single declaration. Several IDs make the resolution ambiguous, and none means the extractor has no anchor.

**Blessing an exception.** Keep one `declare_id!` at the root and move test or auxiliary IDs behind `#[cfg(test)]`. A crate that exports a program ID for another crate to consume should be a library, not the IDL root; scope `#[allow]` to that item if the layout must stay.

## require_post_cpi_balance_reload

Default level: `deny`

**Contract.** Read a custody destination both before and after a token transfer CPI and account from the observed delta.

**Why this matters.** Token-2022 transfer fees can make the amount received differ from the amount requested. Accounting from the requested amount rather than the observed balance delta credits the protocol with tokens it never received.

**Blessing an exception.** Reload the destination with `amount()` after the CPI and compute the delta. This is the fix the lint asks for, so there is no exception to bless: a transfer whose fee is known to be zero still has a correct delta, and reading it costs one load.

## require_program_check_before_cpi

Default level: `deny`

**Contract.** Validate a dynamic CPI target with `assert_address()`, `assert_addresses()`, or `assert_program()` against a compile-time program ID before invoking it.

**Why this matters.** A dynamic program argument controls the CPI target. Without verifying that exact argument, an attacker substitutes a malicious program. An instruction argument is not a trusted expected ID — comparing two attacker-controlled values proves consistency, not authenticity — and success-side adapters such as `map()` can replace the validated binding before execution continues.

**Blessing an exception.** Call `assert_address()`, `assert_addresses()`, or `assert_program()` against a compile-time ID on every continuing path before the invocation and do not discard the result. There is no sound way to bless an unverified dynamic CPI target: the check is the entire security property. Use a hardcoded program ID type when the target is in fact fixed.

## require_reason_for_duplicate_remaining_accounts

Default level: `deny`

**Contract.** Document why a field opts out of distinctness with `#[pina(remaining, distinct = false)]`.

**Why this matters.** Opting out of the duplicate-address check reintroduces the duplicate mutable-account vulnerability. The doc comment forces the author to state the reason, which is the only thing distinguishing a deliberate exception from a mistake.

**Blessing an exception.** Write the doc comment. This lint _is_ the blessing mechanism: it asks for an explanation rather than forbidding the pattern, so an `#[allow]` would remove the only recorded justification.

## require_sysvar_assert_before_sysvar_use

Default level: `deny`

**Contract.** Call `assert_sysvar()` on an account before reading it as a sysvar.

**Why this matters.** A sysvar account is a specific address. Reading an account that merely has a sysvar-shaped layout without checking its address lets an attacker substitute data of their choosing for the clock, rent, or another sysvar.

**Blessing an exception.** Assert the sysvar kind on the account. The assertion derives the expected address from the canonical sysvar ID, so it is strictly stronger than comparing an ID supplied by the caller; prefer it over an `#[allow]` even where the surrounding code looks sufficient.

## require_type_assert_before_zero_copy_cast

Default level: `deny`

**Contract.** Convert account data through a guard-backed Pina conversion instead of a raw zero-copy cast.

**Why this matters.** A raw cast reinterprets account bytes as a struct without proving the account is the expected type or that the data is large enough and aligned. The result is type cosplay: attacker-controlled bytes read as trusted fields.

**Blessing an exception.** Use the guard-backed conversion — `assert_type()` on the account, or a typed loader such as `TokenAccountRef::from_account_view()` — which checks the account type and length and keeps the borrow alive. Do not bless a raw cast on account data: the check it skips is what makes reading the fields sound.

## require_writable_before_account_resize

Default level: `deny`

**Contract.** Call `assert_writable()` on an account before resizing it.

**Why this matters.** Writing to an account the transaction did not mark writable fails at runtime, and the resize is a write. The check also documents that the instruction intends to change the account's size, which a reviewer and the client both need to know.

**Blessing an exception.** Assert writability on the account before the resize. A mutable fixed field parsed through `AccountsCursor::next_mut` already validated writability, so no exception is needed there; reach for `#[allow]` only on a path whose writability is established by a construct the lint cannot follow, and name it.

## require_zeroed_before_close

Default level: `deny`

**Contract.** Call `zeroed()` on an account before closing it.

**Why this matters.** A closed account's lamports are gone but its data survives until the account is reused. A later instruction that reads before writing sees the previous contents, so stale data can be reinterpreted as valid state.

**Blessing an exception.** Zero the account before closing. Prefer Pina's `close_account_zeroed()`, which zeroes and closes as one operation and cannot be reordered. There is no sound reason to close a Pina account without zeroing it.
