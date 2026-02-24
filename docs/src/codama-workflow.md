# Codama Workflow

This repository uses Codama as the IDL and client-generation layer for Pina programs.

The flow has three stages:

1. Generate Codama JSON from Rust programs (`pina idl`).
2. Validate generated JSON against committed fixtures/tests.
3. Render clients (JS with Codama renderers, Rust with `pina_codama_renderer`).

## In This Repository

Generate and validate the whole workspace flow with `devenv` scripts:

```bash
# Generate Codama IDLs for all examples.
codama:idl:all

# Generate Rust + JS clients.
codama:clients:generate

# Generate IDLs + Rust/JS clients in one command.
pina codama generate

# Run the complete Codama pipeline.
codama:test

# Run IDL fixture drift + validation checks used by CI.
test:idl
```

Supporting scripts:

- `scripts/generate-codama-idls.sh`: regenerates `codama/idls/*.json` fixtures for all examples.
- `scripts/verify-codama-idls.sh`: regenerates IDLs/clients, verifies fixtures via Rust and JS tests, and enforces deterministic no-diff output.

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

### 3. Generate Pina-style Rust clients (optional)

This repository ships `crates/pina_codama_renderer`, which emits Rust models aligned with Pina's discriminator-first, fixed-size POD layouts.

```bash
cargo run --manifest-path ./crates/pina_codama_renderer/Cargo.toml -- \
  --idl ./idls/my_program.json \
  --output ./clients/rust
```

You can pass multiple `--idl` flags or `--idl-dir`.

## Renderer Constraints

`pina_codama_renderer` intentionally targets fixed-size layouts. Unsupported patterns produce explicit errors (for example variable-length strings/bytes, unsupported endian/number forms, and non-fixed arrays).

## CI Coverage

Codama checks are enforced in the `ci` workflow via `test:idl`.
