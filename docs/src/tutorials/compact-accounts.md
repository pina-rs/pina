# Compact Accounts

Compact mode is for discriminator-first accounts with a fixed header and one or more bounded, variable-length tails. It is a good fit when unused collection capacity should not consume rent.

## Declare the layout

<!-- {=compactAccountQuickstart} -->

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

fn account_size(entry_count: usize) -> Result<usize, ProgramError> {
	if entry_count > 8 {
		return Err(ProgramError::InvalidArgument);
	}

	Ok(Journal::HEADER_SIZE
		+ entry_count * (core::mem::size_of::<PodU64>() + core::mem::size_of::<u8>()))
}
```

The macro generates `JournalHeader`, `JournalRef`, and `JournalMut`, plus `HEADER_SIZE`, `MAX_SIZE`, checked load/initialize methods, and the compact-account traits used by Pina's typed CPI builders. Every tail length is stored in the fixed header. Active payloads are concatenated after that header in declaration order; declared capacity is a validation bound, not reserved space.

Pina uses Pinapod, its maintained and wire-compatible ZeroPod fork. Each immutable and mutable accessor reads its own length prefix, so compact tails may have independent active lengths.

<!-- {/compactAccountQuickstart} -->

Dynamic fields must form the final suffix. Each must be `Vec<T, N>` with a literal capacity, and `T` must be one of the audited scalar, address, or fixed-array element types accepted by the macro. A fixed field after a vector, strings, and nested dynamic collections are rejected at compile time.

## Calculate size from elements

<!-- {=compactAccountSizeRules} -->

For compact tails `Vec<T0, N0>`, `Vec<T1, N1>`, and so on, the exact encoded size is:

```text
HEADER_SIZE + Σ(active_count[i] × size_of::<T[i]::Pod>())
```

| State              |                                Account data length |
| ------------------ | -------------------------------------------------: |
| Every tail empty   |                                      `HEADER_SIZE` |
| Mixed tail lengths | `HEADER_SIZE + Σ(len[i] × size_of::<T[i]::Pod>())` |
| Every tail full    |                                         `MAX_SIZE` |

`PinaCompactAccount::validate_size` rejects a buffer smaller than the shared header, larger than `MAX_SIZE`, or split across the greatest common byte alignment of all tail element types (`TAIL_ALIGNMENT`). Aligned spare bytes inside those bounds are permitted during grow-before-commit workflows. Checked loading validates every encoded length against its declared capacity and verifies that each active payload fits in the physical buffer. `commit()` returns the exact encoded size to use when shrinking.

<!-- {/compactAccountSizeRules} -->

Prefer element counts in instruction data. Turning arbitrary byte lengths into a public API makes callers reason about the header and allows them to request split-element sizes. A small `account_size` helper keeps the wire API aligned with the schema.

## Create at the needed size

Use `CreateCompactProgramAccount` when Pina should derive the canonical bump, or `CreateCompactProgramAccountWithBump` when a checked instruction argument already carries the bump. Both builders accept any valid initial compact size:

```rust
CreateCompactProgramAccountWithBump {
	account: journal,
	payer: authority,
	owner: &ID,
	seeds: &Journal::seeds(authority.address()).as_slices(),
	bump,
	space: account_size(initial_count)?,
}
.invoke::<Journal>()?;
```

Initialize and commit the compact view after creation. A nonempty initial tail must be written completely; do not treat freshly allocated bytes as initialized collection elements.

## Grow, update, shrink, and clear

<!-- {=compactAccountResizeOrdering} -->

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
	journal
		.set_entries(entries)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.set_markers(markers)
		.map_err(|_| ProgramError::InvalidAccountData)?;
	journal
		.commit()
		.map_err(|_| ProgramError::InvalidAccountData)?
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

`pina idl` emits the compact tail as a size-prefixed dynamic array. `pina generate` then creates configured Rust, TypeScript, and Dart clients without handwritten codecs. `HEADER_SIZE` and `MAX_SIZE` are runtime-side schema constants; client applications should normally submit logical counts and let the program calculate account bytes.

## Use-case checklist

<!-- {=compactAccountUseCaseChecklist} -->

- Create header-only state with `space: T::HEADER_SIZE`, or create directly at any valid nonempty size.
- Load without reallocating through `with_compact_account::<T, _>`.
- Update fixed header fields or replace same-length tail values without changing rent.
- Grow or shrink several tails in one commit; later payloads are shifted to follow earlier payloads.
- Grow up to `T::MAX_SIZE`, funding the rent delta from a writable payer.
- Shrink to the size returned by `commit()`, including clearing every tail back to `T::HEADER_SIZE`, and refund excess rent.
- Reject counts beyond capacity before CPI, and rely on `validate_size` plus checked loaders to reject truncated, oversized, or corrupt data.
- Keep signer, owner, stored-authority, and canonical-PDA checks explicit; compact layout validation does not define an authorization policy.
- Generate the IDL and clients normally. Codama represents every tail as a dynamic array whose count is read from the shared header, preserving zeropod's header-then-payload wire layout.

<!-- {/compactAccountUseCaseChecklist} -->

The complete [`compact_accounts`](https://github.com/pina-rs/pina/tree/main/examples/compact_accounts) example includes unit coverage and isolated Surfpool tests for creation, nonempty initialization, growth, same-size mutation, maximum capacity, shrink, clear, rent adjustment, and rejected authorization and bounds cases.
