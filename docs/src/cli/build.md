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

## Outputs

Pina uses Cargo metadata, including `CARGO_TARGET_DIR`, to derive stable output paths:

```text
<cargo-target>/deploy/<library-name>.so
<cargo-target>/idl/<library-name>.json
```

The library target name is authoritative. It may differ from the package name when `[lib].name` is set.

Cargo output is streamed to the terminal. Pina publishes the SBF binary and IDL atomically only after the compiler artifact and IDL are both ready. A failed Cargo build does not publish either output.

```bash
pina build
pina build --features logs,cpi --no-default-features
pina build --project ./programs/counter
```

Project discovery uses the nearest ancestor `Pina.toml`. Existing projects without that file fall back to Cargo metadata; an ambiguous workspace root fails with the candidate package names instead of guessing.
