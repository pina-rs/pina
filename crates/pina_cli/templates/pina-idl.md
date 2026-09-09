<!-- {@pinaIdlCanonicalExamples} -->

### Multi-file layout

```rust
// src/lib.rs
use pina::*;

mod accounts;
mod instructions;
mod pda;
mod state;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
```

### Canonical dispatch

```rust
#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let ix: MyInstruction = parse_instruction(program_id, &ID, data)?;

		// Prefer one routed arm per variant when possible.
		match ix {
			MyInstruction::Initialize => {
				InitializeAccounts::try_from((program_id, accounts))?.process(data)
			}
			MyInstruction::Update => {
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}
```

### Grouped dispatch with shared accounts

```rust
match ix {
	MyInstruction::Initialize => InitializeAccounts::try_from((program_id, accounts))?.process(data),
	MyInstruction::Toggle | MyInstruction::Update => {
		UpdateAccounts::try_from((program_id, accounts))?.process(data)
	}
}
```

### Accountless dispatch

```rust
match ix {
	MyInstruction::Ping => {
		let _ = PingInstruction::try_from_bytes(data)?;
		Ok(())
	}
	MyInstruction::Initialize => InitializeAccounts::try_from((program_id, accounts))?.process(data),
}
```

### Validation chains

```rust
impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let args = InitializeInstruction::try_from_bytes(data)?;
		let seeds = my_seeds!(self.authority.address().as_ref(), args.bump);

		self.authority.assert_signer()?;
		self.system_program.assert_address(&system::ID)?;
		self.token_program.assert_address(&token::ID)?;
		self.ata_program
			.assert_address(&associated_token_account::ID)?;
		self.state
			.assert_empty()?
			.assert_writable()?
			.assert_seeds_with_bump(seeds, &ID)?;

		Ok(())
	}
}
```

### PDA seed helpers

```rust
const SEED_MY: &[u8] = b"my";

#[macro_export]
macro_rules! my_seeds {
	($authority:expr) => {
		&[SEED_MY, $authority]
	};
	($authority:expr, $bump:expr) => {
		&[SEED_MY, $authority, &[$bump]]
	};
}
```

### Discriminators and account layouts

```rust
#[discriminator]
pub enum MyInstruction {
	Initialize = 0,
	Update = 1,
}

#[discriminator]
pub enum MyAccountType {
	MyState = 1,
}

#[instruction(discriminator = MyInstruction::Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[instruction(discriminator = MyInstruction::Update)]
pub struct UpdateInstruction {
	pub value: PodU64,
}

#[account(discriminator = MyAccountType)]
pub struct MyState {
	pub bump: u8,
	pub value: PodU64,
}
```

<!-- {/pinaIdlCanonicalExamples} -->

<!-- {@pinaIdlDispatchSupport} -->

The extractor currently supports these dispatch shapes:

- Canonical routed arms: `Variant => Accounts::try_from((program_id, accounts))?.process(data)`
- Legacy routed arms: `Variant => Accounts::try_from(accounts)?.process(data)`
- Grouped routed arms: `VariantA | VariantB => SharedAccounts::try_from((program_id, accounts))?.process(data)`
- Accountless arms: `Variant => { let _ = Payload::try_from_bytes(data)?; Ok(()) }`
- Accountless entrypoint fallback: if a single `process_instruction` exists but has no recognizable dispatch map, Pina emits zero-account instruction nodes from the declared payload structs.

Keep in mind:

- Account metadata is inferred for both routed conversion forms. New code should use `Accounts::try_from((program_id, accounts))` so optional slots can recognize the executing program's address.
- Signer/PDA/default-account inference still depends on direct `self.field.assert_*()` chains inside `impl ProcessAccountInfos`. A field inferred as a PDA must resolve to a declared `#[pda]`; generation fails instead of emitting an incomplete link.
- Writable inference comes from either direct `assert_writable()` chains or mutable `#[derive(Accounts)]` fields such as `&'a mut AccountView`.
- If you hide routing or validation behind helper layers, instruction nodes may still exist, but account metadata becomes less complete.
- Multiple files containing `process_instruction`, malformed or unresolved `#[pda]` attributes, missing package names, and missing unconditional modules are rejected as ambiguous or incomplete inputs.

<!-- {/pinaIdlDispatchSupport} -->

