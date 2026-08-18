# pina_fuzz

Fuzz harnesses for the pina Solana framework, targeting the two most security-critical deserialization paths:

- **`PinaAccount::try_from_bytes`** — zero-copy account data reinterpretation with discriminator and content validation
- **`parse_instruction`** — instruction discriminator decoding with program-ID verification

## Structure

```text
crates/pina_fuzz/
├── Cargo.toml                          # Library crate (compiles with cargo check)
├── src/lib.rs                          # Re-exports for the fuzz targets
└── fuzz/
    ├── Cargo.toml                      # Fuzz binary crate (cargo-fuzz runner)
    └── fuzz_targets/
        ├── account_deserialize.rs      # Fuzz PinaAccount validation
        └── parse_instruction.rs        # Fuzz parse_instruction
```

## Running

Install `cargo-fuzz`:

```sh
cargo install cargo-fuzz
```

Run a fuzz target from the `fuzz/` directory:

```sh
cd crates/pina_fuzz/fuzz
cargo fuzz run account_deserialize
cargo fuzz run parse_instruction
```

To reproduce a crash artifact:

```sh
cargo fuzz run account_deserialize <crash-file>
```

## Account types under test

| Type             | Source                  | Size | Discriminator                             |
| ---------------- | ----------------------- | ---- | ----------------------------------------- |
| `CounterState`   | `counter_program`       | 10 B | `CounterAccountType::CounterState = 1`    |
| `RegistryConfig` | `role_registry_program` | 42 B | `RegistryAccountType::RegistryConfig = 1` |
| `RoleEntry`      | `role_registry_program` | 83 B | `RegistryAccountType::RoleEntry = 2`      |

## Instruction types under test

| Type                  | Source                  | Variants                                   |
| --------------------- | ----------------------- | ------------------------------------------ |
| `CounterInstruction`  | `counter_program`       | `Initialize = 0`, `Increment = 1`          |
| `RegistryInstruction` | `role_registry_program` | `Initialize = 0` through `RotateAdmin = 4` |
