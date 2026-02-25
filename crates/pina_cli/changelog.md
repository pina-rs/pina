# Changelog

## 0.1.2 (2026-02-25)

### Features

#### Add codama SDK integration tests for IDL client generation.

Generate both Rust and JavaScript clients from pina-generated IDLs using the codama SDK renderers, and verify that the generated code compiles correctly. The test pipeline covers all four example programs (counter_program, escrow_program, hello_solana, transfer_sol) and validates:

- IDL parsing by the codama SDK
- Rust client code generation and compilation (`cargo check`)
- JavaScript client code generation and TypeScript type-checking (`tsc --noEmit`)

#### Extract POD primitive wrappers into a new publishable `pina_pod_primitives` crate and re-export them from `pina` to preserve API compatibility.

Move `pina_codama_renderer` into `crates/`, update generated Rust clients to depend on `pina_pod_primitives`, reuse instruction docs in rendered output, and remove embedded shared primitive modules.

Add `pina codama generate` for end-to-end Codama IDL/Rust/JS generation with example filtering and configurable JS renderer command.

Expand Codama verification to all examples, move the pnpm workspace to repository root, add CLI snapshot tests with `insta-cmd`, and enforce deterministic regeneration checks for IDLs and generated clients.

### Fixes

#### Added a new `pina init` command to scaffold a starter Pina program project.

The command now:

- Creates a new project directory (default `./<name>`) with `Cargo.toml`, `src/lib.rs`, `README.md`, and `.gitignore`.
- Provides a `--path` option to control destination.
- Provides a `--force` option to overwrite scaffold files when they already exist.

The generated project includes a minimal no-std Pina program skeleton with entrypoint wiring and an `Initialize` instruction.

#### Documentation and release-quality updates across crates:

- Standardized crate README badges to explicitly show crates.io and docs.rs links with current versions.
- Added a dedicated `pina_sdk_ids` crate README with crates.io/docs.rs badges and switched the crate manifest to use it.
- Added workspace coverage tooling with `coverage:all` and a CI `coverage` workflow that produces an LCOV artifact and uploads to Codecov.

#### Improved release and security hardening with additional example/test coverage:

- Added `cargo-deny` and `cargo-audit` tooling plus `security:deny`, `security:audit`, and `verify:security` commands.
- Added a CI security job and a dependency policy (`deny.toml`) for license/source/dependency-ban enforcement.
- Hardened release workflows by validating `pina_cli` release tags against `crates/pina_cli/Cargo.toml` and scoping binary builds to the `pina_cli` package.
- Expanded docs publishing triggers to include docs changes on `main` and added docs verification in the Pages workflow.
- Added a new `todo_program` example, generated Codama IDL output, and Rust snapshot tests to keep generated IDLs aligned with committed `codama/idls/*.json` artifacts.

### Documentation

- Fix markdown JS snippet import ordering so `dprint` formatting checks pass in CI.
- Refresh the `pina_cli` crate README with command documentation for `pina idl`/`pina init`, library API usage, and Codama invocation examples for both local and external projects.

## 0.1.1 (2026-02-20)

### Features

#### Add automated release workflow for pina CLI binary distribution.

Register `pina_cli` as a knope-managed package with versioning, changelogs, and GitHub releases. Add a GitHub Actions workflow that builds and uploads cross-platform binaries when a `pina_cli` release is created. Supports 9 target platforms: Linux (GNU/musl, x86_64/aarch64), macOS (x86_64/aarch64), Windows (x86_64/aarch64), and FreeBSD (x86_64). Each binary includes SHA512 checksums for verification.

#### Add the `pina_cli` crate for automatic Codama IDL generation from Pina program source code.

The crate provides both a library API (`generate_idl()`) and a CLI binary (`pina`) with subcommands (starting with `pina idl`) that parse Rust source files using `syn`, extract program structure (accounts, instructions, errors, PDAs, discriminators), and produce [Codama](https://github.com/codama-idl/codama-rs) standard JSON output for client code generation.

**Key features:**

- Parses `declare_id!()`, `#[discriminator]`, `#[account]`, `#[instruction]`, `#[derive(Accounts)]`, and `#[error]` macros/attributes
- Infers `isSigner` and `isWritable` properties from pina's validation chain pattern (`self.field.assert_signer()?.assert_writable()?`)
- Extracts PDA seed information from `macro_rules!` seed macros and byte-string constants
- Resolves known program addresses (system, token, token-2022, ATA) as default values
- Maps pina's Pod types (`PodU64`, `PodBool`, `Address`, etc.) to Codama type nodes
- Produces valid `codama-nodes` v0.7.x `RootNode` JSON with accounts, instructions, PDAs, errors, and discriminators

**CLI usage:**

```sh
pina idl -p examples/counter_program             # stdout
pina idl -p examples/escrow_program -o idl.json  # file output
```

**Library usage:**

```rust
let root = pina_cli::generate_idl(Path::new("examples/counter_program"), None)?;
let json = serde_json::to_string_pretty(&root)?;
```

Snapshot tests verify IDL output for fixture programs and all four example programs (counter, escrow, transfer_sol, hello_solana).
