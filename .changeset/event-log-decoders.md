---
pina_cli: breaking
pina_codama_renderer: breaking
---

# Generated Event Log Decoders

Generated TypeScript, Dart, and Rust clients now carry the event read path. The Codama IDL renders each event's `[discriminator][migrationVersion][payload]` envelope, so generated decoders align with `Program data:` log records and enforce the current schema version with direction-aware errors.

- TypeScript emits a per-program `events/logs.ts` entry point that decodes `Program data:` lines, projects historical bytes into the current shape when the checked-in migration manifest proves the transition is automatic, and reports `sourceVersion` plus `wasMigrated`.
- Dart emits `events/` modules with the same decode, projection, and log parsing API; the upstream Dart renderer does not render event nodes.
- The Rust client renders `events/` modules with strict decoding, a `try_from_bytes` that distinguishes stale from future versions, and a `project_from_bytes` that returns current bytes with their source version.
- Manual transitions remain the documented limit: generated clients cannot represent them, so those log versions fail closed with a message naming the transition.

## Breaking changes

- `pina_cli`: the parsed event declaration gained the public `migratable` field, so `EventDeclaration` struct literals must set it, and `CodamaError` gained the `EventHistories` variant, so exhaustive matches must handle it.
- `pina_codama_renderer`: `RenderConfig` gained the public `event_histories` field, so struct literals built without `..RenderConfig::default()` must set it.
