<!-- {@compactAccountQuickstart} -->

Compact mode is opt-in. Enable `compact` for schemas and checked loaders; add `account-resize` when using the typed creation and rent-adjusting reallocation builders:

```toml
[dependencies]
pina = { version = "...", features = ["compact", "account-resize"] }
```

The `compact` feature also enables `derive`. Add `compact` to an account with a suffix of one or more bounded `Vec` fields. Fixed fields must come first, and every capacity must be a literal so the macro can audit and generate the maximum layout:

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub entries: Vec<u64, 8>,
	pub markers: PodVec<u8, 8, 8>,
}

let account_bytes = Journal::projected_bytes(active_entry_count, active_marker_count)?;
```

The macro generates `JournalHeader`, `JournalRef`, and `JournalMut`, plus `HEADER_SIZE`, `MIN_SIZE`, `MAX_SIZE`, one `*_CAPACITY` constant per tail, checked size/load/initialize methods, and the compact-account traits used by Pina's typed CPI builders. For this schema, `ENTRIES_CAPACITY` and `MARKERS_CAPACITY` are both eight. Every tail length is stored in the fixed header. Active payloads are concatenated after that header in declaration order; declared capacity is a validation bound, not reserved space.

Pina uses Pinapod, its maintained and wire-compatible ZeroPod fork. Each immutable and mutable accessor reads its own length prefix, so compact tails may have independent active lengths.

<!-- {/compactAccountQuickstart} -->

<!-- {@compactAccountSizeRules} -->

For compact tails `Vec<T0, N0>`, `Vec<T1, N1>`, and so on, the exact encoded size is:

```text
HEADER_SIZE + Σ(active_count[i] × size_of::<T[i]::Pod>())
```

| State              |                                Account data length |
| ------------------ | -------------------------------------------------: |
| Every tail empty   |                                      `HEADER_SIZE` |
| Mixed tail lengths | `HEADER_SIZE + Σ(len[i] × size_of::<T[i]::Pod>())` |
| Every tail full    |                                         `MAX_SIZE` |

Use the generated APIs rather than repeating this formula:

```rust
let target_size = Journal::projected_bytes(entries_count, markers_count)?;
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

<!-- {@compactAccountResizeOrdering} -->

Compact mutation has one important ordering rule:

- **Grow:** calculate and validate the target size, call `ReallocCompactAccount` first, then write and `commit` the longer tail.
- **Same size:** write and `commit`; skip the realloc CPI.
- **Shrink or clear:** write and `commit` the shorter tail first, then call `ReallocCompactAccount` with the returned encoded size.

```rust
if target_size > account.data_len() {
	ReallocCompactAccount {
		account,
		payer,
		new_size: target_size,
		program_id,
	}
	.invoke::<Journal>()?;
}

let encoded_size = {
	let mut data = account.try_borrow_mut()?;
	let mut journal = Journal::try_from_bytes_mut(&mut data)?;
	let committed_size = journal.encoded_size();
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
	debug_assert!(committed_size <= account.data_len());

	encoded_size
};

if encoded_size < account.data_len() {
	ReallocCompactAccount {
		account,
		payer,
		new_size: encoded_size,
		program_id,
	}
	.invoke::<Journal>()?;
}
```

`ReallocCompactAccount` checks the current compact type, validates the destination size, and verifies that a shrink retains every active tail before moving rent. It preserves rent exemption on growth and refunds excess lamports to `payer` on shrink. Scope immutable runtime borrows with `with_compact_account`; use a direct `try_borrow_mut` guard when a tail setter must borrow instruction-local values through `commit`.

<!-- {/compactAccountResizeOrdering} -->

<!-- {@compactAccountUseCaseChecklist} -->

- Create header-only state with `space: Journal::MIN_SIZE`, or create directly at `Journal::projected_bytes(...)`.
- Load without reallocating through `with_compact_account::<T, _>`.
- Update fixed header fields or replace same-length tail values without changing rent.
- Grow or shrink several tails in one commit; later payloads are shifted to follow earlier payloads.
- Grow up to `T::MAX_SIZE`, funding the rent delta from a writable payer.
- Shrink to the size returned by `commit()`, including clearing every tail back to `Journal::MIN_SIZE`, and refund excess rent.
- Use generated `*_CAPACITY` constants and `projected_bytes(...)` to reject each tail count beyond capacity before CPI.
- Compare `encoded_size()` with `account.data_len()` when distinguishing committed content from temporary spare allocation.
- Rely on `validate_size` plus checked loaders to reject truncated, oversized, or corrupt data.
- Keep signer, owner, stored-authority, and canonical-PDA checks explicit; compact layout validation does not define an authorization policy.
- Generate the IDL and clients normally. Codama represents every tail as a dynamic array whose count is read from the shared header, preserving zeropod's header-then-payload wire layout.

<!-- {/compactAccountUseCaseChecklist} -->
