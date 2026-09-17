# Project Setup

## Prefer the scaffold

Start a program with the installed CLI so the generated Pina version, feature names, target configuration, and starter code stay aligned:

```sh
pina init counter_program
cd counter_program
pina doctor
pina keys show
pina lint
pina build
pina test --unit
pina test
pina generate
```

SBF compilation delegates to the Agave CLI's `cargo-build-sbf`; install the Agave CLI before the first SBF build. It also installs the `pina_lint_driver` binary below Cargo home on first `pina lint` use; the driver statically links Pina's official lint catalog for the active toolchain. TypeScript client generation also requires Node.js with npm and `npx`. Keep the generated `pina.toml` as the project-local discovery and client-selection contract.

## Configuration blocks

```toml
[project]
program = "."

[clients]
output = "clients"
languages = [
	"cpi",
	"rust",
	"typescript",
] # also dart, cli-rust, cli-ts, cli-dart
mode = "auto"
scaffold = true

[migrations]
version_type = "u8" # u8, u16, or u32; never u64
auto = true # optional: true, false, or ["accounts", "events", "instructions"]

[migrations.answers]
rename = ["value:points"]
assume_removed = []
```

- `[clients]` resolves `languages`, `output`, `mode`, and `scaffold`. Each language can override `output`, `mode`, and `scaffold` under `[clients.cpi]`, `[clients.rust]`, `[clients.typescript]`, `[clients.dart]`, `[clients.cli_rust]`, `[clients.cli_ts]`, and `[clients.cli_dart]` (the kebab-case `[clients.cli-rust]` form is a deprecated alias).
- `pina init` scaffolds `[migrations]` with `version_type = "u8"` and `auto = true`, plus the `build.rs` rerun directive, so migrations are on by default. The manifest is not scaffolded: it binds the history to the declared program address, and a new project still carries the placeholder `declare_id!`. Set the real address, then run `pina migrations make` once to record the version-0 baseline.
- `version_type` chooses the width once and freezes at the first persistent publication; it cannot be widened after release. `auto` opts whole contract kinds in; `[migrations.answers]` persists rename and removal decisions for CI. Prefer `u8`: versions are counted per contract, so its 255-version budget applies to each account, instruction, and event separately.
- A program with a recorded `auto` policy needs a `build.rs` that emits `cargo:rerun-if-changed=migrations/manifest.json`. `pina migrations make` scaffolds it or prints the exact line, and `pina migrations check` fails until it is present. Commit `migrations/` with the source.

Before deployment, establish the program identity explicitly. Use `pina keys new` for a fresh local identity or validate a keypair produced by trusted platform tooling with `pina keys sync --keypair <path>`. Never use `--force` unless the intended operation is an identity rotation.

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

use pina::*;

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
