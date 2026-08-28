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
const MY_SEED: &[u8] = b"my";

#[macro_export]
macro_rules! my_seeds {
	($authority:expr) => {
		&[MY_SEED, $authority]
	};
	($authority:expr, $bump:expr) => {
		&[MY_SEED, $authority, &[$bump]]
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
- Grouped routed arms: `VariantA | VariantB => SharedAccounts::try_from((program_id, accounts))?.process(data)`
- Accountless arms: `Variant => { let _ = Payload::try_from_bytes(data)?; Ok(()) }`
- Accountless entrypoint fallback: if a single `process_instruction` exists but has no recognizable dispatch map, Pina emits zero-account instruction nodes from the declared payload structs.

Keep in mind:

- Account metadata is only inferred for routed `Accounts::try_from((program_id, accounts))` arms.
- Signer/PDA/default-account inference still depends on direct `self.field.assert_*()` chains inside `impl ProcessAccountInfos`. A field inferred as a PDA must resolve to a declared `#[pda]`; generation fails instead of emitting an incomplete link.
- Writable inference comes from either direct `assert_writable()` chains or mutable `#[derive(Accounts)]` fields such as `&'a mut AccountView`.
- If you hide routing or validation behind helper layers, instruction nodes may still exist, but account metadata becomes less complete.
- Multiple files containing `process_instruction`, malformed or unresolved `#[pda]` attributes, missing package names, and missing unconditional modules are rejected as ambiguous or incomplete inputs.

<!-- {/pinaIdlDispatchSupport} -->

<!-- {@pinaIdlVerificationContract} -->

`test:idl` treats the generated IDL as an API contract. It checks that:

- every example regenerates deterministically into `codama/idls`, `codama/clients/js`, `codama/clients/rust`, and `codama/clients/dart`
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

<!-- {@pinaIdlProgramMetadata} -->

## Canonical on-chain IDLs

The existing `pina idl` generation command also provides explicit lifecycle subcommands:

```text
pina idl generate
pina idl fetch --cluster <CLUSTER> [--program-id <ADDRESS>]
pina idl diff --cluster <CLUSTER> [--program-id <ADDRESS>]
pina idl publish --cluster <CLUSTER> --authority <KEYPAIR>
```

Bare `pina idl [OPTIONS]` is unchanged and remains equivalent to `pina idl generate [OPTIONS]`.

Publication uses the canonical `idl` seed with direct zlib-compressed UTF-8 JSON. Pina validates the complete Codama document and requires its `program.publicKey` to match the target. Network commands always require an explicit cluster.

The transaction planner is the pinned official package `@solana-program/program-metadata@0.9.0`, invoked through an npx-compatible runner without a shell. `npx` may download that exact version when it is not cached. The adapter was cross-checked against upstream commit `33eb527e124cc4a09d8aae448cd306a9bd87db14`.

Use export mode to inspect or multisig-sign every planned transaction without submitting:

```text
pina idl publish --cluster mainnet-beta --authority ./authority.json --export
pina idl publish --cluster mainnet-beta --file ./idl.json \
  --export <MULTISIG_AUTHORITY> --export-encoding base58 --output ./idl-plan.txt
```

The output preserves every `[Transaction #N]` block from the official planner. An export authority is a noop signer; it does not require or accept a local authority/payer secret.

Fetch uses raw mode and locally performs bounded zlib, UTF-8, JSON, Codama-schema, and program-address validation. URL/external-account metadata and alternate encodings fail closed rather than causing an unexpected outbound request.

See the mdBook chapter **Pina CLI → Generate and publish IDLs** for authority, rent, buffer, RPC, multisig, exit-status, and failure-recovery details.

<!-- {/pinaIdlProgramMetadata} -->

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

## Discriminator and payload versioning

| Change                                      | Compatibility impact                                               |
| ------------------------------------------- | ------------------------------------------------------------------ |
| Add a new enum variant                      | Usually backward-compatible if old clients ignore unknown variants |
| Change an existing variant value            | **Breaking** for every historical byte slice                       |
| Reorder or remove struct fields             | **Breaking** (offsets change)                                      |
| Append fields to a struct                   | Mostly non-breaking, but consumers must accept the larger size     |
| Switch primitive width (`u8` → `u16`, etc.) | **Breaking** for serialized payloads at that boundary              |

For on-chain accounts, treat layout as part of protocol ABI:

- Keep field order stable.
- Introduce optional `version` fields at the tail for in-place migration strategies.
- Never change existing discriminator values in place.
- When incompatible layout changes are required, perform explicit migration with a new account version and an operator upgrade flow.

For instruction payloads:

- Prefer additive migration: add a new variant and keep legacy handlers for a release cycle.
- Reject stale payload shapes with explicit errors rather than silently reinterpreting bytes.

<!-- {/pinaDiscriminatorVersionCompatibility} -->
