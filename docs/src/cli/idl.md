# `pina idl`

Parse a Pina program crate and emit a Codama root-node JSON document.

## Synopsis

```text
pina idl [OPTIONS]
```

| Option                | Default            | Meaning                                                 |
| --------------------- | ------------------ | ------------------------------------------------------- |
| `-p, --path <DIR>`    | `.`                | Program crate containing `Cargo.toml` and `src/lib.rs`. |
| `-o, --output <FILE>` | stdout             | Write JSON to a file.                                   |
| `-n, --name <NAME>`   | Cargo package name | Override the emitted program name.                      |
| `--compact`           | off                | Emit one-line JSON instead of pretty-printed JSON.      |

## Output contract

Without `--output`, stdout contains JSON and nothing else. Progress and extraction counts go to stderr, so redirection is safe:

```bash
pina idl --path ./programs/counter_program > ./counter_program.json
jq . ./counter_program.json
```

With `--output`, the JSON is written to that file and stdout is unused:

```bash
mkdir -p ./idls
pina idl \
  --path ./programs/counter_program \
  --output ./idls/counter_program.json
```

The output file is replaced if it exists. Its parent directory must already exist.

## What the extractor reads

The extractor starts at `src/lib.rs` and follows Rust `mod` declarations, including `#[path = "..."]` modules. It derives the public IDL from source shapes rather than requiring a separate schema file.

It recognizes:

- `#[instruction]` payload structs and discriminator values;
- `#[account]` state layouts;
- `#[error]` enums;
- `#[pda]` typed seed declarations;
- `#[derive(Accounts)]` account order and mutability;
- direct validation chains for signer, writable, address, owner, and PDA metadata;
- canonical and grouped instruction dispatch arms.

Read [Codama Workflow](../codama-workflow.md#extractor-coverage) for supported dispatch shapes and source-authoring rules.

## Program naming

By default, the Codama program name comes from `package.name` in the target `Cargo.toml`. `--name` changes the emitted name without changing the Rust package or source tree:

```bash
pina idl -p ./programs/counter_program --name counter_v2
```

## Failure modes

The command exits unsuccessfully when the path is not a readable program crate, modules cannot be resolved, declarations conflict, a supported schema cannot be represented safely, or JSON/output-file writing fails. Diagnostics include source or path context where available.

For complete client generation, continue with [`pina codama generate`](./codama-generate.md).
