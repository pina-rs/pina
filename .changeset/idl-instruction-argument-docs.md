---
pina_cli: fix
---

# Emit instruction argument doc comments in the generated IDL

`pina idl` parsed the `///` doc comments on `#[instruction(...)]` struct fields but dropped them when lowering each field into a Codama `instructionArgumentNode`, so every instruction argument was published with no docs. The generator now copies those comments onto the argument node, matching how account and event fields already carry their docs.

Generated clients pick the docs up on the next `pina generate`: the Rust, CPI, and TypeScript instruction data types gain doc comments on their argument fields, and the generated Rust, TypeScript, and Dart CLIs show the doc text as each argument's `--help` description instead of the bare argument name. Regenerate your IDL and clients after upgrading to pick them up.
