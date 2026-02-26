# anchor_declare_id

<br>

Pina parity port of Anchor's `declare-id` example.

## What this demonstrates

<br>

- Program ID declaration via `declare_id!`.
- Instruction decoding with `parse_instruction`.
- Rejection of program ID mismatches.

## Differences From Anchor

<br>

- Program ID mismatch validation is handled directly by `parse_instruction(program_id, &ID, data)`.
- There is no Anchor `Context` type. The instruction entrypoint receives raw account views.
- The example keeps a minimal `Initialize` instruction and tests behavior directly.

## Run

<br>

```sh
cargo test -p anchor_declare_id
pina idl --path examples/anchor_declare_id --output codama/idls/anchor_declare_id.json
```
