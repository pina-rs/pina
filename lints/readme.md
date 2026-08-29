# Custom Dylint Lints

Pina ships custom [Dylint](https://github.com/trailofbits/dylint) libraries for Solana program development. They turn repository security conventions into compiler diagnostics and are intended to run during normal development and CI.

The lints complement tests and audits; they do not prove that a program's economic design is safe. Every path-sensitive lint documents the approximation it uses so findings can be reviewed with the right expectations.

## Installation and execution

Run the catalog published with your installed Pina CLI:

```bash
pina lint
# Apply machine-applicable suggestions, then inspect the diff.
pina lint --fix
```

`pina lint` downloads precompiled Dylint executables from Pina's reusable release for the pinned Dylint version, then downloads native libraries from the matching Pina release. Executables match the CLI platform; libraries match the host reported by the active Rust compiler. This lets a musl-distributed CLI use GNU libraries on glibc Linux without pretending Rust's musl hosts support dynamic compiler plugins. Pina verifies each release digest, bundle manifest, host/toolchain identity, and file digest. Consuming projects do not register or build Pina's lint source; Dylint may still build its location-specific compiler driver for the active nightly toolchain.

Pina contributors still build the local source catalog when changing a lint:

```sh
devenv shell -- security:dylint
```

`security:dylint` is the authoritative gate. It discovers every package under `examples/` and every `security/*/secure` fixture, then invokes all libraries registered in the root `Cargo.toml` with `--no-deps` and `--locked`. Insecure fixtures are intentionally excluded because they demonstrate rejected patterns.

## Complete lint catalog

| Lint                                                    | Level | Primary invariant                                   |
| ------------------------------------------------------- | ----- | --------------------------------------------------- |
| `require_owner_before_token_cast`                       | deny  | Token data is parsed only after owner validation    |
| `require_empty_before_init`                             | deny  | Program accounts cannot be reinitialized            |
| `require_program_check_before_cpi`                      | deny  | CPI targets are authenticated                       |
| `deny_heap_allocations_in_onchain_instruction_handlers` | warn  | On-chain handlers avoid unbounded allocation cost   |
| `require_program_owned_before_lamport_mutation`         | deny  | Direct lamport debits affect program-owned accounts |
| `require_writable_before_account_resize`                | deny  | Resize targets are writable                         |
| `require_zeroed_before_close`                           | deny  | Closed account data is invalidated                  |
| `require_sysvar_assert_before_sysvar_use`               | deny  | Sysvar accounts cannot be substituted               |
| `require_type_assert_before_zero_copy_cast`             | deny  | Raw zero-copy casts follow type validation          |
| `require_associated_token_address_before_ata_cast`      | deny  | ATA identity is derived and checked                 |
| `require_reason_for_duplicate_remaining_accounts`       | deny  | Duplicate mutable remaining accounts are justified  |
| `require_canonical_bump_before_pda_write`               | deny  | PDA namespaces use canonical bumps                  |
| `deny_account_borrows_across_cpi`                       | deny  | Mutable data guards end before CPI                  |
| `require_consistent_token_program`                      | deny  | Token validation and CPI share one program identity |
| `require_explicit_token_2022_extension_policy`          | deny  | Token-2022 extensions are explicitly allow-listed   |
| `require_post_cpi_balance_reload`                       | deny  | Custody deposits use an observed balance delta      |
| `require_checked_asset_arithmetic`                      | deny  | Economic arithmetic fails on overflow/underflow     |
| `require_bounded_remaining_accounts`                    | deny  | Caller-controlled account work has a visible bound  |
| `require_idl_root_to_define_one_program_id`             | warn  | IDL roots expose exactly one program ID             |
| `require_canonical_instruction_dispatch_for_idl`        | warn  | Entrypoints use discoverable instruction dispatch   |
| `require_explicit_discriminators_and_seed_namespaces`   | warn  | Examples expose type and PDA namespaces             |

## Security and correctness reference

### `require_owner_before_token_cast`

Detects calls to the unchecked `as_token_mint()`, `as_token_account()`, `as_token_2022_mint()`, and `as_token_2022_account()` loaders without an earlier `assert_owner()` or `assert_owners()` on the same account.

An account can contain bytes shaped like token state while being owned by an attacker-controlled program. Parsing those bytes before checking ownership lets spoofed balances or authorities enter trusted logic.

```rust
mint.assert_owners(&[token::ID, token_2022::ID])?;
let mint = mint.as_token_mint()?;
```

The lint tracks the root receiver identifier and lexical call order within one function. Prefer the checked loaders or `*_for_program()` APIs when possible.

### `require_empty_before_init`

Detects `CreateProgramAccount`, `CreateProgramAccountWithBump`, and the matching creation functions when the target account has not first passed `assert_empty()`. It recognizes inline builders and builders stored in local variables.

```rust
state.assert_empty()?;
CreateProgramAccount { account: state, payer, owner: &ID, seeds }.invoke::<State>()?;
```

The analysis tracks concrete local/field places and builder bindings within one function. Validation hidden behind a helper is not treated as proof at the call site.

### `require_program_check_before_cpi`

Detects unchecked `invoke*()` calls. Dynamic CPI program arguments must be dominated by `assert_address()`, `assert_addresses()`, or `assert_program()`. Pina CPI builders whose program identity is fixed by their concrete type are recognized as trusted.

```rust
token_program.assert_address(&token::ID)?;
transfer.invoke_with_program(token_program.address())?;
```

The analyzer intersects validation state across `if` and `match` branches and invalidates proof after assignment or mutable aliasing. A check performed on only one branch is therefore insufficient.

### `require_program_owned_before_lamport_mutation`

Detects direct `send()` calls without an earlier ownership or typed-account check on the debited account.

```rust
vault.assert_owner(&ID)?;
vault.send(amount, recipient)?;
```

Direct lamport mutation is not a system-program transfer: the executing program must own the debited account. The lint uses lexical receiver matching, so keep the validation close to the mutation.

### `require_writable_before_account_resize`

Detects `resize()` without a preceding `assert_writable()` on the same account.

```rust
state.assert_writable()?;
state.resize(new_len)?;
```

The lint tracks lexical call order and receiver identity. It does not infer writability from comments, IDL metadata, or helper functions.

### `require_zeroed_before_close`

Detects `close()` or `close_with_recipient()` without an earlier `zeroed()` on the same account. Prefer `close_account_zeroed()` when the combined helper fits.

```rust
state.zeroed()?;
state.close_with_recipient(recipient)?;
```

This protects against stale bytes remaining observable during the transaction. The lint intentionally does not flag the combined zeroing close helper.

### `require_sysvar_assert_before_sysvar_use`

Detects reads from accounts whose names identify known sysvars without a matching `assert_sysvar()` and expected sysvar ID.

```rust
clock.assert_sysvar(&sysvar::clock::ID)?;
let data = clock.try_borrow()?;
```

The lint recognizes standard Solana sysvar names and instruction-sysvar loader functions. Unusually named sysvar wrappers may require an explicit, local code shape for the heuristic to recognize them.

### `require_type_assert_before_zero_copy_cast`

Detects raw zero-copy cast methods and known `bytemuck` cast functions when no prior `assert_type::<T>()`, `as_account::<T>()`, or `as_account_mut::<T>()` establishes the account layout.

```rust
account.assert_type::<Vault>(&ID)?;
let vault = account.as_account::<Vault>(&ID)?;
```

Pina instruction and account `try_from_bytes()` associated functions are safe framework conversions and are not treated as raw casts. The analysis correlates the nearest account borrow and validation within one function.

### `require_associated_token_address_before_ata_cast`

Detects `as_associated_token_account()` without a prior `assert_associated_token_address()` on the same account.

```rust
vault.assert_associated_token_address(wallet, mint, token_program)?;
let vault = vault.as_associated_token_account(wallet, mint, token_program)?;
```

Prefer `as_associated_token_account_checked()`, which performs owner and ATA derivation validation in one operation. The lint uses lexical receiver matching.

### `require_reason_for_duplicate_remaining_accounts`

Detects `#[pina(remaining, distinct = false)]` on mutable remaining accounts unless the field has a doc-comment explanation of at least five words.

```rust
/// Duplicate entries represent votes and are deduplicated before mutation.
#[pina(remaining, distinct = false)]
pub votes: &'a mut [AccountView],
```

`#[pina(remaining)]` is distinct by default. The word threshold only rejects missing or placeholder explanations; reviewers must still verify the stated invariant.

### `require_canonical_bump_before_pda_write`

Detects `assert_seeds_with_bump()` in instruction paths unless the same account has already passed `assert_canonical_bump()` or `assert_seeds()`.

```rust
let canonical = state.assert_canonical_bump(&seeds, &ID)?;
if canonical != supplied_bump {
	return Err(ProgramError::InvalidSeeds);
}
state.assert_seeds_with_bump(&seeds_with_bump, &ID)?;
```

Multiple valid bump values can otherwise create multiple addresses for one logical namespace. See Solana's [PDA documentation](https://solana.com/docs/core/pda). The lint tracks lexical receiver identity; it cannot inspect opaque validation helpers.

### `deny_account_borrows_across_cpi`

Detects CPI while a local returned by `try_borrow_mut()` or `as_account_mut()` is still alive.

```rust
let amount = {
	let state = account.as_account_mut::<State>(&ID)?;
	state.amount.get()
};
transfer.invoke()?;
```

An explicit `drop(guard)` or the end of a nested block releases the guard. The analysis follows block scope and explicit drops. It resolves method definitions before classifying a borrow or CPI, so an unrelated type that happens to define `try_borrow_mut()` or `invoke()` does not trigger the lint. Account borrows hidden inside custom wrapper constructors and CPIs hidden behind opaque helpers are outside its current model.

### `require_consistent_token_program`

Detects token parsing, ATA derivation, and dynamic token CPI calls that use different program identities within one instruction function.

```rust
token_program.assert_addresses(&SPL_PROGRAM_IDS)?;
let program_id = *token_program.address();
let mint = mint.as_token_mint_for_program(&program_id)?;
transfer.invoke_with_program(&program_id)?;
```

The lint compares resolved identifier paths, including module-qualified constants, so `token::ID` and `token_2022::ID` cannot collapse to the same terminal name. Immutable local aliases are traced back to their original identity, allowing clear names for parsing and CPI without reporting a mismatch. It still rejects reassignment of a program binding between token operations, because the same lexical name would otherwise hide a changed value. Copy and reuse a single immutable, validated address instead of independently deriving, mutating, or hard-coding program IDs.

### `require_explicit_token_2022_extension_policy`

Detects Token-2022-capable mint loads without an explicit call to `assert_no_extensions()` or `assert_extensions_allowed()` in the instruction function.

```rust
let mint = mint_account
	.as_token_mint_for_program(&program_id)?
	.assert_extensions_allowed(&[
		token_2022::state::ExtensionType::ImmutableOwner,
	])?;
```

Extensions can alter transfer, fee, hook, freeze, and authority semantics. Pina therefore requires an allow-list instead of treating the legacy base layout as a complete policy. The analysis pairs a policy with the concrete mint-view binding or with the same direct method chain; a policy asserted on a different mint does not satisfy the rule. Keep each policy adjacent to its mint load so the pairing also remains obvious to reviewers.

An `as_token_mint_for_program(&token::ID)` call with the canonical legacy SPL Token ID is exempt because Token-2022 extensions cannot be present. Dynamic program identities and the explicit Token-2022 loaders still require a policy.

Both policies are inherent, chainable methods on `TokenMintRef` and `TokenAccountRef`; they return the validated view rather than wrapping it in a separate free-function API.

`as_token_mint_for_program()` and `as_token_account_for_program()` only accept the canonical SPL Token and Token-2022 program IDs, require the account owner to match the selected ID, and parse the corresponding concrete layout. The caller therefore cannot make a legacy account appear to be Token-2022 (or vice versa) by supplying an arbitrary address. Extension assertions are a no-op on the validated legacy variant and inspect the actual TLV extension data on the validated Token-2022 variant.

### `require_post_cpi_balance_reload`

Detects token transfers into accounts whose names indicate protocol custody (`vault`, `custody`, `reserve`, or `pool`) unless the destination amount is read before and after CPI.

```rust
let before = vault.as_token_account_for_program(&program_id)?.amount();
transfer.invoke_with_program(&program_id)?;
let after = vault.as_token_account_for_program(&program_id)?.amount();
let received = after.checked_sub(before).ok_or(ProgramError::ArithmeticOverflow)?;
```

Token-2022 transfer fees can make `received` differ from the requested amount; Solana's [on-chain Token-2022 guide](https://www.solana-program.com/docs/token-2022/onchain) describes this accounting requirement. The lint pairs each source-visible `Transfer::new` or `TransferChecked::new` constructor with the direct invocation of that exact builder. It requires the closest destination reads on each side of the transfer to have no intervening CPI, then applies a custody-name heuristic and tracks direct receiver expressions. A static `invoke()` is exempt only when the resolved constructor belongs to the canonical `pinocchio_token` crate, including Pina's `token` re-export; local look-alikes and Token-2022 builders remain covered. Opaque builder wrappers are not diagnosed because the analysis cannot associate them with a particular invocation; audit such wrappers manually or keep the transfer direct in the instruction handler.

### `require_checked_asset_arithmetic`

Detects raw `+`, `-`, `*`, and `/`, plus saturating or wrapping arithmetic, when an operand has an economic identifier component such as `amount`, `balance`, `lamport`, `price`, `reward`, `stake`, or `supply`.

```rust
let next_balance = balance
	.checked_sub(amount)
	.ok_or(ProgramError::ArithmeticOverflow)?;
```

Saturating arithmetic is rejected because silently clamping economic state can violate conservation just as surely as wrapping. Components are split at Rust identifier separators, so `vault_balance` is covered while an unrelated name such as `rebalance_attempts` is not. The naming heuristic favors clear domain names and may not recognize opaque abbreviations.

### `require_bounded_remaining_accounts`

Detects loops whose source mentions `remaining` unless the iterator visibly uses `.take(MAX)` or a dominating constant-bound length guard rejects oversized input first.

```rust
const MAX_REMAINING_ACCOUNTS: usize = 16;
if remaining.len() > MAX_REMAINING_ACCOUNTS {
	return Err(ProgramError::InvalidArgument);
}
for account in remaining {
	process(account)?;
}
```

Remaining accounts are caller-controlled; an explicit bound keeps worst-case compute auditable. Rejecting an oversized list is preferred when every supplied account must be processed, while `.take(MAX)` is suitable only when ignoring surplus accounts is intentional. The guard must compare `remaining.len()` against an integer literal or resolved constant, return early on the oversized path, and dominate the loop. A runtime limit, branch-local check, late check, or opaque helper does not satisfy the rule because it does not establish a source-visible protocol maximum on every path.

## Performance reference

### `deny_heap_allocations_in_onchain_instruction_handlers`

Warns on `collect`, `to_vec`, `to_string`, `clone`, `format!`, `Vec` creation, and `String` creation in functions whose names identify instruction handlers.

```rust
let mut bytes = [0u8; MAX_MESSAGE_BYTES];
bytes[..input.len()].copy_from_slice(input);
```

The lint is a performance warning rather than a correctness denial because some off-chain or bounded on-chain designs may intentionally allocate. It uses method and function-name heuristics and does not estimate actual heap size.

## IDL and example-structure reference

### `require_idl_root_to_define_one_program_id`

Warns when an IDL-oriented example or security crate does not expose exactly one crate-root `declare_id!` expansion.

```rust
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
```

The check is repository-scoped to definition paths under `examples` and `security`. Library crates that intentionally define no program are ignored.

### `require_canonical_instruction_dispatch_for_idl`

Warns when `process_instruction` or an entrypoint does not directly contain a `match` over parsed instruction data.

```rust
match instruction {
	Instruction::Initialize => InitializeAccounts::try_from((program_id, accounts))?.process(data),
	Instruction::Update => UpdateAccounts::try_from((program_id, accounts))?.process(data),
}
```

The check keeps dispatch visible to `pina idl` and reviewers. It verifies the presence of direct match-shaped routing, not semantic exhaustiveness.

### `require_explicit_discriminators_and_seed_namespaces`

Warns when seed assertions in example instruction paths do not visibly use a byte-string namespace, a named `SEED`/`*_SEED` constant, or a generated Pina seed helper.

```rust
const VAULT_SEED: &[u8] = b"vault";
vault.assert_seeds(&[VAULT_SEED, authority.address().as_ref()], &ID)?;
```

Associated seed helpers generated from `#[pda(...)]` are accepted because the macro declaration exposes the namespace at the account type. Receiver-less local functions named like assertion methods are not treated as framework proof. The rule is a reviewability warning and does not replace canonical bump validation.

## Suppression policy

Prefer making validation and bounds explicit instead of suppressing a finding. When a false positive cannot be expressed more clearly, scope `#[allow(...)]` to the smallest item and add a doc comment explaining the invariant. Deny-level security lints should not be disabled at crate or workspace scope.
