# Compact Accounts

Compact mode is for discriminator-first accounts with a fixed header and one bounded, variable-length tail. It is a good fit when unused collection capacity should not consume rent.

Enable the macro and resize helpers:

```shell
cargo add pina --features account-resize,derive
```

## Declare the layout

<!-- {=compactAccountQuickstart} -->

Add `compact` to an account with exactly one trailing, bounded `Vec`. The capacity must be a literal so the macro can audit and generate the maximum layout:

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub entries: Vec<u64, 8>,
}

fn account_size(entry_count: usize) -> Result<usize, ProgramError> {
	if entry_count > 8 {
		return Err(ProgramError::InvalidArgument);
	}

	Ok(Journal::HEADER_SIZE + entry_count * core::mem::size_of::<PodU64>())
}
```

The macro generates `JournalHeader`, `JournalRef`, and `JournalMut`, plus `HEADER_SIZE`, `MAX_SIZE`, checked load/initialize methods, and the compact-account traits used by Pina's typed CPI builders. Only the active tail is allocated; the declared capacity is a validation bound, not reserved space.

<!-- {/compactAccountQuickstart} -->

Only the final field may be dynamic. It must be `Vec<T, N>` with a literal capacity, and `T` must be one of the audited scalar, address, or fixed-array element types accepted by the macro. Strings, multiple vectors, a vector followed by another field, and nested dynamic collections are rejected at compile time.

## Calculate size from elements

<!-- {=compactAccountSizeRules} -->

For a compact account with a trailing `Vec<T, N>`, every valid allocation is:

```text
HEADER_SIZE + active_element_count * size_of::<T::Pod>()
```

| State                 |                     Account data length |
| --------------------- | --------------------------------------: |
| Empty tail            |                           `HEADER_SIZE` |
| Partially filled tail | `HEADER_SIZE + len * TAIL_ELEMENT_SIZE` |
| Full tail             |                              `MAX_SIZE` |

`PinaCompactAccount::validate_size` rejects a buffer smaller than the header, larger than `MAX_SIZE`, or split across an element boundary. The encoded length prefix is also validated against both the physical buffer and declared capacity when the account is loaded.

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

`ReallocCompactAccount` checks the current compact type, validates the destination size, preserves rent exemption on growth, and refunds excess lamports to `payer` on shrink. Scope immutable runtime borrows with `with_compact_account`; use a direct `try_borrow_mut` guard when a tail setter must borrow instruction-local values through `commit`.

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
- Grow up to `T::MAX_SIZE`, funding the rent delta from a writable payer.
- Shrink to any valid element boundary, including clearing back to `T::HEADER_SIZE`, and refund excess rent.
- Reject counts beyond capacity before CPI, and rely on `validate_size` plus checked loaders to reject truncated, oversized, misaligned, or corrupt data.
- Keep signer, owner, stored-authority, and canonical-PDA checks explicit; compact layout validation does not define an authorization policy.
- Generate the IDL and clients normally. Codama represents the tail as a size-prefixed dynamic array and generated Rust codecs preserve compact decoding helpers.

<!-- {/compactAccountUseCaseChecklist} -->

The complete [`compact_accounts`](https://github.com/pina-rs/pina/tree/main/examples/compact_accounts) example includes unit coverage and isolated Surfpool tests for creation, nonempty initialization, growth, same-size mutation, maximum capacity, shrink, clear, rent adjustment, and rejected authorization and bounds cases.
