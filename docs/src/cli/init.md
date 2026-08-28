# `pina init`

Create a standalone Pina program scaffold.

## Synopsis

```text
pina init [OPTIONS] <NAME>
```

| Input              | Default    | Meaning                                                               |
| ------------------ | ---------- | --------------------------------------------------------------------- |
| `NAME`             | required   | Rust package name. ASCII letters, numbers, `-`, and `_` are accepted. |
| `-p, --path <DIR>` | `./<name>` | Destination directory.                                                |
| `--force`          | off        | Overwrite scaffold-owned files that already exist.                    |

## Example

```bash
pina init counter_program
pina init counter_program --path ./programs/counter_program
```

The command creates:

```text
counter_program/
├── .cargo/
│   └── config.toml
├── src/
│   ├── entrypoint.rs
│   └── lib.rs
├── tests/
│   ├── integration.rs
│   └── surfpool/
│       ├── src/
│       │   └── lib.rs
│       └── Cargo.toml
├── .gitignore
├── Cargo.toml
├── Pina.toml
└── README.md
```

The scaffold includes:

- a `no_std` program library and feature-gated SBF entrypoint;
- a typed instruction discriminator and starter instruction;
- an `Accounts` struct with signer validation;
- an SBF Cargo target and `cargo build-program` alias;
- a pinned nightly Rust toolchain with the `rust-src` component;
- release-matched Pina security lint metadata and pinned Dylint tool versions;
- host-side discriminator and program-ID smoke tests;
- Pina and Mollusk dependencies;
- a dedicated host-only test package with one `pina_test` dependency for the isolated Surfpool test;
- project-local discovery and client-generation settings in `Pina.toml`.

Replace the non-system placeholder address in `src/lib.rs` with the deployed program address before deployment.

Install `sbpf-linker` once before the first SBF build:

```bash
cargo install sbpf-linker --version 0.1.8 --locked
```

## Existing destinations

Without `--force`, Pina checks every scaffold-owned destination before writing anything. If one already exists, the command exits without modifying the scaffold.

`--force` overwrites only the known scaffold files listed above. It does not delete unrelated files in the destination directory.

## Next steps

The command prints the exact build, SBF build, test, and IDL commands for the generated package:

```bash
cd ./counter_program
pina lint
pina build
pina test --unit
pina test
pina dev --yes
pina generate
```

Use `pina init --help` for the authoritative command-line surface.

See [Project Configuration](./configuration.md) for every generated `Pina.toml` field.
