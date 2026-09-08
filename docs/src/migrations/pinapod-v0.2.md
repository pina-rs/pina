# Migrate Pina to PinaPod v0.2

This guide covers the coordinated PinaPod v0.2 and Pina migration. PinaPod v0.2 breaks Rust source compatibility, but preserves existing fixed and compact wire bytes. Both repositories are maintained together, so the migration removes the v0.1 names instead of carrying aliases.

## Merge and release in dependency order

1. Merge the PinaPod v0.2 pull request after its native, Miri, compile-fail, and benchmark checks pass.
2. Publish PinaPod v0.2 and its mdBook documentation.
3. Replace Pina's temporary path or Git dependency with the published v0.2 version.
4. Update `Cargo.lock` and run Pina's locked test matrix.
5. Regenerate Pina's IDLs and Rust, TypeScript, and Dart clients.
6. Merge the Pina pull request after its generated-artifact and SBF checks pass.

Do not merge Pina first. Published Pina crates must not contain a path dependency, and Pina's CI rejects a manifest-only dependency update because it runs Cargo with `--locked`.

## Update public names

Replace the v0.1 names as one change:

| PinaPod v0.1                 | PinaPod v0.2                          |
| ---------------------------- | ------------------------------------- |
| `ZeroPod`                    | `PinaPod`                             |
| `ZeroPodFixed`               | `PinaPodFixed`                        |
| `ZeroPodCompact`             | `PinaPodCompact`                      |
| `ZeroPodError`               | `PinaPodError`                        |
| `ZeroPodSchema`              | `PinaPod`                             |
| `from_bytes`                 | `read_exact` or `read_prefix`         |
| `from_bytes_mut`             | `read_exact_mut` or `read_prefix_mut` |
| `validate` on a fixed schema | `validate_exact` or `validate_prefix` |

Pina re-exports the new names from `crates/pina/src/lib.rs`. Pina's `#[account]`, `#[instruction]`, and `#[event]` macros inject `#[derive(PinaPod)]`, so application schemas normally do not name the derive.

Manual `PinaPodFixed` implementations are `unsafe`. The implementation must prove the layout, alignment, initialization, and validation invariants documented by PinaPod. Prefer Pina's schema macros.

## Use fixed strings, vectors, and options directly

Fixed Pina schemas now accept bounded collections and recursively fixed options:

```rust
#[account(discriminator = AccountType)]
pub struct Profile {
	pub authority: Address,
	pub display_name: String<32>,
	pub scores: Vec<u64, 8>,
	pub delegate: Option<Address>,
	pub note: Option<String<64>>,
	pub history: Vec<Option<u16>, 16>,
}
```

The fixed layout reserves every field's full capacity. For example, `String<32>` occupies its length prefix plus 32 data bytes even when it is empty. PinaPod zeroes inactive capacity during initialization and after values become shorter. It also validates every active nested value before returning a safe view.

`String<N>` and `Vec<T, N>` are PinaPod schema aliases. Their source is visible to Rust tooling, but the short names keep declarations readable. Use the `Pod*` forms to select a prefix width:

```rust
#[account(discriminator = AccountType)]
pub struct Archive {
	pub title: PodString<1024, 2>,
	pub entries: PodVec<u64, 1024, 2>,
}
```

The final const argument is the prefix width in bytes. It must be `1`, `2`, `4`, or `8`. Do not use `#[pinapod(prefix = u16)]`; v0.2 does not use an attribute to configure container prefixes.

Move the fixed collection fixtures from `tests/ui/fail` to `tests/ui/pass`. Keep runtime tests for empty, full, and over-capacity values, invalid UTF-8, invalid option tags, invalid nested elements, clearing, and replacement.

## Initialize fixed accounts in one pass

Typed fixed-account creation now has two initialization paths:

- `invoke::<T>()` and `invoke_signed::<T>(signers)` write the discriminator and leave every other byte at zero. Use them only when that completed representation is valid.
- `invoke_with::<T>(initialize)` and `invoke_signed_with::<T>(signers, initialize)` run a caller-supplied initializer before PinaPod validates the completed representation. Use them when the account needs nonzero initial values.

Move a create-then-mutate sequence into the creation call:

