---
pina_cli_renderer: fix
pina_codama_renderer_cli: fix
---

# Fix generated CLI payer and PDA handling

The generated TypeScript, Dart, and Rust CLIs had related defects rooted in the renderers. Fixed-seed PDA accounts could not be fetched from the TypeScript or Dart CLIs: `--address` was mandatory even though the help text promised PDA derivation, while accounts without any PDA node kept requiring it. The Dart `fetch` command also registered no subcommands, so `fetch <account>` could never dispatch.

Payer-resolution accounts (accounts defaulted to the transaction payer) accepted an override option in every CLI, but the override was either silently ignored (TypeScript) or produced a transaction whose signer meta pointed at an address no loaded keypair signs for (Dart and Rust). Since these CLIs sign with exactly one keypair, all three renderers now resolve payer-resolution accounts to the loaded payer and stop emitting the unusable override options.
