# Compact accounts

Compact mode is for discriminator-first accounts with a fixed header and one or more bounded, variable-length tails. It is a good fit when unused collection capacity should not consume rent.

## Declare the layout

<!-- {=compactAccountQuickstart} -->

Compact mode stores a fixed header followed by one or more bounded tails. Enable `compact` for the schema and checked loaders. Enable `account-resize` to apply a patch and adjust rent in one operation:

```toml
[dependencies]
pina = { version = "...", features = ["compact", "account-resize"] }
```

The `compact` feature also enables `derive`. Declare fixed fields first, then the compact tails. `String<N>` uses a one-byte length prefix, and `Vec<T, N>` uses a two-byte length prefix. Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` when the schema needs an explicit prefix width. `PFX` is a byte count and must be `1`, `2`, `4`, or `8`.

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub featured_entry: Option<u64>,
	pub title: String<24>,
	pub entries: Vec<u64, 8>,
	pub markers: PodVec<u8, 8, 8>,
	pub note: Option<String<64>>,
}
```

The compact grammar accepts these tail forms:

- `String<N>`
- `Vec<T, N>` where `T` has a fixed PinaPod representation
- `Option<T>` where `T` has a fixed PinaPod representation
- `Option<String<N>>`
- `Option<Vec<T, N>>` where `T` has a fixed PinaPod representation
- `Vec<String<M>, N>`

`Option<T>` for fixed `T` stays in the header. The other forms use tail storage. A compact schema can contain several tails, but it cannot place a fixed field after the first tail. The macro rejects unsupported nesting and prints the accepted forms in its error.

`N` and `M` accept an integer literal or a `const` item, so a bound declared once is reused everywhere it appears. Fixed `[T; N]` fields and instruction arguments accept the same forms. A constant may be written in terms of other constants and may live in any module of the crate. Pina resolves it during expansion and records the number, so the generated constants, the ABI manifest, and the Codama IDL are identical to the literal spelling:

```rust
const MAX_MEMBERS: usize = 24;

#[account(discriminator = AccountType, compact)]
pub struct Roster {
	pub bump: u8,
	pub members: Vec<Address, MAX_MEMBERS>,
	pub slots: [u8; MAX_MEMBERS],
}
```

A capacity Pina cannot evaluate fails the build with a diagnostic naming the expression, rather than reaching the ABI layer as a name it cannot size. An associated constant such as `Bounds::MAX_MEMBERS` is not resolved: declare the bound as a `const` item at the crate root or in a module.

The macro generates `JournalHeader`, `JournalRef`, and `JournalPatch`. It also generates `HEADER_SIZE`, `MIN_SIZE`, `MAX_SIZE`, checked reads, initialization, projected-size calculation, and atomic updates. `MIN_SIZE` equals `HEADER_SIZE`. Tail prefixes live in the header except for a present `Option<String<N>>` or `Option<Vec<T, N>>`, whose payload retains its own prefix. Each active element of `Vec<String<M>, N>` occupies the fixed `String<M>` footprint, although each string keeps its own logical length.

Pina uses PinaPod for validated alignment-one storage. PinaPod initializes inactive collection capacity and validates each active nested value before Pina returns safe access.

When a compact account also declares `#[pda(..., bump = bump)]`, the macro generates two closure-scoped loaders that differ in how they treat the canonical bump. `Type::with_stored_bump_pda` (named `with_pda` before the split that gave the loaders distinct names) derives one address from the stored bump, so it checks owner, compact data, and the stored-bump PDA address while one runtime borrow remains active. `Type::with_checked_pda` searches the seeds for the canonical bump instead, which additionally rejects an account at any other address and a stored bump that is not canonical. That search makes it the only compact loader that rejects a shadow account created at a noncanonical bump, and it costs more compute than the single derivation `with_pda` performs.

<!-- {/compactAccountQuickstart} -->

Dynamic fields must form the final suffix. Capacities accept an integer literal or a `const` item; explicit prefix widths must be integer literals. The macro rejects a fixed field after the first tail and any nesting outside the grammar above.

## Calculate size from elements

<!-- {=compactAccountSizeRules} -->

Use the generated patch API to calculate account size. Manual arithmetic duplicates the generated header, option, and element-footprint rules:

```rust
let patch = JournalPatch::new()
	.revision(next_revision)
	.replace_entries(&entries)
	.note(Some("Updated"));

let target_size = Journal::updated_len(current_data, &patch)?;
```

`MIN_SIZE` and `HEADER_SIZE` are the smallest valid allocation. `MAX_SIZE` is the largest. The exact encoded length depends on the active values:

- `String<N>` contributes its UTF-8 byte length.
- `Vec<T, N>` contributes `len * size_of::<T::Pod>()`.
- An absent dynamic `Option` contributes no tail bytes.
- A present dynamic `Option` contributes its prefix and active payload.
- `Vec<String<M>, N>` contributes `len * size_of::<PodString<M>>()`. Each element has a fixed footprint.

`PinaCompactAccount::validate_size` rejects a buffer outside `MIN_SIZE..=MAX_SIZE`. Checked loading also validates every length, option tag, active element, and UTF-8 sequence. A `JournalRef` reports `encoded_len()`, `storage_len()`, and `spare_capacity()` without exposing mutable length metadata.

<!-- {/compactAccountSizeRules} -->

Prefer logical values in instruction data. Let the generated patch calculate bytes so callers do not need to reproduce header, option, or element-footprint rules.

## Create at the needed size