```rust
CreateProgramAccountWithBump {
	account: self.profile,
	payer: self.authority,
	owner: &ID,
	seeds: &seeds.as_slices(),
	bump,
}
.invoke_with::<Profile>(|profile| {
	profile.authority = *self.authority.address();
	profile.name.try_set("Alice")?;
	profile.scores.try_set([10, 20, 30])?;
	Ok(())
})?;
```

The closure receives `&mut T::Zc` and returns `Result<(), PinaPodError>`. PinaPod zeros the complete account representation, Pina writes the discriminator, the closure configures the remaining fields, and PinaPod validates once at the end. If the closure or validation fails, PinaPod zeros the account bytes again and the create instruction returns `InvalidAccountData`.

This distinction matters for advanced manual `PinaAccount` implementations. A storage enum such as `Ready = 1` has no valid all-zero discriminant, so `invoke::<T>()` fails validation. Set the field inside `invoke_with` instead:

```rust
.invoke_with::<RequiredState>(|state| {
	state.mode = RequiredMode::Ready.into();
	Ok(())
})?;
```

Pina's audited `#[account]` grammar still rejects arbitrary custom enum fields. The enum example applies to a direct PinaPod schema with a manual `PinaAccount` implementation. Use `invoke_with` for ordinary macro-generated accounts too when atomic initialization is clearer than borrowing and mutating the account after creation.

When the payer needs additional PDA signatures, pass them before the initializer:

```rust
builder.invoke_signed_with::<Profile>(&payer_signers, |profile| {
	profile.authority = authority;
	Ok(())
})?;
```

## Load fixed PDA accounts in one pass

Do not validate a fixed PDA with `assert_type`, load it again through the generated `assert_seeds`, and then load it a third time for mutation. Bounded strings and nested containers make each recursive validation meaningful, so repeating the boundary also repeats its compute cost.

Use the one-pass helpers generated for fixed `#[account]` plus `#[pda(bump = ...)]` schemas:

```rust
let mut profile = Profile::load_pda_mut(
	self.profile,
	self.authority.address(),
	&ID,
)?;
profile.name.try_set("Alice")?;
```

`load_pda_mut` checks writability, owner, exact size, discriminator, every active PinaPod value, and the account address derived from the stored bump before it returns the mutable guard. `load_pda` provides the same one-pass contract for immutable access. Both guards retain the runtime data borrow, so drop them before a CPI that can access the account.

Keep `Type::assert_seeds` for a validation-only path that does not need a typed guard. The one-pass loaders are the preferred path when code reads or writes the account immediately afterward.

## Load compact PDA accounts in one borrow

Do not validate a compact PDA with `assert_compact_type`, load its bump through generated `assert_seeds`, and then parse it again with `with_compact_account`.

Use `Type::with_pda` for a compact `#[account]` with `#[pda(bump = ...)]`:

```rust
let (revision, entries) = Journal::with_pda(
	self.journal,
	self.authority.address(),
	&ID,
	|journal| Ok((journal.revision.get(), journal.entries().len())),
)?;
```

`with_pda` validates ownership, compact data, the canonical bump, and the derived account address before it runs the closure. The closure cannot return the borrowed compact view. Use `assert_compact_type` or generated `assert_seeds` only when code needs validation without field access.

## Keep compact nesting inside the supported grammar

A compact account places fixed fields first and compact tails last. It can contain several tails. PinaPod v0.2 accepts these compact forms:

- `String<N>`
- `Vec<T, N>` where `T` has a fixed PinaPod representation
- `Option<T>` where `T` has a fixed PinaPod representation
- `Option<String<N>>`
- `Option<Vec<T, N>>` where `T` has a fixed PinaPod representation
- `Vec<String<M>, N>`

`Option<T>` for fixed `T` remains an inline header field. The other forms use compact tail storage. `Vec<String<M>, N>` stores fixed-footprint string elements, so each active element occupies its own prefix plus `M` bytes. The strings can have different logical lengths.

```rust
#[account(discriminator = AccountType, compact)]
pub struct Journal {
	pub authority: Address,
	pub revision: u32,
	pub archived_at: Option<u64>,
	pub title: String<64>,
	pub entries: PodVec<u64, 1024, 2>,
	pub note: Option<String<128>>,
	pub labels: Vec<String<24>, 16>,
}
```

Pina's schema classifier is closed. It rejects unsupported nesting instead of accepting an arbitrary `ZcField` implementation. The diagnostic lists the supported forms.

