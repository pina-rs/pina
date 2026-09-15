# `events_program`

<br>

Pina parity port of Anchor's event definitions and serialization semantics.

## What this demonstrates

<br>

- Event discriminators with `#[event(discriminator = ...)]`.
- Declarative numeric and exact-length rules plus custom event hooks.
- Deterministic event payload encoding/decoding.
- Emitting validated records to the `Program data:` transaction log.
- Instruction-to-event mapping logic.

## Differences From Anchor

<br>

- This example does not cover Anchor's `emit_cpi!` transport. Programs emit through the generated `Event::emit` helper, which writes the same `Program data:` log records that the generated Rust, TypeScript, and Dart clients decode.
- Tests validate byte-level roundtrips, expected payload values, and the decoded log records a client observes.

## Run

<br>

```bash
cd examples/events_program
pina test --unit
pina test
pina generate
```
