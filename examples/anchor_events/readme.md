# anchor_events

<br>

Pina parity port of Anchor's event definitions and serialization semantics.

## What this demonstrates

<br>

- Event discriminators with `#[event]`.
- Deterministic event payload encoding/decoding.
- Instruction-to-event mapping logic.

## Differences From Anchor

<br>

- This example focuses on event type/data behavior, not Anchor's `emit!`/`emit_cpi!` transport.
- Event emission is modeled as pure Rust value construction (`build_event`).
- Tests validate byte-level roundtrips and expected payload values.

## Run

<br>

```sh
cargo test -p anchor_events
pina idl --path examples/anchor_events --output codama/idls/anchor_events.json
```
