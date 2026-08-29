---
pina: breaking
pina_cli: breaking
pina_skill: breaking
---

# Adopt struct-based CPI instruction builders

Replace Pina's account-management free functions with documented instruction builders. This is a breaking API change: callers now construct the operation, then choose `.invoke()` or `.invoke_signed(signers)` at the call site.

The new API mirrors Pinocchio's CPI builders, keeps every input visible, and allows programs to forward additional PDA signers without needing a second set of helper functions. Account builders that derive a target PDA automatically append that PDA's signer to any additional signers supplied by the caller.

## Migration map

| Before                                       | After                                                |
| -------------------------------------------- | ---------------------------------------------------- |
| `create_account(...)`                        | `CreateAccount { ... }.invoke()`                     |
| `create_program_account::<T>(...)`           | `CreateProgramAccount { ... }.invoke::<T>()`         |
| `create_program_account_with_bump::<T>(...)` | `CreateProgramAccountWithBump { ... }.invoke::<T>()` |
| `allocate_account(...)`                      | `AllocateAccount { ... }.invoke()`                   |
| `allocate_account_with_bump(...)`            | `AllocateAccountWithBump { ... }.invoke()`           |
| `realloc_account(...)`                       | `ReallocAccount { ... }.invoke()`                    |
| `realloc_account_zero(...)`                  | `ReallocAccountZeroed { ... }.invoke()`              |
| `close_account(...)`                         | `CloseAccount { ... }.invoke()`                      |
| `close_account_zeroed(...)`                  | `CloseAccountZeroed { ... }.invoke()`                |

The raw `space` field on create and allocate builders is now `u64`, matching the System Program ABI. `new_size` on reallocation builders remains `usize`, matching `AccountView::resize`.

## Create a regular account

Before:

```rust
create_account(payer, new_account, 128, &program_id)?;
```

After:

```rust
CreateAccount {
	from: payer,
	to: new_account,
	space: 128,
	owner: &program_id,
}
.invoke()?;
```

Use `.invoke_signed(signers)` instead when another CPI account, such as the funding account, must sign through program-derived seeds. Minimum rent is still read and calculated internally.

## Create a typed canonical PDA

The account type parameter moves from the free function to the invocation method. Canonical derivation still validates the target account and returns its address and bump.

Before:

```rust
let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
let (address, bump) = create_program_account::<EscrowState>(
	escrow_account,
	payer,
	&program_id,
	seeds,
)?;
```

After:

```rust
let seeds: &[&[u8]] = &[b"escrow", authority.address().as_ref()];
let (address, bump) = CreateProgramAccount {
	account: escrow_account,
	payer,
	owner: &program_id,
	seeds,
}
.invoke::<EscrowState>()?;
```

For a caller-provided bump, migrate to `CreateProgramAccountWithBump`:

```rust
CreateProgramAccountWithBump {
	account: escrow_account,
	payer,
	owner: &program_id,
	seeds,
	bump,
}
.invoke::<EscrowState>()?;
```

Both typed builders allocate exactly `T::SIZE` bytes and initialize `T`'s discriminator after successful allocation. The explicit-bump variant verifies that the supplied seeds and bump derive the target account before moving lamports.

## Allocate an untyped PDA

Before:

```rust
let (address, bump) = allocate_account(vault, payer, 64, &program_id, seeds)?;
```

After:

```rust
let (address, bump) = AllocateAccount {
	account: vault,
	payer,
	space: 64,
	owner: &program_id,
	seeds,
}
.invoke()?;
```

Use `AllocateAccountWithBump { ..., bump }` when the bump is already available and must be checked. Allocation preserves the existing behavior for prefunded addresses: it tops up rent when needed, allocates the requested bytes, and assigns the owner. Callers remain responsible for initializing untyped data.

## Reallocate an account

Before:

```rust
realloc_account(account, new_size, payer, &program_id)?;
realloc_account_zero(account, new_size, payer, &program_id)?;
```

After:

```rust
ReallocAccount {
	account,
	payer,
	new_size,
	program_id: &program_id,
}
.invoke()?;

ReallocAccountZeroed {
	account,
	payer,
	new_size,
	program_id: &program_id,
}
.invoke()?;
```

Both variants continue to validate writability and ownership, balance rent when growing or shrinking, enforce the runtime's single-instruction growth limit, and rely on the runtime to zero newly allocated bytes. Use `.invoke_signed(signers)` when a growth transfer requires PDA signer seeds.

## Close an account

Before:

```rust
close_account(account, recipient)?;
close_account_zeroed(account, recipient)?;
```

After:

```rust
CloseAccount { account, recipient }.invoke()?;
CloseAccountZeroed { account, recipient }.invoke()?;
```

Close builders expose only `.invoke()`: closing is a direct, checked mutation of the supplied program account rather than a CPI, so signer seeds would have no effect. Prefer `CloseAccountZeroed` when stale account bytes are relevant to the program's threat model.

## Generated CPI modules

Generated instruction account structs now expose documented public fields and are invoked directly. Free convenience constructors are no longer generated.

Before:

```rust
update(accounts, new_price).invoke_signed(&program, signers)?;
```

After:

```rust
instructions::Update {
	accounts,
	new_price,
}
.invoke_signed(&program, signers)?;
```

This applies to newly generated projects as well as the examples shipped in the workspace.
