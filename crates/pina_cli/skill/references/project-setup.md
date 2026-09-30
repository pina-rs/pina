# Project Setup

## Prefer the scaffold

Start a program with the installed CLI so the generated Pina version, feature names, target configuration, and starter code stay aligned:

```sh
pina init counter_program
cd counter_program
pina doctor
pina keys new               # replace the shared placeholder declare_id!
pina migrations create --auto true  # track every contract; record version 0 for the real address
pina lint
pina build
pina test --unit
pina test
pina generate
```

The order matters. Every scaffold starts with the same placeholder address, `Fg6PaFpoGXkYsidMpWxTWqkZkkM8NufCHCX9ddLKBqd7`, which nobody can deploy (`pina doctor` warns about it). The scaffold records no migration policy: the policy lives only in `migrations/manifest.json`, which `pina migrations create --auto true` writes. Until that run nothing is tracked, so `pina build` and `pina generate` produce a program and clients without version envelopes; record the baseline before generating clients anyone keeps. Snapshot after `pina keys new`: an unpublished history rebinds to a new address on the next `create`, but it is cleaner to record it once. Add `--version-type u16` (or `u32`) to that first run only if one contract may need more than 255 versions.

SBF compilation delegates to the Agave CLI's `cargo-build-sbf`; install the Agave CLI before the first SBF build. `pina lint` downloads the `pina_lint_driver` binary built for the active nightly on first use, below Cargo home; the driver statically links Pina's official lint catalog. The scaffold's `rust-toolchain.toml` pins the nightly the matching Pina release publishes drivers for, so keep that pin unless you are prepared to build the driver with `pina lint --build-driver` (which needs the `rustc-dev` component, and a nightly whose compiler internals the lint source still compiles against). TypeScript and Dart client generation also require Node.js with npm and `npx`. Keep the generated `pina.toml` as the project-local discovery and client-selection contract.

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

[migrations.answers]
rename = ["value:points"]
assume_removed = []
```

- `[clients]` resolves `languages`, `output`, `mode`, and `scaffold`. Each language can override `output`, `mode`, and `scaffold` under `[clients.cpi]`, `[clients.rust]`, `[clients.typescript]`, `[clients.dart]`, `[clients.cli_rust]`, `[clients.cli_ts]`, and `[clients.cli_dart]` (the kebab-case `[clients.cli-rust]` form is a deprecated alias).
- `pina init` scaffolds the `build.rs` rerun directive and the `account-resize` feature that the reserved `Migrate` route needs, but no `[migrations]` table and no manifest. The manifest binds the history to the declared program address, and a new project still carries the placeholder `declare_id!`. Run `pina keys new`, then `pina migrations create --auto true` once to turn migrations on and record the version-0 baseline.
- The program keypair lives at `target/deploy/<library-name>-keypair.json`, below the git-ignored `target/`, so `cargo clean` deletes it. Back it up (outside the repository) before the first deployment; it is the program's address and, by default, how later upgrades find it.
- `pina.toml` holds no migration policy. The version width and the `auto` policy live only in the manifest: `pina migrations create --version-type <u8|u16|u32>` and `--auto <POLICY>` record them, a run without either flag keeps what is recorded, and the retired `[migrations].version_type` and `[migrations].auto` keys fail every command that reads `pina.toml`, naming the flag that replaces them. The width can change only while nothing is published and cannot be widened after release. Prefer `u8`: versions are counted per contract, so its 255-version budget applies to each account, instruction, and event separately. `[migrations.answers]` persists rename, removal, and manual-conversion decisions for CI.
- A program with a recorded `auto` policy needs a `build.rs` that emits `cargo:rerun-if-changed=migrations/manifest.json`. `pina migrations create` scaffolds it or prints the exact line, and `pina migrations check` fails until it is present. Commit `migrations/` with the source.

Before deployment, establish the program identity explicitly. Use `pina keys new` for a fresh local identity or validate a keypair produced by trusted platform tooling with `pina keys sync --keypair <path>`. Never use `--force` unless the intended operation is an identity rotation.

Use `pina init --help` before selecting a destination or replacing existing scaffold files. The command preserves existing files unless the user explicitly supplies `--force`; inspect the destination before using that flag, because `--force` also rewrites `src/lib.rs` back to the placeholder `declare_id!`.

Package names must start with a letter or `_`, use only ASCII letters, digits, `_`, and `-`, stay within 64 characters, and not be a Rust keyword: the name becomes the crate name of the program and of every generated client.

The scaffold declares no `[workspace]`. Inside an existing Cargo workspace, `pina init` prints the enclosing manifest; add the program to that workspace's `members`, and add the dependencies generated Rust clients inherit to its `[workspace.dependencies]`.

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
#![no_std]

use pina::*;

nostd_entrypoint!(process_instruction);

fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: ProgramInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		ProgramInstruction::Initialize => {
			InitializeAccounts::try_from((program_id, accounts))?.process(data)
		}
	}
}
```

Pass `(program_id, accounts)`: that is the only `try_from` form the accounts derive implements, and the form the IDL extractor recognizes when it infers account metadata. A `pina init` scaffold keeps `nostd_entrypoint!` in `src/entrypoint.rs` behind the `bpf-entrypoint` feature rather than at the top level; follow whichever layout the project already uses.

Adapt names and dispatch arms to the program; do not introduce a generic router when a direct match remains clear.

`no_std` is unconditional rather than `#![cfg_attr(not(test), no_std)]`: `nostd_entrypoint!` already expands to the panic handler and allocator the BPF target needs, and it expands away on the host where the standard library is linked anyway. A crate that also builds a host `cdylib` may need `std` for unwinding, and that case is what the conditional `extern crate std` below is for — not the `no_std` attribute itself:

```rust
#[cfg(all(
	not(any(target_os = "solana", target_arch = "bpf")),
	not(feature = "bpf-entrypoint"),
	not(test)
))]
extern crate std;
```

Add that block only when a host `cdylib` build actually requires it; a plain library does not.

## Feature discipline

- Keep `pina` default features off when the project uses an explicit minimal feature set.
- Enable token or Token-2022 support only for programs that call those APIs.
- Compile tests without the on-chain entrypoint when the project follows the common library-testing pattern.
- Build the deployable program with the repository's pinned SBF toolchain and linker configuration.
