# compact_accounts

<br>

A focused reference for declaring, creating, loading, resizing, and generating clients for a Pina compact account.

## Layout

<br>

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

The concrete `Journal` example has a 58-byte header containing a `PodOption<PodU64>`, plus a 24-byte UTF-8 title capacity, eight `u64` entry slots, and eight `u8` marker slots. Its exact encoded size is `58 + title_len + 8 × entry_count + marker_count` bytes from 58 through 154. The marker tail uses an explicit eight-byte length prefix, exercising wide-prefix client generation with a small declared capacity. Initialization fills both vectors with their index, resize changes vector lengths, rename grows or shrinks the string with exact rent adjustment, and write sets the optional featured entry without reallocating.

## Size rules

<br>

<!-- {=compactAccountSizeRules} -->

For compact string and vector tails, the exact encoded size is:

```text
HEADER_SIZE + Σ(active_string_bytes[i]) + Σ(active_vec_count[i] × size_of::<T[i]::Pod>())
```

| State              |                                Account data length |
| ------------------ | -------------------------------------------------: |
| Every tail empty   |                                      `HEADER_SIZE` |
| Mixed tail lengths | `HEADER_SIZE + Σ(len[i] × size_of::<T[i]::Pod>())` |
| Every tail full    |                                         `MAX_SIZE` |

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

## Safe mutation order

<br>

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
	journal.featured_entry.set(Some(featured_entry));
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

## Covered use cases

<br>

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
- Generate the IDL and clients normally. Codama represents every tail as a dynamic array whose count is read from the shared header, preserving Pinapod's header-then-payload wire layout.

<!-- {/compactAccountUseCaseChecklist} -->

The native tests cover the three-tail size formula, `PodOption` and UTF-8 string round trips, all invalid buffer shapes, empty/partial/full codecs, every instruction codec, revision overflow, and authority-bound PDA derivation. The Surfpool suite deploys the SBF artifact and verifies exact lengths and rent-exempt balances across string and vector creation, growth, same-size updates, shrinkage, and clearing, plus rollback for bounds and authorization failures.

## Run

<br>

```sh
cd examples/compact_accounts
pina test --unit
pina test
pina generate
```
