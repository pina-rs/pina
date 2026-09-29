# Getting Started

## Build a program with the CLI

To write your own program you need the `pina` CLI and the external tools it drives, not this repository:

- [rustup](https://rustup.rs/): the scaffold's `rust-toolchain.toml` selects the pinned nightly.
- The [Agave CLI](https://docs.anza.xyz/cli/install), which provides `cargo-build-sbf` for `pina build` and `solana` for `pina deploy`.
- Node.js with `npx` when you generate TypeScript or Dart clients.
- The [Surfpool CLI](https://docs.surfpool.run/toolchain/cli) for `pina dev`; `pina test` embeds Surfpool's SDK in the generated test package.

```bash
npm install --global @pina-rs/cli
pina init my_program
cd my_program
pina doctor               # checks every prerequisite above
pina keys new             # replace the shared placeholder program ID
pina migrations create    # record the version-0 ABI baseline
pina build
pina test --unit
pina test
pina generate
```

Run `pina keys new` and `pina migrations create` before anything else. The scaffold enables migrations, so `pina build`, `pina idl`, and `pina generate` refuse to run until the baseline exists, and the baseline records the program ID it belongs to. See the [`pina init` reference](./cli/init.md) for what the scaffold contains.

## Work on Pina itself

### Prerequisites

- Rust nightly toolchain from `rust-toolchain.toml`
- `devenv` (Nix-based environment)
- `gh` (for GitHub workflows)

### Setup

<!-- {=devEnvironmentSetupCommands} -->

```bash
devenv shell
install:all
```

<!-- {/devEnvironmentSetupCommands} -->

See `pina init --help` for options like `--path` and `--force`. The [CLI reference](./cli/index.md) documents every command, output contract, and automation workflow.

For agent-assisted project work, install the [Pina skill](./agent-skill.md).

If `pnpm-workspace.yaml` sets `useNodeVersion`, `devenv shell` activates the matching pnpm-managed `node`/`npm`/`npx`/`corepack` toolchain automatically.

### Build and test

<!-- {=buildAndTestCommands} -->

```bash
cargo build --all-features
cargo test
```

<!-- {/buildAndTestCommands} -->

For a deterministic Docker build suitable for Solana's verified-build workflow, install `solana-verify` 0.5.1, start Docker, commit the complete source tree, and run:

```bash
pina build --verify
```

This produces the canonical deploy artifact plus a hash-bound Pina build record. It does not perform on-chain verification. See [`pina build`](./cli/build.md#deterministic-verified-build-artifacts) for the trust model, prerequisites, and limitations.

### Common quality checks

<!-- {=commonQualityChecksCommands} -->

```bash
lint:clippy
lint:format
verify:docs
```

<!-- {/commonQualityChecksCommands} -->

### Generate a Codama IDL

```bash
pina idl --path ./examples/counter_program --output ./codama/idls/counter_program.json
```

See [Codama Workflow](./codama-workflow.md) for end-to-end generation and external-project usage.

Before adapting an example for a program that controls assets, work through the [Production Readiness](./production-readiness.md) gate. Examples demonstrate scoped framework behavior; they are not audited deployment templates.

### Build this documentation

<!-- {=docsBuildCommand} -->

```bash
docs:build
```

<!-- {/docsBuildCommand} -->

The generated site is written to `docs/book/`.