Use `CreateCompactProgramAccount` when Pina should derive the canonical bump. Pass an ordinary patch to `invoke`, or use `invoke_with_bump` when the patch stores that bump. `CreateCompactProgramAccountWithBump` accepts instruction data only when it matches the canonical bump. The builders perform this validation themselves, so do not call `assert_canonical_bump` or `assert_seeds_with_bump` first.

```rust
CreateCompactProgramAccount {
	account: journal,
	payer: authority,
	owner: &ID,
	seeds: &Journal::seeds(authority.address()).as_slices(),
	space: Journal::HEADER_SIZE,
}
.invoke_with_bump::<Journal, _>(|bump| {
	JournalPatch::new()
		.bump(bump)
		.authority(*authority.address())
		.revision(0)
})?;
```

The typed create builder applies the patch while it initializes the account. Omitted patch fields use their zero, empty, or absent representation. Use `JournalPatch::new()` when all header fields may remain zero and every tail starts empty. To create nonempty tails, add their replacement methods to the patch and allocate enough `space` for the encoded values. `UpdateResizableAccount` can populate or replace several tails later.

## Grow, update, shrink, and clear

<!-- {=compactAccountResizeOrdering} -->

Apply all compact changes through one patch:

```rust
UpdateResizableAccount {
	account: self.journal,
	rent_account: self.authority,
	program_id: &ID,
	patch: JournalPatch::new()
		.revision(next_revision)
		.replace_entries(&entries)
		.note(Some("Updated")),
}
.invoke::<Journal>()?;
```

`UpdateResizableAccount` preflights the patch's structural representation and calculates the final encoded length before it changes the account. It grows the allocation before applying a longer representation. For a shorter representation, it applies the patch before shrinking the allocation. If the allocation stays the same size, the builder skips the resize. It adjusts the rent balance through `rent_account` and clears bytes removed by the patch. A structural or size preflight failure leaves account data and lamports unchanged.

With the `validation` feature, Pina checks application rules on the completed compact representation after applying the patch. Always propagate an update error with `?`; Solana transaction rollback is what restores the previous bytes and any rent moved earlier in the instruction.

Use `invoke_signed::<Journal>(signers)` when `rent_account` is a PDA that must sign the system transfer used for growth. The patch and resize ordering stay the same.

The `rent_account` field has the same meaning across `UpdateResizableAccount`, `ReallocAccount`, `ReallocAccountZeroed`, and `ReallocCompactAccount`: it funds growth and receives a shrink refund. The lower-level builders take an explicit `target_size`; the high-level builder derives it from the patch.

The generated patch owns the update plan, so callers do not coordinate `set_*`, `commit`, and `ReallocCompactAccount`. Use `Journal::with_pda` to read a stored-bump compact PDA without a separate `assert_compact_type` or `assert_seeds` pass. End the closure before invoking `UpdateResizableAccount`.

<!-- {/compactAccountResizeOrdering} -->

Solana limits the per-instruction increase of account data. A schema can declare a larger `MAX_SIZE`, but callers may need multiple transactions when a single growth step would cross that runtime limit.

## Load safely

Immutable reads are closure-scoped so Pinocchio's borrow guard stays alive for the compact view:

```rust
let (revision, count) = Journal::with_pda(
	journal,
	authority,
	&ID,
	|state| Ok((state.revision.get(), state.entries().len())),
)?;
```

`Journal::with_pda` checks the owner, discriminator, size, prefixes, active elements, and the address the stored bump derives before the closure runs, using a single derivation. `Journal::with_checked_pda` searches for the canonical bump instead, which also rejects a shadow account created at a noncanonical bump; use it when an untrusted caller chooses which account the handler loads. Check the signer and stored authority separately because loading an account does not grant authority.

For a compact account without a stored bump, use `with_compact_account::<T, _>`. Use `assert_compact_type::<T>` only when code validates the account without reading its fields. Calling it before either loader repeats the complete compact-data validation.

## Codama clients

`pina idl` emits compact tails with their prefix and capacity metadata. `pina generate` creates Rust, TypeScript, and Dart clients that reject over-capacity values at encode and decode boundaries. Client applications submit logical values and let the generated patch calculate account bytes.

## Use-case checklist

<!-- {=compactAccountUseCaseChecklist} -->

- Supply the required generated `patch` to every compact create builder, including `JournalPatch::new()` for an all-zero, empty-tail default.
- Create empty compact state with `space: T::MIN_SIZE`; allocate enough space for any nonempty values included in the initial patch.
- Read an ordinary compact account through `with_compact_account::<T, _>`.
- Read a stored-bump compact PDA through its generated `Type::with_stored_bump_pda` helper, or `Type::with_checked_pda` when an untrusted caller chooses which account the handler loads.
- Replace fixed fields and several tails in one generated patch.
- Grow or shrink up to `T::MAX_SIZE` through `UpdateResizableAccount`.
- Treat `rent_account` as both the growth funder and the shrink refund recipient.
- Reject unsupported nesting at compile time and reject corrupt prefixes, tags, UTF-8, and elements at load time.
- Keep signer, writable, and stored-authority checks explicit. `Type::with_stored_bump_pda` covers owner, layout, and stored-bump PDA address validation; `Type::with_checked_pda` adds the canonical bump search.
- Regenerate the IDL and clients after a compact schema changes. Do not edit generated clients by hand.

<!-- {/compactAccountUseCaseChecklist} -->

The complete [`compact_accounts_program`](https://github.com/pina-rs/pina/tree/main/examples/compact_accounts_program) example includes unit coverage and isolated Surfpool tests for creation, nonempty initialization, growth, same-size mutation, maximum capacity, shrink, clear, rent adjustment, and rejected authorization and bounds cases.
