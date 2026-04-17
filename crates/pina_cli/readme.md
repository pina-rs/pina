# `pina_cli`

<br>

CLI and library for generating Codama IDLs from Pina programs.

The binary name is `pina`.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

<br>

```bash
cargo install pina_cli
```

Or run from source in this repository:

```bash
cargo run -p pina_cli -- --help
```

## Commands

<br>

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
```

### `pina init`

<br>

Scaffold a new Pina program project.

```bash
pina init my_program
pina init my_program --path ./programs/my_program --force
```

### `pina profile`

<br>

Static CU profiler for compiled SBF programs.

```bash
pina profile target/deploy/my_program.so
pina profile target/deploy/my_program.so --json
pina profile target/deploy/my_program.so --output report.json
```

### `pina codama generate`

<br>

Generate Codama IDLs and Rust/JS clients from one or more example program crates.

```bash
pina codama generate
pina codama generate --example counter_program --example todo_program
```

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

### Pina-style Rust clients

<br>

This repository includes `crates/pina_codama_renderer`, which renders discriminator-first/bytemuck Rust client models from Codama JSON.

```bash
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

## Parser Expectations

<br>

The IDL parser supports multi-file programs — it follows `mod` declarations from `src/lib.rs` to discover accounts, instructions, and discriminators across all source files.

<!-- {=pinaIdlDispatchSupport} -->

The extractor currently supports these dispatch shapes:

- Canonical routed arms: `Variant => Accounts::try_from(accounts)?.process(data)`
- Grouped routed arms: `VariantA | VariantB => SharedAccounts::try_from(accounts)?.process(data)`
- Accountless arms: `Variant => { let _ = Payload::try_from_bytes(data)?; Ok(()) }`
- Instruction-only fallback: if Pina finds `#[instruction]` structs but no recognizable dispatch map, it still emits zero-account instruction nodes from those payload structs.

Keep in mind:

- Account metadata is only inferred for routed `Accounts::try_from(accounts)` arms.
- Signer/writable/PDA/default-account inference still depends on direct `self.field.assert_*()` chains inside `impl ProcessAccountInfos`.
- If you hide routing or validation behind helper layers, instruction nodes may still exist, but account metadata becomes less complete.

<!-- {/pinaIdlDispatchSupport} -->

<!-- {=pinaIdlVerificationContract} -->

`test:idl` treats the generated IDL as an API contract. It checks that:

- every example regenerates deterministically into `codama/idls`, `codama/clients/js`, and `codama/clients/rust`
- generated JSON passes Codama's JS validator
- generated JS clients typecheck
- generated Rust clients compile
- for every example, generated instruction/account/error counts match the source declarations:
  - `#[instruction]`
  - `#[account]`
  - `#[error]`

That last count-parity check is important because it catches silent extraction regressions where a program still produces valid JSON, but one or more instruction surfaces disappear.

<!-- {/pinaIdlVerificationContract} -->

For best IDL extraction fidelity, follow the rules documented in [`crates/pina_cli/rules.md`](./rules.md).

[crate-image]: https://img.shields.io/crates/v/pina_cli.svg?style=flat-square
[crate-link]: https://crates.io/crates/pina_cli
[docs-image]: https://docs.rs/pina_cli/badge.svg
[docs-link]: https://docs.rs/pina_cli/
[ci-status-image]: https://github.com/pina-rs/pina/workflows/ci/badge.svg
[ci-status-link]: https://github.com/pina-rs/pina/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicense-blue.svg?style=flat-square
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/pina-rs/pina/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/pina-rs/pina
