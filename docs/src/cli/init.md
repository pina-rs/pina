# `pina init`

Create a standalone Pina program scaffold.

## Synopsis

```text
pina init [OPTIONS] <NAME>
```

| Input              | Default    | Meaning                                                                                                              |
| ------------------ | ---------- | -------------------------------------------------------------------------------------------------------------------- |
| `NAME`             | required   | Rust package name. 1-64 ASCII letters, numbers, `-`, and `_`, starting with a letter or `_`, and not a Rust keyword. |
| `-p, --path <DIR>` | `./<name>` | Destination directory.                                                                                               |
| `--force`          | off        | Overwrite scaffold-owned files that already exist.                                                                   |

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
├── build.rs
├── Cargo.toml
├── pina.toml
├── README.md
└── rust-toolchain.toml
```

The scaffold includes:

- a `no_std` program library and feature-gated SBF entrypoint;
- a typed instruction discriminator and starter instruction;
- an `Accounts` struct with signer validation;
- a `cargo build-program` alias for the Agave `cargo build-sbf` driver;
- a pinned nightly Rust toolchain with the `rust-src` and `clippy` components, matching the nightly Pina publishes prebuilt lint drivers for;
- `[migrations]` enabled with `auto = true` and the `build.rs` rerun directive, but no manifest yet, because the history binds to the program address;
- no source-installed lint tooling; `pina lint` resolves a prebuilt `pina_lint_driver` for the project's active toolchain, and `pina lint --build-driver` compiles one when no prebuilt driver matches;
- host-side discriminator and program-ID smoke tests;
- a `pina` dependency with the `account-resize`, `logs`, and `derive` features (no Mollusk; add it when you need VM-level unit tests);
- a dedicated host-only test package with one `pina_test` dependency for the isolated Surfpool test;
- project-local discovery and client-generation settings in `pina.toml`.

Every scaffold starts with the same non-system placeholder address, which nobody holds the keypair for, so it can be neither deployed nor safely snapshotted; `pina doctor` warns while it is in place. Run `pina keys new` to give the program its own identity, then `pina migrations create` to record the version-0 baseline. Until that baseline exists, `pina build`, `pina idl`, and `pina generate` refuse to run, while `cargo check` and `cargo test` still pass.

SBF builds use the Agave CLI's `cargo-build-sbf`, so install the Agave CLI before the first `pina build`. Client generation for TypeScript and Dart needs Node.js with `npx`.

The scaffold declares no `[workspace]`. When the destination is inside an existing Cargo workspace, `pina init` prints the enclosing manifest: add the program to that workspace's `members`, and declare the dependencies generated Rust clients inherit in its `[workspace.dependencies]`.

## Existing destinations

Without `--force`, Pina checks every scaffold-owned destination before writing anything. If one already exists, the command exits without modifying the scaffold.

`--force` overwrites only the known scaffold files listed above. It does not delete unrelated files in the destination directory, but it does rewrite `src/lib.rs`, including resetting `declare_id!` to the placeholder.

## Next steps

The command prints the next steps for the generated package:

```bash
cd ./counter_program
pina keys new
pina migrations create
pina lint
pina build
pina test --unit
pina test
pina dev --yes
pina generate
```

`pina keys new` writes the program keypair to `target/deploy/<name>-keypair.json`. The scaffold's `.gitignore` excludes `target/`, and `cargo clean` deletes it, so back the keypair up before deploying.

Use `pina init --help` for the authoritative command-line surface.

See [Project Configuration](./configuration.md) for every generated `pina.toml` field.
