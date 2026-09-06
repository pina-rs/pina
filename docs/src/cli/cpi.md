# `pina cpi`

Generate a standalone, `no_std` Pina CPI crate from a Codama or Anchor IDL.

## Synopsis

```text
pina cpi (--idl <FILE> | --stdin) --output <DIR> [--npx <COMMAND>]
```

| Input             | Default | Meaning                                                   |
| ----------------- | ------- | --------------------------------------------------------- |
| `--idl <FILE>`    | none    | Codama or raw Anchor IDL to normalize and render.         |
| `--stdin`         | off     | Read a normalized Codama root from a visitor pipeline.    |
| `-o, --output`    | none    | Directory for the generated standalone crate.             |
| `--npx <COMMAND>` | `npx`   | Runner for raw Anchor conversion; unused for Codama IDLs. |

```bash
pina cpi --idl ./target/idl/counter.json --output ./clients/counter-cpi
pina cpi --idl ./anchor-idl.json --output ./clients/anchor-cpi
```

Codama roots are rendered natively. Raw Anchor IDLs are normalized with `@codama/nodes-from-anchor`, then passed to the same renderer. The output crate contains a validated `ProgramAccount` and direct struct-based calls exposing `.invoke()` and `.invoke_signed()`. Each call contains its account references and a typed `*Instruction` field whose `to_bytes()` output is passed as CPI data. Account and argument fields preserve their IDL documentation and are labelled with their role.

The CLI test suite passes a committed raw Anchor IDL through the real pinned converter, checks the generated struct and documentation surface, and runs `cargo check` on the standalone `no_std` crate. The `pina_bpf` example also consumes the generated Prop AMM CPI client in its signer and PDA-signer execution paths.

Use `pina generate --client cpi` when the source is the current Pina program. For a reusable Codama script, install `@pina-rs/codama-renderer-cpi`; Codama normalizes either source format before passing its current transformed root to the visitor.
