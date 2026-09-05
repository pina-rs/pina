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

Codama roots are rendered natively. Raw Anchor IDLs are normalized with `@codama/nodes-from-anchor`, then passed to the same renderer. The output crate contains a validated `ProgramAccount`, typed CPI account sets, and instruction builders exposing `.invoke()` and `.invoke_signed()`.

Use `pina generate --client cpi` when the source is the current Pina program. For a reusable Codama script, install `@pina-rs/codama-renderer-cpi`; Codama normalizes either source format before passing its current transformed root to the visitor.
