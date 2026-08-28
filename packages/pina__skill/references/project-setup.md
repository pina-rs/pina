# Project Setup

## Prefer the scaffold

Start a program with the installed CLI so the generated Pina version, feature names, target configuration, and starter code stay aligned:

```sh
pina init counter_program
cd counter_program
pina build
pina test --unit
pina test
pina generate
```

The scaffold pins the nightly toolchain and `rust-src` needed by `pina build`; install its compatible linker with `cargo install sbpf-linker --version 0.1.8 --locked` before the first SBF build. TypeScript client generation also requires Node.js with npm and `npx`. Keep the generated `Pina.toml` as the project-local discovery and client-selection contract.

Use `pina init --help` before selecting a destination or replacing existing scaffold files. The command preserves existing files unless the user explicitly supplies `--force`; inspect the destination before using that flag.

## Expected boundaries

A Pina program should keep these concerns separate:

- on-chain instruction processing and account types in a `no_std`-compatible library;
- the SBF entrypoint behind the project's `bpf-entrypoint` feature;
- host tests and VM fixtures in test-only modules or integration tests;
- embedded Surfpool RPC tests in the isolated `tests/surfpool` Cargo package, separate from the fast native loop and SBF dependency graph;
- IDL and generated clients outside hand-written program source.

Do not add a general application framework, async runtime, serializer, or allocator to the on-chain path unless the program's requirements justify it.

## Existing projects

Before adding Pina to an existing crate, inspect its Rust edition, toolchain, Solana dependencies, target configuration, and entrypoint. Prefer the versions and features emitted by the same installed `pina` CLI that will maintain the project. Avoid copying dependency versions from an unrelated repository.

The usual structure is:

```rust
#![cfg_attr(not(test), no_std)]

use pina::prelude::*;

nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: ProgramInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		ProgramInstruction::Initialize => InitializeAccounts::try_from(accounts)?.process(data),
	}
}
```

Adapt names and dispatch arms to the program; do not introduce a generic router when a direct match remains clear.

## Feature discipline

- Keep `pina` default features off when the project uses an explicit minimal feature set.
- Enable token or Token-2022 support only for programs that call those APIs.
- Compile tests without the on-chain entrypoint when the project follows the common library-testing pattern.
- Build the deployable program with the repository's pinned SBF toolchain and linker configuration.
