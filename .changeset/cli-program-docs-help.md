---
pina_cli: breaking
pina_cli_renderer: feat
pina_cpi_renderer: feat
pina_codama_renderer_cli: feat
---

# Carry program docs into generated CLI help

The IDL's program node now carries documentation from the program crate itself: the crate root's module docs (`//!`) come first, followed by the doc comment on the entrypoint surface — the hand-written `process_instruction` function when it has one, else the `#[discriminator(entrypoint)]` enum that generates it. A hand-written dispatch is authoritative, so its docs win over the enum's when both surfaces are annotated.

Every generated CLI renders those lines as its main description. The Rust clap CLI emits them as the `Cli` doc comment, so the first paragraph shows as `about` and the full text as the long help behind `--help`; the TypeScript CLI passes them to commander's `.description()` and keeps the first line in `package.json`; the Dart CLI passes them to the `CommandRunner` description. A program without either doc source keeps the previous `Command-line interface for <program>` fallback.

Per-instruction subcommands, argument flags, and account flags were already doc-derived from the IDL; this closes the one gap at the program level. The committed IDL and client fixtures are regenerated with the new program `docs`, and `pina_bpf_program` (which documents neither surface) regenerates unchanged. The generated CPI crates also pick the docs up on their program-ID constants, which that renderer already supported; those constants now carry an `#[allow(rustdoc::broken_intra_doc_links)]`, because the copied docs' intra-doc links resolve in the program crate but not in the generated one.

**Breaking:** `ProgramIr` and `DiscriminatorEnum` gain a public `docs` field, so exhaustive struct literals in downstream code need the new field. Everything else assembles those types through the parse and construction helpers, which fill the field in.
