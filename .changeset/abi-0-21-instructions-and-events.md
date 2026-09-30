---
pina: breaking
pina_macros: breaking
pina_cli: breaking
pina_codama_renderer: breaking
---

# Snapshot instructions and decode events per version

- **Instructions carry no version envelope by default.** Under an `auto` policy that covers instructions, a plain `#[instruction]` is recorded as a snapshot (`"envelope": false`) with no version byte on the wire, so every instruction payload is one byte shorter. The build fails when the struct drifts from its snapshot. `pina migrations create` replaces an unpublished snapshot; after publication it refuses a payload change (`PublishedPayloadChanged`: declare a new discriminator) but still appends optional accounts in place.
- **Full instruction migrations are an explicit opt-in.** `#[instruction(..., migrations)]` keeps the envelope and adjacent transitions. The `#[discriminator(entrypoint)]` dispatcher converts a historical payload before calling the new `ProcessAccountInfos::process_from_version(data, source_version)`, which defaults to `process`, so handlers no longer normalize data themselves. An added instruction argument always needs a manual transition, because a zero-filled argument is indistinguishable from a client's zero. `normalize_instruction_data` returns a `NormalizedInstruction` that also reports the source version.
- **Programs published under ABI 0.20** keep enveloped instructions: add `migrations` to each published `#[instruction]`. The build error names the contract, and `pina migrations create` refuses to add or remove an envelope once an instruction is published (`EnvelopeAddition`, `EnvelopeRemoval`).
- **Events are versioned, not migrated.** The program emits only the current version, and changing an event appends a version with no transition. `normalize_event_data`, `MigratableEvent`, and `CurrentEventData` are removed. Codama IDLs describe every earlier version as its own event (`<Event>V<n>`), so the TypeScript, Dart, and Rust clients decode each log with the schema that emitted it. The program log parsers fail closed on a version no generated event describes, instead of projecting old logs into the current shape. Decoded events no longer carry `sourceVersion`/`wasMigrated`, and client generation no longer reads the migration manifest.
- **TypeScript event decoders are named for what they do.** The generated per-event decoder `normalize<Event>Event` is renamed `decode<Event>Event` (for example `decodeValueChangedEventEvent` and `decodeValueChangedEventV0Event`), matching the Dart clients' existing `decode<Event>Event`. It decodes one version and converts nothing. Rename calls to the old name.
