# Compact Accounts

Compact mode is for discriminator-first accounts with a fixed header and one or more bounded, variable-length tails. It is a good fit when unused collection capacity should not consume rent.

## Declare the layout

<!-- {=compactAccountQuickstart} -->

Compact mode is opt-in. Enable `compact` for schemas and checked loaders; add `account-resize` when using the typed creation and rent-adjusting reallocation builders:

```toml
[dependencies]
pina = { version = "...", features = ["compact", "account-resize"] }
```

The `compact` feature also enables `derive`. Add `compact` to an account with a suffix of one or more bounded `String` or `Vec` fields. Fixed fields, including `Option<scalar>` values stored as `PodOption`, must come first. Every dynamic capacity must be a literal so the macro can audit and generate the maximum layout:

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub featured_entry: Option<u64>,
	pub title: PodString<24>,
	pub entries: Vec<u64, 8>,
	pub markers: PodVec<u8, 8, 8>,
}

let account_bytes = Journal::projected_bytes(
	title.len(),
	active_entry_count,
	active_marker_count,
)?;
```

The macro generates `JournalHeader`, `JournalRef`, and `JournalMut`, plus `HEADER_SIZE`, `MIN_SIZE`, `MAX_SIZE`, one `*_CAPACITY` constant per tail, checked size/load/initialize methods, and the compact-account traits used by Pina's typed CPI builders. For this schema, `TITLE_CAPACITY` is 24 and `ENTRIES_CAPACITY` and `MARKERS_CAPACITY` are both eight. Every tail length is stored in the fixed header. Active UTF-8 bytes and vector elements are concatenated after that header in declaration order; declared capacity is a validation bound, not reserved space.

Pina uses Pinapod, its maintained and wire-compatible ZeroPod fork. Each immutable and mutable accessor reads its own length prefix, so compact tails may have independent active lengths.

<!-- {/compactAccountQuickstart} -->

Dynamic fields must form the final suffix. Each must be `String<N>`, `PodString<N, PFX>`, `Vec<T, N>`, or `PodVec<T, N, PFX>` with literal capacity and prefix values. Vector elements must use one of the audited scalar, address, or fixed-array types accepted by the macro. Fixed fields after the first dynamic field and nested dynamic collections are rejected at compile time.

## Calculate size from elements

<!-- {=compactAccountSizeRules} -->

For compact string and vector tails, the exact encoded size is:

```text
HEADER_SIZE + Σ(active_string_bytes[i]) + Σ(active_vec_count[i] × size_of::<T[i]::Pod>())
```

| State              |                                                                         Account data length |
| ------------------ | ------------------------------------------------------------------------------------------: |
| Every tail empty   |                                                                               `HEADER_SIZE` |
| Mixed tail lengths | `HEADER_SIZE + Σ(active_string_bytes[i]) + Σ(active_vec_count[i] × size_of::<T[i]::Pod>())` |
| Every tail full    |                                                                                  `MAX_SIZE` |

Use the generated APIs rather than repeating this formula:

```rust
let target_size = Journal::projected_bytes(title.len(), entries_count, markers_count)?;
let allocated_size = account.data_len();
let encoded_size = journal.encoded_size();
let staged_size = journal.projected_size();
```

- `account.data_len()` is the physical allocation and rent basis.
- `encoded_size()` is the committed logical size from the stored tail lengths.
- `projected_size()` on a mutable view includes staged `set_*` replacements.
- `Journal::projected_bytes(...)` validates each requested count independently and returns the target logical size before a view exists.

`PinaCompactAccount::validate_size` rejects a buffer smaller than the shared header, larger than `MAX_SIZE`, or split across the greatest common byte alignment of all tail element types (`TAIL_ALIGNMENT`). Aligned spare bytes inside those bounds are permitted during grow-before-commit workflows, so `encoded_size()` can be smaller than `account.data_len()`. Checked loading validates every encoded length against its declared capacity and verifies that each active payload fits in the physical buffer. `commit()` returns the exact encoded size to use when shrinking.

<!-- {/compactAccountSizeRules} -->

Prefer element counts in instruction data. Turning arbitrary byte lengths into a public API makes callers reason about the header and allows them to request split-element sizes. The generated `projected_bytes(...)` method keeps the wire API aligned with the schema and validates each tail's declared capacity.

## Create at the needed size

Use `CreateCompactProgramAccount` when Pina should derive the canonical bump, or `CreateCompactProgramAccountWithBump` when a checked instruction argument already carries the bump. Both builders accept any valid initial compact size:

```rust
CreateCompactProgramAccountWithBump {
	account: journal,
	payer: authority,
	owner: &ID,
	seeds: &Journal::seeds(authority.address()).as_slices(),
	bump,
	space: Journal::projected_bytes(
		initial_title.len(),
		initial_entry_count,
		initial_marker_count,
	)?,
}
.invoke::<Journal>()?;
```

Initialize and commit the compact view after creation. A nonempty initial tail must be written completely; do not treat freshly allocated bytes as initialized collection elements.

## Grow, update, shrink, and clear

<!-- {=compactAccountResizeOrdering} -->

Compact mutation has one important ordering rule:

- **Grow:** allocate and fund rent before staging longer tails.
- **Same size:** commit without reallocating.
- **Shrink or clear:** commit shorter tails before truncating bytes and refunding rent.

Use `ResizeCompactAccount` for normal compact updates. It applies that ordering, enforces the exact `target_size`, and skips the physical resize when the allocation is unchanged:

```rust
let target_size = Journal::projected_bytes(title.len(), entries.len(), markers.len())?;

ResizeCompactAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke::<Journal, _>(|data| {
	let mut journal = Journal::try_from_bytes_mut(data)?;
	journal
		.featured_entry
		.set(Some(PodU64::from(featured_entry)));
	journal
		.set_title(title)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.set_entries(entries)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.set_markers(markers)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	let projected_size = journal.projected_size();
	let encoded_size = journal
		.commit()
		.map_err(|_| ProgramError::InvalidAccountData)?;
	debug_assert_eq!(encoded_size, projected_size);

	Ok(())
})?;
```

The callback receives the account bytes under one mutable runtime borrow. Create the concrete mutable view inside the callback so Pinapod can prove that borrowed tail slices outlive the view. Call `commit()` before returning. After the callback returns, `ResizeCompactAccount` reloads the committed view and rejects a size that differs from `target_size`. It then drops the data borrow before shrinking.

Use `invoke_signed` when `rent_account` is a PDA that must sign the system transfer used for growth. The callback contract stays the same:

```rust
ResizeCompactAccount {
	account,
	rent_account,
	target_size,
	program_id,
}
.invoke_signed::<Journal, _>(rent_account_signers, |data| {
	let mut journal = Journal::try_from_bytes_mut(data)?;
	// Stage every string/vector tail, then commit before returning.
	journal.commit().map_err(|_| ProgramError::InvalidAccountData)?;

	Ok(())
})?;
```

Use `ReallocCompactAccount` when you need explicit allocation control, such as reserving temporary spare bytes. `invoke` resizes immediately. You must enforce the compact ordering yourself:

```rust
if target_size > account.data_len() {
	ReallocCompactAccount {
		account,
		rent_account,
		target_size,
		program_id,
	}
	.invoke::<Journal>()?;
}

let encoded_size = {
	let mut data = account.try_borrow_mut()?;
	let mut journal = Journal::try_from_bytes_mut(&mut data)?;
	journal
		.set_title(title)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.set_entries(entries)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.set_markers(markers)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal.commit().map_err(|_| ProgramError::InvalidAccountData)?
};

if encoded_size < account.data_len() {
	ReallocCompactAccount {
		account,
		rent_account,
		target_size: encoded_size,
		program_id,
	}
	.invoke::<Journal>()?;
}
```

`ReallocCompactAccount` checks the current compact type and target allocation. Before an explicit shrink, it also verifies that the retained bytes contain the full committed layout. Both builders preserve rent exemption on growth and return excess lamports to `rent_account` after shrinkage. Scope immutable runtime borrows with `with_compact_account`. Create mutable views under a direct `try_borrow_mut` guard when tail setters borrow instruction-local values through `commit()`.

<!-- {/compactAccountResizeOrdering} -->

Solana limits the per-instruction increase of account data. A schema can declare a larger `MAX_SIZE`, but callers may need multiple transactions when a single growth step would cross that runtime limit.

## Load safely

Immutable reads are closure-scoped so Pinocchio's borrow guard stays alive for the compact view:

```rust
let (revision, count) = journal.with_compact_account::<Journal, _>(
	&ID,
	|state| Ok((state.revision.get(), state.entries().len())),
)?;
```

Validate the account's owner and authorization policy before trusting data. `assert_compact_type::<Journal>` checks owner, discriminator, size, prefix, and active elements, while PDA and signer checks remain the program's responsibility.

## Codama clients

`pina idl` emits compact text as a size-prefixed UTF-8 string and compact vectors as size-prefixed dynamic arrays. `pina generate` then creates configured Rust, TypeScript, and Dart clients without handwritten codecs. `HEADER_SIZE` and `MAX_SIZE` are runtime-side schema constants; client applications should normally submit logical byte and element counts and let the program calculate account bytes.

## Use-case checklist

<!-- {=compactAccountUseCaseChecklist} -->

- Create header-only state with `space: Journal::MIN_SIZE`, or create directly at `Journal::projected_bytes(...)`.
- Load without reallocating through `with_compact_account::<T, _>`.
- Update fixed header fields or replace same-length tail values without changing rent.
- Store optional scalar header values with `Option<T>`; generated views expose their `PodOption` representation without changing the account size.
- Grow or shrink several string and vector tails in one commit; later payloads are shifted to follow earlier payloads.
- Grow up to `T::MAX_SIZE`, funding the rent delta from a writable rent account.
- Shrink to the size returned by `commit()`, including clearing every tail back to `Journal::MIN_SIZE`, and refund excess rent.
- Use generated `*_CAPACITY` constants and `projected_bytes(...)` to reject each tail count beyond capacity before CPI.
- Compare `encoded_size()` with `account.data_len()` when distinguishing committed content from temporary spare allocation.
- Rely on `validate_size` plus checked loaders to reject truncated, oversized, or corrupt data.
- Keep signer, owner, stored-authority, and canonical-PDA checks explicit; compact layout validation does not define an authorization policy.
- Generate the IDL and clients normally. Codama represents string tails as size-prefixed UTF-8 strings and vector tails as size-prefixed dynamic arrays. Their counts are read from the shared header, preserving Pinapod's header-then-payload wire layout.

<!-- {/compactAccountUseCaseChecklist} -->

The complete [`compact_accounts`](https://github.com/pina-rs/pina/tree/main/examples/compact_accounts) example includes unit coverage and isolated Surfpool tests for creation, nonempty initialization, growth, same-size mutation, maximum capacity, shrink, clear, rent adjustment, and rejected authorization and bounds cases.