Compact creation also requires an initial patch. Even a header-only account must make the initialization plan explicit:

```rust
CreateCompactProgramAccountWithBump {
	account: self.journal,
	payer: self.authority,
	owner: &ID,
	seeds: &Journal::seeds(self.authority.address()).as_slices(),
	bump,
	patch: JournalPatch::new()
		.bump(bump)
		.authority(*self.authority.address())
		.revision(0),
	space: Journal::MIN_SIZE,
}
.invoke::<Journal>()?;
```

The builder applies `patch` while it initializes the allocated bytes. Omitted patch fields use their zero, empty, or absent representation. Include nonempty tail replacements in the patch and allocate enough `space` for those encoded values.

## Replace staged compact mutation with a patch

PinaPod v0.1 exposed a mutable view whose setters accumulated changes before `commit()`. The caller also had to order `ReallocCompactAccount` differently for growth and shrink. Remove that lifecycle.

Build a generated patch and pass it to Pina's typed update operation:

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

`JournalPatch` distinguishes an unchanged field from a field set to an empty or absent value. A method named after a fixed field replaces that field. A `replace_*` method replaces a collection tail. The patch validates every requested value and computes the target length before Pina changes account bytes or lamports.

`UpdateResizableAccount` ends the data borrow before resizing. It grows the account before writing, or writes a valid shorter representation before shrinking. It clears bytes removed by the update. If preflight validation fails, account bytes and lamport balances remain unchanged.

Use `rent_account` for this builder. The account both funds growth and receives excess rent after a shrink. The lower-level `ReallocAccount`, `ReallocAccountZeroed`, and `ReallocCompactAccount` builders use the same field name and take an explicit `target_size`. Creation and allocation builders retain `payer` because those APIs only fund a new account.

## Keep wire bytes stable

The following source changes do not change existing serialized data:

- Renaming the derive and traits changes Rust names only.
- Replacing fixed read method names changes validation entry points only.
- Replacing create-then-mutate code with `invoke_with` changes initialization order, not the completed fixed-account bytes.
- Replacing `assert_type` plus `assert_seeds` plus `as_account*` with `load_pda*` changes validation order, not account bytes.
- Replacing `assert_compact_type` plus `assert_seeds` plus `with_compact_account` with `with_pda` changes validation order, not account bytes.
- Replacing staged compact mutation with a patch changes how callers produce the same compact bytes.
- Adding client capacity metadata changes validation, not the encoded prefix or payload.

The following schema changes do change layout and require the normal on-chain migration process:

- Adding a new field to an existing account.
- Changing a field's capacity or prefix width.
- Moving a field or changing its fixed versus compact classification.
- Changing a discriminator value or width.

PinaPod pins byte-for-byte v0.1 fixtures. Pina also keeps account, instruction, IDL, and generated-client fixtures so both sides detect an accidental layout change.

## Regenerate clients and enforce capacity

Run Pina's normal IDL and client generation commands after the Rust schemas compile. Compact capacity must be structured schema data, not a number recovered from field documentation. Generated TypeScript and Dart clients enforce each capacity at both boundaries:

- Encoders reject `capacity + 1` before writing a prefix.
- Decoders reject an oversized prefix before allocating or iterating.
- Multi-tail accounts check each field against its own capacity.
- Decoders keep discriminator checks and consume the exact encoded length.

Do not edit files under `codama/clients/*/generated` by hand.

## Verify the migration

Run these commands from the Pina repository:

```sh
devenv shell docs:sync
devenv shell verify:docs
devenv shell cargo test -p pina_root --test ui --locked
devenv shell cargo test -p pina --all-features --locked
devenv shell cargo test -p pina_cli --all-features --locked
devenv shell cargo test -p pina_codama_renderer --locked
devenv shell build:pina:no-default
devenv shell test:idl
devenv shell -- report:cu:compare:main
```

Then run the generated TypeScript and Dart contract suites, the tracked SBF compact-account build, and the Surfpool lifecycle tests. The failure cases must compare both account data and lamport balances before and after the rejected update.

The compute-unit report must show savings as positive values and increases as negative values. Exact runtime increases fail unless `scripts/compute-unit-policy.json` records a reviewed absolute ceiling. See [Compute-unit performance](../compute-unit-performance.md) for the PinaPod v0.2 measurements and methodology.
