# `pina_cli`

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

<br>

CLI and library for generating Codama IDLs from Pina programs.

The binary name is `pina`.

The complete command reference is published in the [Pina CLI book](https://pina-rs.github.io/pina/cli/index.html).

<!-- {=crateReadmeBadgeRow:"pina_cli"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina**cli-orange?logo=rust)](https://crates.io/crates/pina_cli) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina**cli-1f425f?logo=docs.rs)](https://docs.rs/pina_cli/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

## Installation

<br>

```bash
npm install --global @pina-rs/cli
pina --help
```

Or install through Cargo:

```bash
cargo install pina_cli
```

Or run from source in this repository:

```bash
cargo run -p pina_cli -- --help
```

## Commands

<br>

### `pina build`

Build the discovered program for SBF and publish its IDL.

```bash
pina build
pina build --features logs,cpi --no-default-features
```

Outputs use Cargo's target directory: `deploy/<library>.so` and `idl/<library>.json`.

### `pina generate`

Generate the clients selected in `pina.toml`, or override them for one invocation.

```bash
pina generate
pina generate --client rust --client typescript
```

Rust-only generation does not require Node.js.

### `pina verify`

Build deterministically, compare the artifact with a deployed program, and publish a source record:

```bash
pina build --verify
pina verify check --program-id <ADDRESS> --cluster devnet
pina verify record \
  --program-id <ADDRESS> \
  --cluster devnet \
  --build-record ./target/pina/verifiable/my_program-<HASH>.json \
  --authority ./upgrade-authority.json
```

See the [verification command reference](https://pina-rs.github.io/pina/cli/verify.html) for exit codes, mainnet acknowledgement, multisig export, remote submission, and keypair/RPC safety rules.

### `pina idl`

<br>

Generate a Codama `rootNode` JSON from a Pina program crate.

```bash
# Write to stdout.
pina idl --path ./examples/counter_program

# Write to a file.
pina idl --path ./examples/counter_program --output ./codama/idls/counter_program.json

# Override program name in generated output.
pina idl --path ./examples/counter_program --name my_program_alias

# Emit compact, machine-readable JSON on stdout.
pina idl --path ./examples/counter_program --compact > counter_program.json
```

IDL JSON is the only stdout output when `--output` is omitted. Progress and extraction counts are written to stderr.

### `pina docs`

List bundled reference topics or render one in the terminal.

```bash
pina docs
pina docs pina-overview
pina docs pina-idl
```

### `pina init`

<br>

Scaffold a new project-aware Pina program with `pina.toml`.

```bash
pina init my_program
pina init my_program --path ./programs/my_program --force
```

Generated manifests do not install lint tooling. `pina lint` builds and manages the `pina_lint_driver` binary itself.

### `pina lint`

Discover the current program and run Pina's official security lints. The first invocation installs the `pina_lint_driver` binary built from the `pina_lints` release matching this CLI below Cargo home; the driver statically links every lint and is used as `cargo check`'s `RUSTC_WRAPPER`. Fix mode applies only machine-applicable suggestions. Lint levels honor the `[lints]` table of the project's `pina.toml`.

```bash
pina lint
pina lint --fix
```

### `pina test`

Run native/Mollusk tests quickly, or build SBF and run the generated isolated Surfpool test:

```bash
pina test --unit
pina test
pina test --filter initialize
```

### `pina dev`

Build once, then let Surfpool watch and redeploy the canonical SBF artifact. Development is offline unless an upstream network or credential-free HTTP(S) RPC URL is selected explicitly. Explicit URLs are visible in Surfpool's process arguments, so they must never contain secrets. The first run requires explicit permission to create Surfpool's deployment runbook.

```bash
pina dev --yes # first run; review and commit the generated txtx.yml
pina dev
pina dev --network devnet
```

### `pina profile`

<br>

Static CU profiler for compiled SBF programs.

```bash
pina profile
pina profile target/deploy/my_program.so
pina profile target/deploy/my_program.so --json
pina profile target/deploy/my_program.so --output report.json
```

When no binary is supplied, Pina discovers the current project's canonical deploy artifact. Output publication rejects aliases, hardlinks, symbolic links, and reparse points that could overwrite the input program.

### `pina deploy`

Plan an explicit deployment without contacting a cluster:

```bash
pina deploy --project ./programs/my_program --cluster devnet \
  --upgrade-authority ./keys/devnet-authority.json \
  --payer ./keys/devnet-payer.json --dry-run --json
```

Every remote write requires confirmation or `--yes`; named mainnet and custom remote endpoints also require `--allow-mainnet`. Query-bearing RPC URLs are rejected because the external Agave `solana` executable receives its endpoint through process arguments. Keypair reads are size-bounded and, on Unix, require owner-private permissions. The program keypair is validated against `declare_id!` before planning and revalidated immediately before deployment.

### `pina codama generate`

<br>

Generate Codama IDLs and Rust/JavaScript/Dart clients from one or more example program crates.

```bash
pina codama generate
pina codama generate --example counter_program --example todo_program
```

The combined command keeps Codama's ergonomic JavaScript string and array types, then adds Pina-specific runtime validation at the generated client's wire boundary. Over-capacity values fail instead of being truncated; discriminators, booleans, and UTF-8 are checked during decoding.

## Library API

<br>

`pina_cli` can also be embedded directly:

```rust
use std::path::Path;

let root = pina_cli::generate_idl(Path::new("./examples/counter_program"), None)?;
println!("{}", serde_json::to_string_pretty(&root)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Codama Workflow

<br>

1. Generate IDL with `pina idl`.
2. Feed the JSON into Codama renderers.

### JavaScript clients in another project

<br>

```bash
pina idl --path ./programs/my_program --output ./idls/my_program.json
pnpm add -D codama @codama/renderers-js
```

```js
import { renderVisitor as renderJsVisitor } from "@codama/renderers-js";
import { createFromFile } from "codama";

const codama = await createFromFile("./idls/my_program.json");
await codama.accept(renderJsVisitor("./clients/js/my_program"));
```

The stock visitor emits the correct fixed-size wire layout, but its generic codecs are intentionally permissive. Use `pina codama generate` when you want the generated JavaScript client to enforce the same canonical zeropod values as the on-chain program.

### Pina-style Rust clients

<br>

This repository includes `crates/pina_codama_renderer`, which renders discriminator-first, zeropod-validated Rust client models from Codama JSON.

```bash
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

## Parser Expectations

<br>

The IDL parser supports multi-file programs. It follows `mod` declarations and explicit `#[path = "..."]` modules from `src/lib.rs` to discover accounts, instructions, and discriminators across all reachable source files. Missing unconditional modules are errors; missing `#[cfg(...)]` modules are treated as inactive.

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

<!-- {=pinaIdlProgramMetadata} -->

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

For best IDL extraction fidelity, follow the rules documented in [`crates/pina_cli/rules.md`](./rules.md).
