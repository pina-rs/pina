# compact_accounts

<br>

A focused reference for declaring, creating, loading, resizing, and generating clients for a Pina compact account.

## Layout

<br>

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

The concrete `Journal` example has a 40-byte header and eight `u64` slots, so its valid sizes are `40 + 8 × entry_count` bytes from 40 through 104. Initialization fills entries with their index, resize preserves the existing prefix and fills new slots the same way, and write changes one active value without reallocating.

## Size rules

<br>

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

## Safe mutation order

<br>

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

## Covered use cases

<br>

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

The native tests cover the size formula, all invalid buffer shapes, empty/partial/full codecs, every instruction codec, revision overflow, and authority-bound PDA derivation. The Surfpool suite deploys the SBF artifact and verifies exact lengths and rent-exempt balances across create, grow, same-size update, shrink, and clear, plus rollback for bounds and authorization failures.

## Run

<br>

```sh
cd examples/compact_accounts
pina test --unit
pina test
pina generate
```
