# anchor_declare_program

Pina parity port of Anchor's `declare-program` behavior.

## What this demonstrates

- Modeling external program IDs.
- Validating executable program accounts.
- Guarding CPI-style paths with explicit program checks.

## Differences From Anchor

- External program validation is explicit (`assert_external_program_id`) instead of framework constraints.
- Account checks are performed manually via chained `AccountView` assertions.
- Instruction routing is explicit `match`-based dispatch.

## Run

```sh
cargo test -p anchor_declare_program
pina idl --path examples/anchor_declare_program --output codama/idls/anchor_declare_program.json
```
