# `pina_lints`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

`pina_lints` is Pina's self-contained replacement for the previous [Dylint](https://github.com/trailofbits/dylint) setup: every security, performance, and IDL lint that Pina ships lives in this one importable crate, so the lints are built into Pina instead of being distributed as separate Dylint libraries. They turn repository security conventions into compiler diagnostics and are intended to run during normal development and CI.

The crate keeps the Dylint authoring shape — each lint lives in its own module under `lints` and declares itself with `declare_late_lint!` or `declare_pre_expansion_lint!` — but no lint registers itself; registration is centralized in `register_all_lints`. The crate builds as both a library and a cdylib that exports the Dylint-compatible `register_lints` symbol, so a Dylint driver can still load it as a single library. It also ships the bundled `pina_lint_driver` binary, a `rustc` wrapper with every lint statically linked; `pina lint` runs it as `RUSTC_WORKSPACE_WRAPPER`, so it needs no external lint tooling.

The lints complement tests and audits; they do not prove that a program's economic design is safe. Every path-sensitive lint documents the approximation it uses so findings can be reviewed with the right expectations.

<!-- {=crateReadmeBadgeRow:"pina_lints"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina__lints-orange?logo=rust)](https://crates.io/crates/pina_lints) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina__lints-1f425f?logo=docs.rs)](https://docs.rs/pina_lints/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

## Installation and execution

Run the catalog shipped with your installed Pina CLI:

```bash
pina lint
# Apply machine-applicable suggestions, then inspect the diff.
pina lint --fix
```

`pina lint` builds and manages the bundled `pina_lint_driver` under Cargo home, then invokes cargo with the driver as `RUSTC_WORKSPACE_WRAPPER`. Cargo calls the driver with the arguments it would have passed to `rustc`; the driver registers every lint compiled into this crate, and compilation continues normally. Cargo preserves and nests an existing `RUSTC_WRAPPER`, such as `sccache`, outside the lint driver. Because the lints are statically linked into the driver, no external lint tooling is downloaded or installed. The driver reads a few environment variables: `PINA_LINT_NO_DEPS` skips dependency crates, `PINA_LINT_LEVELS` forwards configured lint levels to `rustc` (see [Configuring lint levels](#configuring-lint-levels)), `PINA_LINT_ONLY` restricts linting to a single lint, and `PINA_LINT_LIST` prints the lint catalog instead of compiling. `PINA_LINT_NO_DEPS`, `PINA_LINT_LEVELS`, and `PINA_LINT_ONLY` are recorded in dep-info, so changing them invalidates cargo's cached check results.

The crate itself is nightly-only: the lint passes and the driver link against the Rust compiler's unstable `rustc_private` crates. It is published to crates.io, but consuming projects never build it — the CLI builds and manages the driver from the project directory for the active toolchain.

Pina contributors still run the in-workspace driver when changing a lint:

```sh
devenv shell -- security:pina-lint
```

`security:pina-lint` is the authoritative gate. It builds the workspace's `pina_lint_driver` binary and runs cargo with `RUSTC_WORKSPACE_WRAPPER` pointing at it, discovering every package under `examples/` and every `security/*/secure` fixture, then checks each one in the driver's no-deps mode with `--locked`. Insecure fixtures are intentionally excluded because they preserve examples of unsafe patterns.

## Importing the lints

Every lint constant and pass is public:

```rust,ignore
use pina_lints::lints::require_consistent_token_program::REQUIRE_CONSISTENT_TOKEN_PROGRAM;
use pina_lints::lints::require_consistent_token_program::RequireConsistentTokenProgram;
```

Tooling that needs to validate lint names or enumerate the catalog can read `pina_lints::LINT_NAMES`, which lists every lint in the crate in catalog (alphabetical) order.

## Configuring lint levels

Lint levels are configured in the project's `pina.toml` under the `[lints]` table. Each entry maps a lint name to `allow`, `warn`, or `deny`; lints that are not listed use their built-in default level (the "Level" column in the catalog below). The Pina CLI reads the table and passes the result to `pina_lint_driver` through the `PINA_LINT_LEVELS` environment variable; the driver forwards each level to `rustc` as an `--allow`, `--warn`, or `--deny` argument.

```toml
[lints]
deny_heap_allocations_in_onchain_instruction_handlers = "deny"
require_explicit_discriminators_and_seed_namespaces = "allow"
```

Deny-level security lints should not be disabled at crate scope; see the [suppression policy](#suppression-policy) below.

## Complete lint catalog

| Lint                                                    | Level | Primary invariant                                   |
| ------------------------------------------------------- | ----- | --------------------------------------------------- |
| `require_empty_before_init`                             | deny  | Program accounts cannot be reinitialized            |
| `require_program_check_before_cpi`                      | deny  | CPI targets are authenticated                       |
| `deny_heap_allocations_in_onchain_instruction_handlers` | warn  | On-chain handlers avoid unbounded allocation cost   |
| `require_writable_before_account_resize`                | deny  | Resize targets are writable                         |
| `require_zeroed_before_close`                           | deny  | Closed account data is invalidated                  |
| `require_sysvar_assert_before_sysvar_use`               | deny  | Sysvar accounts cannot be substituted               |
| `require_type_assert_before_zero_copy_cast`             | deny  | Raw account casts use guard-backed typed loading    |
| `require_reason_for_duplicate_remaining_accounts`       | deny  | Duplicate mutable remaining accounts are justified  |
| `deny_unchecked_remaining_mut`                          | deny  | Direct mutable remaining accounts reject aliases    |
| `require_canonical_bump_before_pda_write`               | deny  | PDA namespaces use canonical bumps                  |
| `deny_account_borrows_across_cpi`                       | deny  | Mutable data guards end before CPI                  |
| `deny_unused_account_borrow_guards`                     | warn  | Unread borrow guards are discarded immediately      |
| `require_consistent_token_program`                      | deny  | Token validation and CPI share one program identity |
| `require_explicit_token_2022_extension_policy`          | deny  | Token-2022 extensions are explicitly allow-listed   |
| `require_post_cpi_balance_reload`                       | deny  | Custody deposits use an observed balance delta      |
| `require_checked_asset_arithmetic`                      | deny  | Economic arithmetic fails on overflow/underflow     |
| `require_bounded_remaining_accounts`                    | deny  | Caller-controlled account work has a visible bound  |
| `require_idl_root_to_define_one_program_id`             | warn  | IDL roots expose exactly one program ID             |
| `require_canonical_instruction_dispatch_for_idl`        | warn  | Entrypoints use discoverable instruction dispatch   |
| `require_explicit_discriminators_and_seed_namespaces`   | warn  | Examples expose type and PDA namespaces             |

## Security and correctness reference

### `require_empty_before_init`

Detects `CreateProgramAccount`, `CreateProgramAccountWithBump`, and the matching creation functions when the target account has not first passed `assert_empty()`. It recognizes inline builders and builders stored in local variables.

```rust
state.assert_empty()?;
CreateProgramAccount { account: state, payer, owner: &ID, seeds }.invoke::<State>()?;
```

The analysis tracks concrete local/field places and builder bindings within one function. Validation hidden behind a helper is not treated as proof at the call site.

### `require_program_check_before_cpi`

Detects `invoke_with_unverified_program()` and `invoke_signed_with_unverified_program()` calls whose exact dynamic CPI program argument has not first passed `assert_program()`, `assert_address()`, or `assert_addresses()` on every path to the invocation. Prefer `assert_program()` when the handler receives an explicit program account because it checks both the address and the executable flag.

```rust
token_program.assert_program(&token::ID)?;
transfer.invoke_with_unverified_program(token_program.address())?;
```

Pinocchio Token's `.invoke_with_program()` and `.invoke_signed_with_program()` methods call `Program::verify()` themselves, so they do not need a separate assertion. Prefer those verified methods unless the handler has already validated the program account and deliberately needs the lower-overhead unverified variant. Static `.invoke()` and `.invoke_signed()` builders encode their target program and also need no separate program account assertion. Passing a constant such as `&token::ID` to an unverified invocation is accepted because the caller cannot substitute the value.

The analyzer tracks concrete local variables and fields and intersects validation state across continuing `if` and `match` branches. Checking an unrelated account does not authorize the dynamic target, and checking one branch does not authorize a later call unless every path that continues establishes the same proof.

Call unverified CPI methods directly with method or UFCS syntax. Taking one as a function value is denied at the function item, including casts, assignments, containers, closures, and conditional expressions. This deliberate boundary keeps the exact target argument visible to the lint instead of approximating Rust's full value and closure data flow. If a reviewed abstraction must store one of these functions, use a narrowly scoped lint allowance and document how it authenticates the supplied program.

To migrate existing code, remove assertions that exist only before `.invoke()`, `.invoke_signed()`, `.invoke_with_program()`, or `.invoke_signed_with_program()`. Keep exact-target validation before the explicitly unverified methods. Existing code that used a verified method only to satisfy this lint needs no API change; this release corrects the false positive.

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
state.close_with_recipient(&ID, recipient)?;
```

This protects against stale bytes remaining observable during the transaction. The lint intentionally does not flag the combined zeroing close helper.

### `require_sysvar_assert_before_sysvar_use`

Detects raw reads from accounts whose names identify known sysvars without a matching `assert_sysvar()` and expected sysvar ID.

```rust
clock.assert_sysvar(&sysvar::clock::ID)?;
let data = clock.try_borrow()?;
```

Prefer Pinocchio's checked typed loaders when you need the sysvar value. They validate the account address while parsing, so a separate `assert_sysvar()` call would repeat the same check:

```rust
let clock = Clock::from_account_view(clock_account)?;
let rent = Rent::from_account_view(rent_account)?;
let instructions = Instructions::try_from(instructions_account)?;
```

Keep `assert_sysvar()` when code only validates identity or deliberately borrows the raw account data. The lint identifies Pinocchio constructors that do not validate identity by their resolved definition: `Clock` and `Rent` byte constructors, `Instructions::new_unchecked`, and `SlotHashes::new` / `new_unchecked`. It reports direct calls at their source and rejects storing these constructors as function values. This source boundary catches replacement through adapters, helper calls, mutable borrows, aliases, and destructuring without attempting to reconstruct arbitrary downstream value provenance.

To migrate existing code, replace manual byte parsing with the matching checked typed loader. Checked results and checked constructor function values can use ordinary Rust extraction, adapters, tuples, patterns, and control flow without special lint knowledge. Call an identity-unchecked constructor directly so its source remains visible to the lint. For deliberate raw parsing, call `assert_sysvar()` before borrowing the data, then place a narrow lint allowance directly on the reviewed constructor. No change is needed for identity-only checks or asserted raw account access. The separate raw-read heuristic still uses standard Solana sysvar account names, so unusually named raw accounts may require a direct, local assertion.

```rust,ignore
rent_account.assert_sysvar(&sysvar::rent::ID)?;
let data = rent_account.try_borrow()?;
// Reviewed exception: the preceding assertion fixes the raw data's identity.
#[allow(require_sysvar_assert_before_sysvar_use)]
let rent = Rent::from_bytes(&data)?;
```

### `require_type_assert_before_zero_copy_cast`

Detects known `bytemuck` cast functions in `pina::ProcessAccountInfos::process` implementations and conventional `process_instruction` entrypoints. Unrelated functions or inherent methods named `process` remain outside the lint boundary. Use a Pina conversion that validates and borrows the account as one operation.

```rust
let vault = account.as_account::<Vault>(&ID)?;
```

For PDAs, prefer the generated `load_pda*` methods because they also validate the address and stored bump. `assert_type::<T>()` remains useful when a handler only needs validation, but it is a moment-in-time check and does not make a later raw cast safe. Pina instruction and account `try_from_bytes()` associated functions are safe framework conversions and are not treated as raw casts.

### `require_reason_for_duplicate_remaining_accounts`

Detects `#[pina(remaining, distinct = false)]` on mutable remaining accounts unless the field has a doc-comment explanation of at least five words.

```rust
/// Duplicate entries represent votes and are deduplicated before mutation.
#[pina(remaining, distinct = false)]
pub votes: &'a mut [AccountView],
```

`#[pina(remaining)]` is distinct by default. The word threshold only rejects missing or placeholder explanations; reviewers must still verify the stated invariant.

### `deny_unchecked_remaining_mut`

Detects direct calls to Pina's `AccountsCursor::remaining_mut()`. The method validates writability but preserves duplicate addresses, so one logical account can appear more than once in the returned mutable slice.

```rust
let remaining = cursor.remaining_mut_distinct()?;
```

The lint resolves the method definition before reporting, so same-named methods from other crates are ignored. It rejects method syntax, UFCS calls, stored function items, and calls hidden by local or external macros. Pina's `#[derive(Accounts)]` expansion remains exempt: the macro uses `remaining_mut()` only for the explicit, documented `#[pina(remaining, distinct = false)]` escape hatch, which is checked separately by `require_reason_for_duplicate_remaining_accounts`.

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

This lint applies to validation-only assertion chains. `CreateProgramAccountWithBump` and `CreateCompactProgramAccountWithBump` enforce canonicality inside the builder, so creation handlers should not call either assertion first. Prefer the canonical builders when the instruction does not need to carry a bump.

### `deny_account_borrows_across_cpi`

Detects CPI while a local returned by `try_borrow_mut()` or `as_account_mut()` is still alive.

```rust
let amount = {
	let state = account.as_account_mut::<State>(&ID)?;
	state.amount.get()
};
transfer.invoke()?;
```

An explicit `drop(guard)` or the end of a nested block releases the guard. The analysis follows block scope, locally bound closure calls, match guards, nested binding patterns, guard-returning aliases, and calls to the real `std::mem::drop`. Closure bodies are evaluated with the borrow state at each visible invocation, so defining a callback before a borrow or dropping a borrow before invoking it is modeled in execution order. It resolves method and guard types before classifying a borrow or CPI, so unrelated same-named operations do not create or discharge a proof. Account borrows hidden inside custom wrapper constructors, closures invoked through opaque higher-order helpers, and CPIs hidden behind opaque helpers are outside its current model.

### `deny_unused_account_borrow_guards`

Detects account borrow guards bound to locals that are never read.

```rust
// Flagged: the guard is bound but never read, so the account data borrow
// stays open until the end of the enclosing scope for nothing.
let _guard = mint.as_token_mint_for_program(&token_program)?;

// Preferred: discard the validation value immediately.
mint.as_token_mint_for_program(&token_program)?;
```

Assertion-style guards exist only for their `?` validation. Binding them without reading keeps the borrow open to the end of the scope, which obscures the borrow boundary and can turn a later borrow of the same account data into a runtime panic. Discard immediately by calling the validation as a `?` statement — optionally wrapped as `drop(account.try_borrow()?)` — or write `let _ = account.try_borrow()?;` at the creation site; when the value matters, read it. `let _ = guard;` does not move an existing local in Rust, so it does not release the borrow and remains a lint warning. Passing a bound guard to a later `drop(local)` is also flagged because the borrow stayed open in between. The lint recognizes the concrete `solana_account_view::Ref` and `RefMut` binding types re-exported by Pinocchio and Pina, including aliases, values returned through function pointers, and bindings nested in tuple or `let ... else` patterns. It counts any use of the binding — method calls, field access, `&` borrows, and closure captures — as a read. Wrapper types that contain a guard are outside its current model. A `drop(local)` call is treated as a discard rather than a read only when it resolves to `std::mem::drop`; a shadowing `drop` function is an ordinary use.

The warning carries a suggested rewrite: `pina lint --fix` rewrites the binding into the immediate `?` statement. When the only other occurrence of the binding is a later `drop(local);`, the suggested edit also removes that statement — but it is marked `MaybeIncorrect` rather than machine-applicable, because releasing the borrow earlier than the user wrote it is observable to the code in between and needs human review. Suggestions are withheld entirely when the guard binding originates inside a macro expansion or when a `drop` appears outside a statement (for example inside a closure tail), because the rewrite could not be applied safely there.

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

Saturating arithmetic is rejected because silently clamping economic state can violate conservation just as surely as wrapping. The check applies to primitive integers; custom domain types own their arithmetic contract and are not given an inapplicable `checked_*` suggestion. Components are split at Rust identifier separators, so `vault_balance` is covered while an unrelated name such as `rebalance_attempts` is not. The naming heuristic favors clear domain names and may not recognize opaque abbreviations.

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

Remaining accounts are caller-controlled; an explicit bound keeps worst-case compute auditable. Rejecting an oversized list is preferred when every supplied account must be processed, while `.take(MAX)` is suitable only when ignoring surplus accounts is intentional. Standard adapters that cannot increase cardinality, such as `filter`, `map`, and `enumerate`, preserve a preceding `take`; expanding adapters such as `flat_map` must be bounded afterward. The guard must compare `remaining.len()` against an integer literal or resolved constant, return early on the oversized path, and dominate the loop. The analysis follows local aliases and computes loop-carried state to a fixed point. Reassignment, mutable borrows, `&mut self` calls, and closures that may replace a checked binding invalidate its bound, including for later iterations of an enclosing loop. A runtime limit, branch-local check, late check, or opaque helper does not satisfy the rule because it does not establish a source-visible protocol maximum on every path.

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

The check is repository-scoped by source file: `declare_id!` expansions are inspected in crates whose sources live under an `examples` or `security` directory. The crate-root program id is the contract, and every additional declaration is reported at its own call site, so module-scoped `#[allow(...)]` suppresses only the extra declarations (as `examples/declare_program` demonstrates). Library crates that intentionally define no program are ignored.

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

Warns when seed assertions in example instruction paths do not visibly use a byte-string namespace, a named `SEED`/`SEED_*`/`*_SEED` constant, or a generated Pina seed helper.

```rust
const SEED_VAULT: &[u8] = b"vault";
vault.assert_seeds(&[SEED_VAULT, authority.address().as_ref()], &ID)?;
```

Associated seed helpers generated from `#[pda(...)]` are accepted because the macro declaration exposes the namespace at the account type. Receiver-less local functions named like assertion methods are not treated as framework proof. The rule is a reviewability warning and does not replace canonical bump validation.

## Suppression policy

Prefer making validation and bounds explicit instead of suppressing a finding. When a false positive cannot be expressed more clearly, scope `#[allow(...)]` to the smallest item and add a doc comment explaining the invariant. Deny-level security lints should not be disabled at crate or workspace scope.

## Testing

UI fixtures live under `tests/ui/<lint>/`. Each fixture is compiled with the bundled `pina_lint_driver` — the lints are statically linked into the driver — and the emitted diagnostics are compared with the committed `.stderr` file next to the fixture. `PINA_LINT_ONLY` restricts the driver to the lint under test, so each fixture observes the same single-lint behavior the previous one-library-per-lint Dylint setup had.

Fixtures support two directives:

- `// aux-build: <name>.rs` — compile `auxiliary/<name>.rs` first and pass it to the fixture through `--extern`. The auxiliary source chooses its own crate type through `#![crate_type]` (for example proc-macro fixtures); sources without an inner attribute fall back to a plain library.
- `// normalize-stderr-test: "<regex>" -> "<replacement>"` — rewrite the actual stderr before comparing it with the expectation. Paths under the fixture directory are replaced with `$DIR` first, mirroring the convention of the Rust repository's UI tests.

To update a `.stderr` expectation, run the test, copy the saved actual stderr over the `.stderr` file, and re-run. On a mismatch the harness saves the actual stderr to a `pina-lints-ui` directory under the system temp directory and prints the saved path in its failure report.
