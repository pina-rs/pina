# `pina build`

Build the current Pina program for SBF and refresh its Codama IDL.

## Synopsis

```text
pina build [OPTIONS]
```

| Input                   | Default           | Meaning                                             |
| ----------------------- | ----------------- | --------------------------------------------------- |
| `-p, --project <DIR>`   | current directory | Start directory for project discovery.              |
| `--features <FEATURE>`  | none              | Extra Cargo features; repeat or separate by commas. |
| `--no-default-features` | off               | Disable the program's default Cargo features.       |

`bpf-entrypoint` is always enabled and deduplicated from explicit features.

SBF compilation uses Cargo's unstable `-Z build-std=core,alloc` support. The `cargo` executable selected by `CARGO`, or `cargo` from `PATH`, must therefore be a nightly toolchain with the `rust-src` component installed.

## Outputs

Pina uses Cargo metadata, including `CARGO_TARGET_DIR`, to derive stable output paths:

```text
<cargo-target>/deploy/<library-name>.so
<cargo-target>/idl/<library-name>.json
```

The library target name is authoritative. It may differ from the package name when `[lib].name` is set.

Cargo output is streamed to the terminal. Pina stages both outputs first and atomically replaces each destination file. A project-scoped advisory lock prevents concurrent builds from interleaving different IDL and SBF versions. If the second replacement fails, Pina attempts to restore the previous IDL. A failed Cargo build does not publish either output; the two separate files do not form a crash-atomic filesystem transaction.

```bash
pina build
pina build --features logs,cpi --no-default-features
pina build --project ./programs/counter
```

Project discovery uses the nearest ancestor `Pina.toml`. Existing projects without that file fall back to Cargo metadata; an ambiguous workspace root fails with the candidate package names instead of guessing.

See [Project Configuration](./configuration.md) for the complete `Pina.toml` schema and path rules.
