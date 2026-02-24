# `pina_cli`

CLI and library for generating Codama IDLs from Pina programs.

The binary name is `pina`.

[![Crates.io][crate-image]][crate-link] [![Docs.rs][docs-image]][docs-link] [![CI][ci-status-image]][ci-status-link] [![License][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

```bash
cargo install pina_cli
```

Or run from source in this repository:

```bash
cargo run -p pina_cli -- --help
```

## Commands

### `pina idl`

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

Scaffold a new Pina program project.

```bash
pina init my_program
pina init my_program --path ./programs/my_program --force
```

## Library API

`pina_cli` can also be embedded directly:

```rust
use std::path::Path;

let root = pina_cli::generate_idl(Path::new("./examples/counter_program"), None)?;
println!("{}", serde_json::to_string_pretty(&root)?);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Codama Workflow

1. Generate IDL with `pina idl`.
2. Feed the JSON into Codama renderers.

### JavaScript clients in another project

```bash
pina idl --path ./programs/my_program --output ./idls/my_program.json
pnpm add -D codama @codama/renderers-js
```

```js
import { createFromFile } from "codama";
import { renderVisitor as renderJsVisitor } from "@codama/renderers-js";

const codama = await createFromFile("./idls/my_program.json");
await codama.accept(renderJsVisitor("./clients/js/my_program"));
```

### Pina-style Rust clients

This repository includes `codama/pina_codama_renderer`, which renders discriminator-first/bytemuck Rust client models from Codama JSON.

```bash
cargo run --manifest-path ./codama/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

## Parser Expectations

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
