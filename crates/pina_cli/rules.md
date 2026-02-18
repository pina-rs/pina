# Rules for Reliable IDL Generation

Follow these rules so `pina idl` can extract complete and accurate metadata from program source.

1. **Single-file programs** Keep the program entrypoint, accounts, instructions, and macros in `src/lib.rs`. Multi-file module traversal is not supported yet.

2. **`declare_id!` is required** Declare exactly one program ID using `declare_id!("...")`.

3. **Validation must live in `process()`** Signer, writable, PDA, and default-address inference is collected from `impl ProcessAccountInfos for ... { fn process(...) { ... } }` bodies.

4. **Use direct `self.<field>.assert_*()` chains** Validation inference expects assertions on direct `self.field` chains. Indirection through helper functions or temporaries may be missed.

5. **Use `match` dispatch in `process_instruction`** Instruction wiring is inferred from a `match` over the discriminator in `process_instruction` (top-level or inside `mod entrypoint`) that calls `<Accounts>::try_from(accounts)?.process(data)`.

6. **Use explicit discriminator values** All `#[discriminator]` enum variants should define explicit integer values (`Initialize = 0`, `Update = 1`, ...).

7. **Seed macros must include `seeds` in the macro name** PDA macro discovery is heuristic and only scans `macro_rules!` names containing `seeds` (for example `counter_seeds!` or `seeds_counter!`).

8. **Seed constants must be byte-string constants** Use `const NAME: &[u8] = b"...";` for constant PDA seeds so they can be lifted into IDL.

9. **Reference discriminator enums in attributes** Account and instruction structs should always point to their discriminator enums via attributes such as `#[account(discriminator = AccountDiscriminator)]` and `#[instruction(discriminator = InstructionDiscriminator, ...)]`.

10. **Use known program ID paths for default-value inference** For automatic default account public keys, use `assert_address` with these paths: `system::ID`, `token::ID`, `token_2022::ID`, or `associated_token_account::ID`.
