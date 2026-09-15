---
pina: feat
---

# Emit event records to the transaction log

`#[event]` structs now generate an `emit` helper that writes a validated
`[discriminator][schema version][payload]` record to the `Program data:`
transaction log:

```rust
MyEvent::emit(|event| {
	event.data = 5;
	event.label = *b"hello\0\0\0";
	Ok(())
})?;
```

Generated Rust, TypeScript, and Dart clients already shipped decoders for those
log lines, but nothing on chain produced them. Programs had to build the record
bytes themselves and had no supported way to publish them, so an event type
could be declared, validated, and generated into three clients while never
reaching an indexer. `emit` closes that gap: it builds the record through the
same generated `initialize` path that `try_from_bytes` validates, so an emitted
record always decodes against the current schema, and it publishes the whole
envelope as one `sol_log_data` slice so the base64 payload is decodable.

`emit` requires Pina's `logs` feature. A build without it returns
`ProgramError::UnsupportedSysvar` rather than dropping the record, so a
misconfigured program fails loudly instead of appearing to emit.

`events_program` now emits through this helper, and its Surfpool suite asserts
that each instruction produces exactly one decodable `Program data:` record and
that the generated client reconstructs the configured field values. The example
enables the `logs` feature, which it previously lacked.
