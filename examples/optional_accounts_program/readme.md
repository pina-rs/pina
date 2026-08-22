# Optional Accounts Program

Demonstrates pina's optional account slots: `Option<&'a AccountView>` and `Option<&'a mut AccountView>` fields inside `#[derive(Accounts)]` structs.

The number of accounts stays fixed for every instruction. When a caller omits an optional account, the generated Codama clients fill the slot with a readonly account meta pointing at this program's own address. On-chain, a slot holding the executing program's address parses as `None`; any other address parses as `Some`.

## Coverage matrix

| Instruction | Optional field                           | Behaviour when present                                  |
| ----------- | ---------------------------------------- | ------------------------------------------------------- |
| `init`      | —                                        | Creates the store PDA (baseline).                       |
| `touch`     | `store: Option<&mut>`                    | Increments the on-chain counter.                        |
| `inspect`   | `store: Option<&>`, `witness: Option<&>` | Store must hold `StoreState`; witness must be a signer. |
| `note`      | `note: Option<&>`                        | Logged as an opaque readonly reference.                 |

## Instructions

| Discriminant | Name      | Required accounts                               | Optional accounts  |
| ------------ | --------- | ----------------------------------------------- | ------------------ |
| `0`          | `Init`    | `authority` (signer), `store`, `system_program` | —                  |
| `1`          | `Touch`   | `authority` (signer)                            | `store` (mutable)  |
| `2`          | `Inspect` | `authority` (signer)                            | `store`, `witness` |
| `3`          | `Note`    | `authority` (signer)                            | `note`             |

## Running the tests

```sh
devenv shell -- cargo test -p optional_accounts_program
devenv shell -- cargo build-optional-accounts-program
devenv shell -- env SBF_OUT_DIR=target/deploy cargo test \
  -p optional_accounts_program --test on_chain -- \
  --include-ignored --nocapture
```

The Surfpool end-to-end coverage lives in `codama/tests/surfpool/src/` and the LiteSVM client tests in `codama/tests/litesvm/src/optionalAccounts.test.ts`.
