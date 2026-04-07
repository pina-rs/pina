# Codama Workflow

This repository uses Codama as the IDL and client-generation layer for Pina programs.

The flow has three stages:

1. Generate Codama JSON from Rust programs (`pina idl`).
2. Validate generated JSON against committed fixtures/tests.
3. Render clients (JS with Codama renderers, Rust with `pina_codama_renderer`).

## In This Repository

Generate and validate the whole workspace flow with `devenv` scripts:

<!-- {=codamaWorkflowCommands} -->

```bash
# Generate Codama IDLs for all examples.
codama:idl:all

# Generate Rust + JS clients.
codama:clients:generate

# Generate IDLs + Rust/JS clients in one command.
pina codama generate

# Run the complete Codama pipeline.
codama:test

# Run IDL fixture drift + validation checks used by CI.
test:idl

# Run Quasar SVM generated-client e2e checks alongside LiteSVM.
pnpm run test:quasar-svm
```

<!-- {/codamaWorkflowCommands} -->

Supporting scripts:

- `scripts/generate-codama-idls.sh`: regenerates `codama/idls/*.json` fixtures for all examples.
- `scripts/verify-codama-idls.sh`: regenerates IDLs/clients, verifies fixtures via Rust and JS tests, and enforces deterministic no-diff output.

## In a Separate Project

You do not need to copy this entire repository to use Codama with Pina.

### 1. Generate IDL from your program

```bash
pina idl --path ./programs/my_program --output ./idls/my_program.json
```

### 2. Generate JS clients with Codama

```bash
pnpm add -D codama @codama/renderers-js
```

```js
import { renderVisitor as renderJsVisitor } from "@codama/renderers-js";
import { createFromFile } from "codama";

const codama = await createFromFile("./idls/my_program.json");
await codama.accept(renderJsVisitor("./clients/js/my_program"));
```

### 3. Generate Pina-style Rust clients (optional)

This repository ships `crates/pina_codama_renderer`, which emits Rust models aligned with Pina's discriminator-first, fixed-size POD layouts.

```bash
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

You can pass multiple `--idl` flags or `--idl-dir`.

## Renderer Constraints

`pina_codama_renderer` intentionally targets fixed-size layouts. Unsupported patterns produce explicit errors (for example variable-length strings/bytes, unsupported endian/number forms, and non-fixed arrays).

## Source shapes that extract cleanly

Use the same program shapes described in `crates/pina_cli/rules.md` to keep IDL extraction predictable.

<!-- {=pinaIdlCanonicalExamples} -->

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
		accounts: &[AccountView],
		data: &[u8],
	) -> ProgramResult {
		let ix: MyInstruction = parse_instruction(program_id, &ID, data)?;

		// Add one arm per instruction variant.
		match ix {
			MyInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
			MyInstruction::Update => UpdateAccounts::try_from(accounts)?.process(data),
		}
	}
}
```

### Validation chains

```rust
impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(&self, data: &[u8]) -> ProgramResult {
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

#[instruction(discriminator = MyInstruction, variant = Initialize)]
pub struct InitializeInstruction {
	pub bump: u8,
}

#[instruction(discriminator = MyInstruction, variant = Update)]
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

For the full checklist and rationale, see [`crates/pina_cli/rules.md`](../../crates/pina_cli/rules.md).

## CI Coverage

Codama checks are enforced in the `ci` workflow via `test:idl`.
