---
pina_cli: minor
---

Add the `pina_cli` crate for automatic Codama IDL generation from Pina program source code.

The crate provides both a library API (`generate_idl()`) and a CLI binary (`pina`) with subcommands (starting with `pina idl`) that parse Rust source files using `syn`, extract program structure (accounts, instructions, errors, PDAs, discriminators), and produce [Codama](https://github.com/codama-idl/codama-rs) standard JSON output for client code generation.

**Key features:**

- Parses `declare_id!()`, `#[discriminator]`, `#[account]`, `#[instruction]`, `#[derive(Accounts)]`, and `#[error]` macros/attributes
- Infers `isSigner` and `isWritable` properties from pina's validation chain pattern (`self.field.assert_signer()?.assert_writable()?`)
- Extracts PDA seed information from `macro_rules!` seed macros and byte-string constants
- Resolves known program addresses (system, token, token-2022, ATA) as default values
- Maps pina's Pod types (`PodU64`, `PodBool`, `Address`, etc.) to Codama type nodes
- Produces valid `codama-nodes` v0.7.x `RootNode` JSON with accounts, instructions, PDAs, errors, and discriminators

**CLI usage:**

```sh
pina idl -p examples/counter_program             # stdout
pina idl -p examples/escrow_program -o idl.json  # file output
```

**Library usage:**

```rust
let root = pina_cli::generate_idl(Path::new("examples/counter_program"), None)?;
let json = serde_json::to_string_pretty(&root)?;
```

Snapshot tests verify IDL output for fixture programs and all four example programs (counter, escrow, transfer_sol, hello_solana).
