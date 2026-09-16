# `pina cpi`

Generate a standalone, `no_std` Pina CPI crate from a Codama or Anchor IDL.

## Synopsis

```text
pina cpi (--idl <FILE> | --stdin) --output <DIR> [OPTIONS]
```

| Input             | Default | Meaning                                                    |
| ----------------- | ------- | ---------------------------------------------------------- |
| `--idl <FILE>`    | none    | Codama or raw Anchor IDL to normalize and render.          |
| `--stdin`         | off     | Read a normalized Codama root from a visitor pipeline.     |
| `-o, --output`    | none    | Directory for the generated standalone crate.              |
| `--mode <MODE>`   | `auto`  | `auto`, `create`, `update`, or complete `overwrite`.       |
| `--no-scaffold`   | off     | Generate `src/generated` without Cargo.toml or src/lib.rs. |
| `--npx <COMMAND>` | `npx`   | Runner for raw Anchor conversion; unused for Codama IDLs.  |

```bash
pina cpi --idl ./target/idl/counter.json --output ./clients/counter-cpi
pina cpi --idl ./anchor-idl.json --output ./clients/anchor-cpi
pina cpi --idl ./anchor-idl.json --output ./clients/anchor-cpi --mode update
```

Instruction arguments may be little-endian native integers (`u8`-`u128` and `i8`-`i128`), booleans, public keys, fixed `u8` arrays, PinaPod's bounded strings and vectors, and anything reachable through the IDL's `definedTypes`: structs, enums, options, tuples, length-prefixed arrays, and maps. Anchor IDLs route most non-primitive arguments through that table, so a real third-party IDL renders without hand-editing.

Floating point, `shortU16`, bare (unprefixed) strings and byte slices, and big-endian integers are rejected before rendering, each with the reason in the error message.

An instruction whose arguments are all fixed-width returns `[u8; LEN]` from `to_bytes()`. When an argument's length depends on caller-supplied data the instruction instead exposes `MAX_DATA_LEN` and takes a caller-owned buffer through `encode_into` and `invoke_signed`, so the crate stays `no_std` and allocator-free either way.

Accounts from the IDL render into a `generated/accounts` module with a struct, the account discriminator, an encoded-size constant, a `matches` guard, and — when every field has a fixed offset — a `parse` function.

To adopt another program's interface end to end, including provenance and the program-ID binding test, see [Import a Foreign Program](./import.md).

Codama roots are rendered natively. Raw Anchor IDLs are normalized with `@codama/nodes-from-anchor`, then passed to the same renderer. The output crate contains a validated `ProgramAccount` and direct struct-based calls exposing `.invoke()` and `.invoke_signed()`. Each call contains its account references and a typed `*Ix` field named `ix`; its `to_bytes()` output is passed as CPI data. Account and argument fields preserve their IDL documentation and are labelled with their role.

The default `auto` mode creates missing scaffold files on initial generation and preserves them on updates. `create` fails if the target is nonempty, `update` fails if it is absent or empty, and `overwrite` removes the entire crate before generating it again. Use `--no-scaffold` when a workspace already supplies its own manifest and entrypoint.

The CLI test suite passes a committed raw Anchor IDL through the real pinned converter, checks the generated struct and documentation surface, and runs `cargo check` on the standalone `no_std` crate. The `pina_bpf` example also consumes the generated Prop AMM CPI client in its signer and PDA-signer execution paths.

Use `pina generate --client cpi` when the source is the current Pina program. For a reusable Codama script, install `@pina-rs/codama-renderer-cpi`; Codama normalizes either source format before passing its current transformed root to the visitor.
