---
"pina_cli_renderer": minor
"@pina-rs/codama-renderer-cli": minor
"pina_cli": minor
---

Add CLI client generation for Codama IDLs in three languages: `pina generate --client cli-rust`, `--client cli-ts`, and `--client cli-dart`.

- `pina_cli_renderer` (Rust): renders complete clap-based CLI crates — one subcommand per instruction with typed flags and validation, automatic PDA derivation from IDL seed definitions, a `fetch` command per state account (including compact accounts), decoded program-error names, and shared RPC/keypair/send/simulate plumbing with https-only endpoint enforcement.
- `@pina-rs/codama-renderer-cli`: renders commander-based TypeScript CLI apps on top of generated `@solana/kit` clients, and `args`-based Dart CLI commands on top of generated solana_kit clients, each with the same command surface and safety rules.
- `pina codama generate` gains `--cli-rust-out`, `--cli-ts-out`, and `--cli-dart-out`; generated Rust CLIs for every example program are committed under `codama/clients/cli/rust`, TypeScript apps under `codama/clients/cli/ts`, and one shared Dart package under `codama/clients/cli/dart`, all exercised in CI.

Selecting a CLI implies its base client, and projects normally pick one CLI; selecting several prints a warning.
