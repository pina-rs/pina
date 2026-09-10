---
pina_cli_renderer: feat
pina_cli: feat
pina_codama_renderer: fix
pina_codama_renderer_cli: feat
---

Add CLI client generation for Codama IDLs in three languages: `pina generate --client cli-rust`, `--client cli-ts`, and `--client cli-dart`.

- `pina_cli_renderer` (new crate): renders complete clap-based CLI crates — one subcommand per instruction with typed, validated flags, automatic PDA derivation from IDL seed definitions, a `fetch` command per state account (compact-aware), decoded program-error names, and shared RPC/keypair/send/simulate plumbing with https-only endpoint enforcement and account-owner checks.
- `@pina-rs/codama-renderer-cli` (new package): renders commander-based TypeScript CLI apps on generated `@solana/kit` clients and `args`-based Dart CLI commands on generated solana_kit clients, with the same command surface and safety rules.
- `pina_cli`: `pina codama generate` gains opt-in `--cli-rust-out`, `--cli-ts-out`, and `--cli-dart-out`; CLI generation runs only when one of them is provided.
- `pina_codama_renderer`: emit `Vec::with_capacity(remaining_accounts.len())` instead of `0 + len` for instructions without accounts.

Generated CLIs for every example program are committed under `codama/clients/cli/{rust,ts,dart}`; the Rust crates are workspace members, the TypeScript apps are covered by `check:js`, and the Dart package is analyzer-clean.
