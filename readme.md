<p align="center">
	<a href="./.github/assets/README.md">
		<img
			src="./.github/assets/logo.png"
			alt="Pina: a pineapple made from interlocking ribbons"
			width="200"
		>
	</a>
</p>

<br>

<p align="center">
	<strong>Pina</strong> is a Solana program framework built on
	<a href="https://github.com/anza-xyz/pinocchio">pinocchio</a>: programs deploy with the
	<em>smallest binaries</em> of any Solana framework, spend compute like <em>hand-written</em>
	code, and evolve through the only <em>managed ABI migrations</em> in the ecosystem.
</p>

<br>

<p align="center">
	<a href="#why-pina"><strong>Why Pina?</strong></a> ·
	<a href="#how-pina-compares"><strong>Comparison</strong></a> ·
	<a href="#getting-started"><strong>Getting Started</strong></a> ·
	<a href="https://pina-rs.github.io/pina/"><strong>Docs</strong></a> ·
	<a href="#examples"><strong>Examples</strong></a> ·
	<a href="#contributing"><strong>Contributing</strong></a>
</p>

<br>

<!-- {=crateReadmeBadgeRow:"pina"} -->

[![Crates.io](https://img.shields.io/badge/crates.io-pina-orange?logo=rust)](https://crates.io/crates/pina) [![Docs.rs](https://img.shields.io/badge/docs.rs-pina-1f425f?logo=docs.rs)](https://docs.rs/pina/) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![Coverage](https://codecov.io/gh/pina-rs/pina/branch/main/graph/badge.svg)](https://codecov.io/gh/pina-rs/pina) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

<!-- {/crateReadmeBadgeRow} -->

> Pina is currently unaudited and still hardening. See [SECURITY.md](./SECURITY.md) for the current readiness statement, supported versions, and private vulnerability reporting instructions.

## Why Pina?

<br>

<!-- {=pinaFeatureHighlights} -->

- **Smallest binaries, lowest framework overhead**: built on `pinocchio` instead of `solana-program`, so the framework's dispatch and validation cost a few bytes and a few compute units, not thousands of CU per instruction.
- **`no_std` and allocation-free**: every on-chain crate compiles to the `bpfel-unknown-none` SBF target with no heap and no allocator.
- **Framework-managed ABI migrations**: version envelopes, generated transitions, and a publication ledger evolve account and instruction schemas without breaking deployed programs — the only Solana framework that ships this.
- **Validated zero-copy deserialization**: PinaPod validates account data before Pina returns an in-place view, with no heap allocation.
- **Discriminator-first layouts**: every account, instruction, and event type carries a typed discriminator as its first field, one byte by default.
- **Declarative validation**: chain assertions on `AccountView` references, or compile `#[pina(validate(...))]` rules into allocation-free checks.
- **Proc-macro sugar**: `#[account]`, `#[instruction]`, `#[event]`, `#[error]`, `#[discriminator]`, and `#[derive(Accounts)]` eliminate boilerplate.
- **CPI helpers**: PDA account creation, lamport transfers, and token operations as documented instruction structs.

<!-- {/pinaFeatureHighlights} -->

Macros stay minimal, the IDL is inferred from the validation code you already write (`payer.assert_signer()?` becomes a `signer` constraint in the generated IDL), and one language runs end to end: on-chain Rust, and generated Rust, TypeScript, or Dart clients. The [project goals](https://pina-rs.github.io/pina/project-goals.html) describe the philosophy in full.

## How Pina compares

<br>

### Capabilities

<br>

| Capability                       | Pina                                                                                          | Anchor v1                        | Anchor v2 (`lang-v2`)      | Quasar                    |
| -------------------------------- | --------------------------------------------------------------------------------------------- | -------------------------------- | -------------------------- | ------------------------- |
| **Version compared**             | v<!-- {~pinaVersion:"{{ cargo.workspace.package.version }}"} -->0.23.0<!-- {/pinaVersion} --> | 1.2.1                            | 2.0.0-rc.1                 | rev `b0de7db`             |
| IDL generated from source        | ✅                                                                                            | ✅                               | ✅                         | ✅                        |
| Declarative account validation   | ✅                                                                                            | ✅                               | ✅                         | ✅                        |
| Typed generated clients          | ✅ Rust · JS/TS · Dart · CPI crates                                                           | ✅ TS · Rust CPI                 | 🟡 Rust · TS (alpha)       | ✅ Rust · JS · Solana Kit |
| `no_std` on-chain runtime        | ✅ allocation-free                                                                            | ❌ std + heap                    | 🟡 allocator by default    | ✅ zero-allocation        |
| Validated zero-copy accounts     | ✅ validated views                                                                            | 🟡 opt-in `AccountLoader`        | ✅ Pod accounts by default | ✅ pointer-cast views     |
| One-byte discriminators          | ✅ default (`u8`–`u64`)                                                                       | ❌ 8-byte hash                   | 🟡 opt-in                  | ✅                        |
| Managed ABI migrations           | ✅ version envelopes · transitions · publication ledger                                       | ❌ off-chain migrate script only | ❌                         | ❌                        |
| Versioned events                 | ✅ every released schema decodable by clients                                                 | ❌                               | ❌                         | ❌                        |
| Security lints on program source | ✅ `pina lint`                                                                                | ❌                               | ❌                         | ❌                        |
| CU profiler                      | ✅ static `.so` analysis                                                                      | 🟡 trace flamegraphs only        | 🟡 shares the Anchor CLI   | ✅ static + flamegraph    |
| Upgrade rehearsal before deploy  | ✅ `pina deploy --rehearse`                                                                   | ❌                               | ❌                         | ❌                        |

✅ first-class · 🟡 partial · ❌ not available.

Anchor v1 is the current stable line and Anchor v2 its pre-release rewrite; both share one CLI. Quasar publishes no versioned crates, so its pinned revision is the version. The Pina version follows this repository and refreshes on every release. Update the Anchor and Quasar cells here when bumping the fixtures under [`benchmarks/framework-comparison/`](./benchmarks/framework-comparison/).

### On-chain cost

<br>

The same hello-world and counter programs written in each framework, built with `cargo build-sbf --lto` under one release profile, and executed in a Mollusk VM. Pinocchio (hand-written) is the floor a framework has to justify. Regenerate with `devenv shell -- benchmark:frameworks`; the full methodology and caveats live in [the framework comparison](https://pina-rs.github.io/pina/framework-comparison.html).

<!-- BEGIN GENERATED: readme-framework-comparison -->

#### Hello world

| Framework                | Version       | Size (bytes) | `hello` CU | vs Pinocchio size |
| ------------------------ | ------------- | -----------: | ---------: | ----------------: |
| Pina                     | 0.23.0        |        1,616 |        136 |              −49% |
| Pinocchio (hand-written) | 0.11.2        |        3,160 |        111 |               +0% |
| Quasar                   | rev `b0de7db` |        2,520 |        115 |              −20% |
| Anchor v1                | 1.2.1         |       74,416 |        421 |            +2255% |
| Anchor v2 (`lang-v2`)    | 2.0.0-rc.1    |        1,880 |        127 |              −41% |

#### Counter

| Framework                | Version       | Size (bytes) | `initialize` CU | `increment` CU | vs Pinocchio size |
| ------------------------ | ------------- | -----------: | --------------: | -------------: | ----------------: |
| Pina                     | 0.23.0        |        7,592 |           1,694 |            360 |              +17% |
| Pinocchio (hand-written) | 0.11.2        |        6,512 |           1,490 |          1,721 |               +0% |
| Quasar                   | rev `b0de7db` |        7,808 |           3,488 |            330 |              +20% |
| Anchor v1                | 1.2.1         |      139,408 |          12,676 |         10,374 |            +2041% |
| Anchor v2 (`lang-v2`)    | 2.0.0-rc.1    |        8,696 |           3,458 |          2,117 |              +34% |

<!-- END GENERATED: readme-framework-comparison -->

## Getting started

<br>

<!-- {=pinaInstallation} -->

```sh
cargo add pina
```

To enable SPL token support:

```sh
cargo add pina --features token
```

<!-- {/pinaInstallation} -->

Install the prebuilt CLI from npm on macOS, Linux, Windows, or FreeBSD:

```sh
npm install --global @pina-rs/cli
pina --help
```

The Rust-native installation remains available with `cargo install pina_cli`. Agent tooling can install the bundled project skill separately:

```sh
npm install --global @pina-rs/skill
pina-skill --install
```

Scaffold a program and run it through its first build:

```sh
pina init my_program
cd my_program
pina build
cargo test
```

- [Getting started](https://pina-rs.github.io/pina/getting-started.html) — toolchain setup and project layout.
- [Your first program](https://pina-rs.github.io/pina/tutorials/first-program.html) — accounts, instructions, and dispatch, end to end.
- [Migrating from Anchor](https://pina-rs.github.io/pina/tutorials/migrating-from-anchor.html) — a side-by-side mapping of Anchor concepts.
- [Crate features](https://pina-rs.github.io/pina/crates-and-features.html) — `compact`, `validation`, `token`, `account-resize`, and the logging toggles.

## Documentation

<br>

The [book](https://pina-rs.github.io/pina/) is the documentation home; API docs are on [docs.rs](https://docs.rs/pina/). Frequently used pages:

- [Framework comparison](https://pina-rs.github.io/pina/framework-comparison.html) — how the tables above are measured.
- [How ABI migrations flow](https://pina-rs.github.io/pina/migrations/flow.html) — version envelopes, transitions, and the publication ledger.
- [Core concepts](https://pina-rs.github.io/pina/core-concepts.html) — accounts, instructions, discriminators, and validation chains.
- [Security model](https://pina-rs.github.io/pina/security-model.html) and the [security guide](./security/readme.md) — vulnerable and secure patterns for all 11 common Solana attack categories.
- [Pina CLI reference](https://pina-rs.github.io/pina/cli/index.html) — every command, including `pina lint`, `pina profile`, `pina dev`, and `pina deploy --rehearse`.
- [Codama workflow](https://pina-rs.github.io/pina/codama-workflow.html) — IDL extraction and client generation for Rust, JS/TS, and Dart.

## Examples

<br>

| Example                                                         | Description                                                                            |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| [`counter_program`](examples/counter_program)                   | PDA state management with initialize and increment                                     |
| [`escrow_program`](examples/escrow_program)                     | Full token escrow with SPL token operations                                            |
| [`multisig_program`](examples/multisig_program)                 | Production-shaped multisig: bitmask votes, timelocked vault execution, spending limits |
| [`account_realloc_program`](examples/account_realloc_program)   | Dynamic compact account lifecycle with typed, rent-adjusted patches                    |
| [`compact_accounts_program`](examples/compact_accounts_program) | Atomic compact patches, rent adjustment, and generated clients                         |
| [`transfer_sol_program`](examples/transfer_sol_program)         | CPI and direct lamport transfers                                                       |
| [`events_program`](examples/events_program)                     | Deterministic event serialization                                                      |
| [`hello_solana_program`](examples/hello_solana_program)         | Minimal program — entrypoint, accounts, logging                                        |

Two dozen examples live in [`examples/`](./examples); the annotated list is in [the docs](https://pina-rs.github.io/pina/examples.html). The Anchor parity ports (`declare-id`, `declare-program`, duplicate-mutable-account checks, sysvar checks, custom errors, …) keep behaviour comparable while you migrate.

## Workspace packages

<br>

<!-- {=pinaWorkspacePackages} -->

| Package                 | Path                          | Description                                                                   |
| ----------------------- | ----------------------------- | ----------------------------------------------------------------------------- |
| `pina`                  | `crates/pina`                 | Core framework: traits, account loaders, CPI helpers, and Pod types.          |
| `pina_macros`           | `crates/pina_macros`          | Proc macros: `#[account]`, `#[instruction]`, `#[event]`, and others.          |
| `pina_cli`              | `crates/pina_cli`             | CLI for building, testing, inspecting, and generating Pina program artifacts. |
| `pina_codama_renderer`  | `crates/pina_codama_renderer` | Repository-local Codama Rust renderer for Pina-style clients.                 |
| `pina_cpi_renderer`     | `crates/pina_cpi_renderer`    | Standalone Codama renderer generating Pina CPI client crates.                 |
| `pina_lints`            | `crates/pina_lints`           | Pina security lints and the driver behind `pina lint`.                        |
| `pina_test`             | `crates/pina_test`            | Surfpool-backed program test harness.                                         |
| `pina_profile`          | `crates/pina_profile`         | Static and trace-driven CU profiler for SBF programs.                         |
| `pina_sdk_ids`          | `crates/pina_sdk_ids`         | Typed constants for well-known Solana program/sysvar IDs.                     |
| `@pina-rs/codama-nodes` | `packages/nodes-from-pina`    | Pina IDL conversion and normalization for Codama root nodes.                  |
| `@pina-rs/cli`          | `packages/pina__cli`          | npm launcher for the prebuilt platform-specific CLI packages.                 |
| `@pina-rs/skill`        | `packages/pina__skill`        | Agent guidance and a non-destructive local skill installer.                   |

<!-- {/pinaWorkspacePackages} -->

## Security

<br>

Pina validates account identity, signer status, writability, ownership, and data shape before any cast, mutation, or CPI, and its [security lints](./crates/pina_lints/readme.md) catch common Solana mistakes at compile time:

```sh
pina lint
```

See [SECURITY.md](./SECURITY.md) for the readiness statement and private vulnerability reporting, the [security model](https://pina-rs.github.io/pina/security-model.html) for the guarantees Pina does and does not make, and the [security guide](./security/readme.md) for worked vulnerable/secure pairs across all 11 common Solana attack categories.

## Contributing

<br>

Contributions are welcome! Please open an issue or pull request on [GitHub](https://github.com/pina-rs/pina). Read the [development workflow](https://pina-rs.github.io/pina/development-workflow.html) first — this repository reproduces every CI job locally before pushing.

## License

<br>

Licensed under the [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).
