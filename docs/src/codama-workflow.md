# Codama Workflow

This repository uses Codama as the IDL and client-generation layer for Pina programs.

The flow has three stages:

1. Generate Codama JSON from Rust programs (`pina idl`).
2. Validate generated JSON against committed fixtures/tests.
3. Render clients (JS and Dart with Codama renderers, Rust with `pina_codama_renderer`).

## In This Repository

Generate and validate the whole workspace flow with `devenv` scripts:

<!-- {=codamaWorkflowCommands} -->

```bash
# Generate Codama IDLs for all examples.
codama:idl:all

# Generate Rust + JS + Dart clients.
codama:clients:generate

# Generate IDLs + Rust/JS/Dart clients in one command.
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
- `scripts/verify-codama-idls.sh`: regenerates IDLs/clients, verifies fixtures and generated clients with Rust, JS, and Dart tests, and enforces deterministic no-diff output (including untracked files).

The generated Dart package lives in `codama/clients/dart`. It exposes one package-root library per example, pins dependency resolution in `pubspec.lock`, and checks all 20 example IDLs as a single inventory. CI runs `dart format`, `dart analyze --fatal-infos`, and `dart test` over the checked-in output.

### Solana Kit dependencies

Pina uses the published `codama-renderers-dart@0.5.1` renderer and Solana Kit Dart packages at `^0.8.0`. These releases include schema normalization, package exports, discriminator enforcement, exact instruction decoding, capacity-aware account decoding, fixed-capacity overflow rejection, canonical boolean, option, and UTF-8 codecs, and wide-enum support.

The generated-client contract suite verifies those behaviors directly. Dependency upgrades must continue to pass the same byte-level contracts without patches, Git overrides, or generated-source rewrites.

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

Generated clients are treated as an untrusted boundary. The checked-in contract suite requires fixed-capacity encoders to reject overflow and requires decoders to enforce discriminators, canonical boolean and option tags, exact top-level lengths, and embedded NUL preservation in semantic strings. Fixed byte arrays are intentionally opaque: generated clients preserve their bytes exactly, while the on-chain program's checked helpers validate any application-level length, UTF-8, or element-count convention inside them. A renderer version that cannot satisfy those contracts is rejected instead of producing a client with a different wire format.

### 3. Generate Dart clients with Codama

`pina codama generate` renders the same IDLs into the checked-in Dart package at `codama/clients/dart`. Each program has a package-root entrypoint, for example:

```dart
import 'package:pina_codama_clients/profile_program.dart';
```

Run `codama:test` to resolve the committed lockfile, format the generated Dart, analyze it with fatal infos, and execute the byte-level contract tests.

### 4. Generate Pina-style Rust clients (optional)

This repository ships `crates/pina_codama_renderer`, which emits Rust models aligned with Pina's discriminator-first, fixed-size POD layouts.

```bash
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

You can pass multiple `--idl` flags or `--idl-dir`.

## Renderer Constraints

`pina_codama_renderer` intentionally targets fixed-size layouts. Unsupported patterns produce explicit errors (for example variable-length strings/bytes, unsupported endian/number forms, and non-fixed arrays).

## Extractor coverage

<!-- {=pinaIdlDispatchSupport} -->

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

For the full checklist and rationale, see [`crates/pina_cli/rules.md`](../../crates/pina_cli/rules.md).

## CI Coverage

<!-- {=pinaIdlVerificationContract} -->

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
