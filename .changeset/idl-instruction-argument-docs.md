---
pina_cli: fix
pina_cli_renderer: fix
---

# Emit instruction argument doc comments in the generated IDL

`pina idl` parsed the `///` doc comments on `#[instruction(...)]` struct fields but dropped them when lowering each field into a Codama `instructionArgumentNode`, so every instruction argument was published with no docs. The generator now copies those comments onto the argument node, matching how account and event fields already carry their docs.

Generated clients pick the docs up on the next `pina generate`: the Rust, CPI, and TypeScript instruction data types gain doc comments on their argument fields, and the generated Rust, TypeScript, and Dart CLIs show the doc text as each argument's `--help` description instead of the bare argument name. Regenerate your IDL and clients after upgrading to pick them up.

Because those docs are copied verbatim from the program source, an intra-doc link such as ``[`KIND_VAULT`]`` resolves in the program crate but not in the generated Rust CLI. The CLI renderer now allows `rustdoc::broken_intra_doc_links` in its generated files, as the CPI renderer already does, so documenting the CLI with `-D warnings` keeps passing.
