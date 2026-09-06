---
pina: breaking
pina_cli: feat
pina_codama_renderer: feat
pina_macros: feat
---

# Add First-Class Compact Accounts

Adds checked dynamic account loaders and typed rent-adjusting builders, updates the realloc example, and covers the lifecycle in unit, SBF, generated-client, and Surfpool tests. This is a major release for `pina` so downstream programs can adopt the expanded account-layout contract explicitly.

Compact mode is disabled by default. Enable `compact` for compact schemas and loaders, and combine it with `account-resize` for the typed creation and rent-adjusting reallocation builders:

```toml
[dependencies]
pina = { version = "0.13", features = ["compact", "account-resize"] }
```

Compact mode uses a fixed header and a suffix of bounded vectors. Each vector length lives in the header and only active elements occupy account bytes:

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub bump: u8,
	pub authority: Address,
	pub revision: u32,
	pub entries: Vec<u64, 8>,
	pub markers: Vec<u8, 16>,
}

let account_bytes = Journal::HEADER_SIZE
	+ active_entry_count * core::mem::size_of::<PodU64>()
	+ active_marker_count * core::mem::size_of::<u8>();
Journal::validate_size(account_bytes)?;
```

`PinaCompactAccount::TAIL_ELEMENT_SIZE` is replaced by `TAIL_ALIGNMENT`, the greatest common byte alignment across all dynamic element types.

Pina now uses Pinapod 0.1.0, its maintained and wire-compatible ZeroPod fork, so immutable and mutable accessors both use each compact vector's own active length.

When resizing, allocate before writing a longer tail and commit a shorter tail before refunding its excess rent:

```rust
if target_size > account.data_len() {
	ReallocCompactAccount {
		account,
		payer,
		new_size: target_size,
		program_id: &ID,
	}
	.invoke::<Journal>()?;
}

let encoded_size = {
	let mut data = account.try_borrow_mut()?;
	let mut journal = Journal::try_from_bytes_mut(&mut data)?;
	journal.set_entries(entries).map_err(|_| ProgramError::InvalidAccountData)?;
	journal.set_markers(markers).map_err(|_| ProgramError::InvalidAccountData)?;
	journal.commit().map_err(|_| ProgramError::InvalidAccountData)?
};

if encoded_size < account.data_len() {
	ReallocCompactAccount {
		account,
		payer,
		new_size: encoded_size,
		program_id: &ID,
	}
	.invoke::<Journal>()?;
}
```