<!-- {@pinaIdlVerificationContract} -->

`test:idl` treats the generated IDL as an API contract. It checks that:

- every example regenerates deterministically into `codama/idls`, `codama/clients/js`, `codama/clients/rust`, `codama/clients/cpi`, and `codama/clients/dart`
- generated JSON passes Codama's JS validator
- generated JS clients typecheck
- generated Rust clients compile
- generated Dart clients resolve with the lockfile, format cleanly, pass static analysis, and pass codec contract tests
- for every example, generated instruction/account/error counts match the source declarations:
  - `#[instruction]`
  - `#[account]`
  - `#[error]`

That last count-parity check is important because it catches silent extraction regressions where a program still produces valid JSON, but one or more instruction surfaces disappear.

<!-- {/pinaIdlVerificationContract} -->

<!-- {@pinaDiscriminatorLayoutDecisionMatrix} -->

## Discriminator layout decision matrix

The discriminator strategy determines byte layout, parser guarantees, and cross-protocol compatibility.

| Goal                                                                                 | Recommended layout                                                                                                                     |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| Keep layout **minimal and zero-copy** while staying explicit                         | **Current Pina model**: discriminator bytes are the first field inside `#[account]`, `#[instruction]`, and `#[event]` structs.         |
| Preserve compatibility with existing Anchor-account payloads (SHA-256 hash prefixes) | **Legacy adapter model**: custom raw wrapper types parse/write the existing 8-byte external prefix before converting to typed structs. |
| Minimize account size growth when you have many types                                | **Use `u8`** (default) discriminator width.                                                                                            |
| You need more than 256 route variants                                                | **Use `u16` / `u32` / `u64`** by setting `#[discriminator(primitive = ...)]`.                                                          |
| Avoid schema migrations across existing serialized data                              | Keep existing field order and discriminator values; only append fields.                                                                |

### Raw discriminator width by use-case

| Width | Max variants               | Storage cost (bytes) | Recommended when                                              |
| ----- | -------------------------- | -------------------- | ------------------------------------------------------------- |
| `u8`  | 256                        | 1                    | Most programs and instructions                                |
| `u16` | 65,536                     | 2                    | Medium-large routing tables and explicit version partitioning |
| `u32` | 4,294,967,296              | 4                    | Very large enums, rarely needed                               |
| `u64` | 18,446,744,073,709,551,616 | 8                    | Legacy interoperability shims or reserved growth              |

- Discriminator width only affects the first field bytes.
- Widths above 8 are rejected at macro expansion time.
- Wider discriminators improve variant space, but increase CPI payload and account rent by the exact number of bytes.

<!-- {/pinaDiscriminatorLayoutDecisionMatrix} -->

<!-- {@pinaDiscriminatorVersionCompatibility} -->

## Discriminators and ABI migrations

| Change                                           | Compatibility impact                                                     |
| ------------------------------------------------ | ------------------------------------------------------------------------ |
| Add a new discriminator variant                  | Backward-compatible; existing routes keep their identity                 |
| Change an existing discriminator value           | **Breaking** for every historical byte slice                             |
| Change a migration-aware account or payload      | Compatible only when the checked-in history has an adjacent transition   |
| Append optional accounts to an instruction route | Compatible when the existing positional list remains an identical prefix |
| Reorder, remove, or escalate an instruction slot | **Breaking**; create a new instruction discriminator                     |
| Change the migration version width after release | **Breaking** for every migration-aware wire contract                     |

Add `migrations` to an account, instruction, or event attribute to opt into a framework-owned version field. Pina places the field immediately after the discriminator. Set its width once for the program:

```toml
[migrations]
version-type = "u8"
```

Run `pina migrations make` before a release. Pina updates the replaceable draft when the current version is unpublished. After `pina deploy` records a non-local publication, the next schema change creates a new version and adjacent transition. Normal builds run `pina migrations check` and fail on drift, incomplete manual transitions, or changed published code.

An old instruction can omit only newly appended optional accounts. Pina does not synthesize signers, writable privileges, PDAs, or required accounts. Any change to an existing process slot requires a new discriminator.

Historical events are immutable. Generated event decoders validate their exact released shape, project them into current-shape scratch bytes, and retain the source version so consumers can distinguish an absent historical field from an emitted default value.

<!-- {/pinaDiscriminatorVersionCompatibility} -->
