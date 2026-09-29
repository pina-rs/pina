---
pina_cli: fix
pina_codama_renderer: fix
pina_codama_renderer_cli: fix
---

# Harden generated clients against spoofed events, broken `Migrate` calls, and injected docs

- **Event attribution.** The TypeScript and Dart `parse<Program>EventsFromLogs` helpers decoded every `Program data:` line in a transaction, so any program — including one the caller invokes through CPI — could forge an event with the right discriminator, and one foreign future-version line made the whole parse throw. The parsers now follow the runtime's `Program <address> invoke [n]` / `success` / `failed` frames and decode a line only while the program (or an explicit `programAddress`) is the innermost invocation.
- **`Migrate` system program.** The TypeScript, Dart, and Rust `Migrate` composers filled an omitted `systemProgram` with the program-address placeholder, which the on-chain `MigrateContext` rejects, so the documented `getMigrateInstruction({ state, payer })` call always failed. Slot 1 now defaults to the system program, and the payer and system program slots are always sent.
- **Payer writability.** The IDL marked a payer passed to Pina's account-creation builders (and the `from`/`to` of `CreateAccount`/`Transfer`) read-only when its Rust field was a shared reference, so every client sent it read-only and the transaction failed with a privilege escalation whenever the payer was not also the fee payer. The extractor now marks those accounts writable; `counter_program`, `todo_program`, and `float_accounts_program` IDLs and clients are regenerated accordingly.
- **Doc-comment injection.** A doc comment containing `*/` closed the TypeScript JSDoc block, and a multi-line `#[doc = "..."]` value ended a `///` comment, turning the rest into live generated code. Docs are now split into lines and defused before rendering, and the Dart CLI renderer escapes `$` so IDL text can no longer become an interpolated Dart expression.
- **Packaging.** `@pina-rs/codama-renderer-cli` 0.22.0 was published without `dist/`, so `cli-ts` and `cli-dart` generation failed outside this repository. The publish job now builds the package and verifies the tarball, and its declaration build no longer inherits `noEmit`.
