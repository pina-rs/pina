# compact_accounts

<br>

A focused reference for declaring, creating, loading, resizing, and generating clients for a Pina compact account.

## Layout

<br>

<!-- {=compactAccountQuickstart} -->

Compact mode stores a fixed header followed by one or more bounded tails. Enable `compact` for the schema and checked loaders. Enable `account-resize` to apply a patch and adjust rent in one operation:

```toml
[dependencies]
pina = { version = "...", features = ["compact", "account-resize"] }
```

Declare fixed fields first, then the compact tails. `String<N>` uses a one-byte length prefix, and `Vec<T, N>` uses a two-byte length prefix. Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` when the schema needs an explicit prefix width. `PFX` is a byte count and must be `1`, `2`, `4`, or `8`.

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
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

The macro generates `JournalHeader`, `JournalRef`, and `JournalPatch`. It also generates `HEADER_SIZE`, `MAX_SIZE`, checked reads, initialization, projected-size calculation, and atomic updates. Tail prefixes live in the header except for a present `Option<String<N>>` or `Option<Vec<T, N>>`, whose payload retains its own prefix. Each active element of `Vec<String<M>, N>` occupies the fixed `String<M>` footprint, although each string keeps its own logical length.

Pina uses PinaPod for validated alignment-one storage. PinaPod initializes inactive collection capacity and validates each active nested value before Pina returns safe access.

<!-- {/compactAccountQuickstart} -->

The example exercises multiple independent tails, an explicit prefix width, atomic growth and shrink, and validation in generated clients. Its tests compare exact account bytes and rent balances across both successful and rejected updates.

## Create with a patch

The compact create builders require the generated patch, including when the account starts with empty tails:

```rust
CreateCompactProgramAccountWithBump {
	account: self.journal,
	payer: self.authority,
	owner: &ID,
	seeds: &seeds.as_slices(),
	bump,
	patch: JournalPatch::new()
		.bump(bump)
		.authority(*self.authority.address())
		.revision(0),
	space: Journal::HEADER_SIZE,
}
.invoke::<Journal>()?;
```

The patch supplies the initial fixed values and tails in one typed plan. Omitted fields use their zero, empty, or absent representation. Add tail replacements to the patch and increase `space` when the account must start with nonempty tails.

## Size rules

<br>

<!-- {=compactAccountSizeRules} -->

Use the generated patch API to calculate account size. Manual arithmetic duplicates the generated header, option, and element-footprint rules:

```rust
let patch = JournalPatch::new()
	.revision(next_revision)
	.replace_entries(&entries)
	.note(Some("Updated"));

let target_size = Journal::updated_len(current_data, &patch)?;
```

`HEADER_SIZE` is the smallest valid allocation. `MAX_SIZE` is the largest. The exact encoded length depends on the active values:

- `String<N>` contributes its UTF-8 byte length.
- `Vec<T, N>` contributes `len * size_of::<T::Pod>()`.
- An absent dynamic `Option` contributes no tail bytes.
- A present dynamic `Option` contributes its prefix and active payload.
- `Vec<String<M>, N>` contributes `len * size_of::<PodString<M>>()`. Each element has a fixed footprint.

`PinaCompactAccount::validate_size` rejects a buffer outside `HEADER_SIZE..=MAX_SIZE`. Checked loading also validates every length, option tag, active element, and UTF-8 sequence. A `JournalRef` reports `encoded_len()`, `storage_len()`, and `spare_capacity()` without exposing mutable length metadata.

<!-- {/compactAccountSizeRules} -->

## Atomic updates

<br>

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

`UpdateResizableAccount` validates the complete patch and calculates the final encoded length before it changes the account. It then grows before writing or shrinks after writing, adjusts the rent balance through `rent_account`, and clears bytes removed by the patch. If validation or size calculation fails, both account data and lamport balances remain unchanged.

The field is named `rent_account` because the same account funds growth and receives a shrink refund. Existing low-level builders such as `ReallocAccount` keep their `payer` fields.

The generated patch owns the update plan, so callers do not coordinate `set_*`, `commit`, and `ReallocCompactAccount`. Borrow the account for a `JournalRef` only while reading. End that borrow before invoking `UpdateResizableAccount`.

<!-- {/compactAccountResizeOrdering} -->

## Covered use cases

<br>

<!-- {=compactAccountUseCaseChecklist} -->

- Supply the required generated `patch` to every compact create builder, including `JournalPatch::new()` for an all-zero, empty-tail default.
- Create empty compact state with `space: T::HEADER_SIZE`; allocate enough space for any nonempty values included in the initial patch.
- Read through `with_compact_account::<T, _>` or a generated `TRef`.
- Replace fixed fields and several tails in one generated patch.
- Grow or shrink up to `T::MAX_SIZE` through `UpdateResizableAccount`.
- Treat `rent_account` as both the growth funder and the shrink refund recipient.
- Reject unsupported nesting at compile time and reject corrupt prefixes, tags, UTF-8, and elements at load time.
- Keep signer, writable, owner, stored-authority, and canonical-PDA checks explicit. Layout validation does not grant authority.
- Regenerate the IDL and clients after a compact schema changes. Do not edit generated clients by hand.

<!-- {/compactAccountUseCaseChecklist} -->

The native tests cover the size formula, all invalid buffer shapes, empty/partial/full codecs, every instruction codec, revision overflow, and authority-bound PDA derivation. The Surfpool suite deploys the SBF artifact and verifies exact lengths and rent-exempt balances across create, grow, same-size update, shrink, and clear, plus rollback for bounds and authorization failures.

## Run

<br>

```sh
cd examples/compact_accounts
pina test --unit
pina test
pina generate
```
