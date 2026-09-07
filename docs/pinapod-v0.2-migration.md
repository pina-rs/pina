# PinaPod v0.2 downstream migration

This document is the migration inventory for adopting PinaPod v0.2 in Pina. The upstream v0.2 API is not available yet, so this branch must not add compatibility adapters for names or signatures that may still change.

The migration preserves the existing account and instruction wire formats. A metadata change in Codama is allowed because it does not change bytes stored on-chain or sent in instruction data.

## Requirements before the dependency update

Do not update Pina's dependency until PinaPod provides all of these contracts:

- The public derive and trait name is `PinaPod`.
- Fixed `String`, fixed `Vec`, and recursively nested `Option` storage initialize every byte that Pina may copy into account or instruction buffers.
- Validation recursively checks every active value before safe access, including an active value nested inside `Option`.
- Compact tail lengths cannot be changed independently of their backing bytes through a safe mutable header API.
- Compact updates validate every requested length and capacity before mutating the destination.
- The v0.1 fixed and compact layouts have byte-for-byte fixtures proving wire compatibility.

The dependency has one source of truth in the workspace `Cargo.toml`. It currently selects `pinapod = "0.1.0"`, so Cargo cannot resolve v0.2 without an explicit manifest update. After a v0.2 release, update the workspace dependency and `Cargo.lock` together. Do not leave a git or path override in a publishable crate. Pina's CI uses `--locked`, which will reject a manifest-only upgrade.

## Migration inventory

### Public PinaPod API

The following Pina sources directly name v0.1 traits and methods and must migrate together:

- `crates/pina/src/lib.rs` re-exports the dependency and its `ZeroPod*` APIs.
- `crates/pina/src/traits.rs` defines the account boundary on `ZeroPodFixed` and `ZeroPodCompact`.
- `crates/pina/src/impls.rs` loads account borrows through those traits.
- `crates/pina_macros/src/support.rs` emits fixed-layout loaders.
- `crates/pina_macros/src/account.rs` emits compact-layout loaders and implementations.
- `crates/pina_macros/src/schema.rs` emits compile-time storage and layout proofs.
- `crates/pina_codama_renderer/src/render/accounts.rs` and `crates/pina_codama_renderer/src/render/instructions.rs` generate downstream Rust clients that name the old traits.

Delete the old names once every site is migrated. Both repositories are controlled together, so a compatibility module would preserve two public contracts without reducing migration risk.

Cheapest proof: run the Pina macro UI suite, Pina library tests with all features, the no-default feature matrix, renderer snapshots, and a real SBF example build.

### Fixed collections and nested options

`crates/pina_macros/src/schema.rs` is the single boundary that currently rejects these source types. Keep the restrictions until the upstream initialization and recursive-validation contracts are proven.

These UI fixtures isolate the current restrictions:

- `tests/ui/fail/account_string_field.rs` for fixed `String`.
- `tests/ui/fail/instruction_vec_field.rs` for fixed `Vec`.
- `tests/ui/fail/account_nested_option.rs` for nested `Option`.

Once PinaPod v0.2 is ready, extend the closed recursive classifier rather than falling back to an arbitrary user implementation. Move those three fixtures from `tests/ui/fail` to `tests/ui/pass` and delete their `.stderr` snapshots in the same change. Add runtime boundary tests beside `crates/pina/tests/schema_boundary.rs` for:

- empty, full, and over-capacity collections;
- invalid UTF-8 in active string bytes;
- invalid active elements nested through both `Option` and `Vec`;
- noncanonical option tags at every nesting level;
- initialized inactive capacity after default construction, clearing, and replacement.

The CLI already parses these shapes in `crates/pina_cli/src/parse/types.rs`, and `crates/pina_cli/tests/fixtures/semantic_options.rs` exercises nested options and collections for IDL generation. That parser coverage is not evidence that the on-chain schema macro accepts the same source.

Cheapest proof: make the three UI fixtures pass, then run `cargo test -p pina_root --test ui` and `cargo test -p pina --test schema_boundary` inside the devenv shell.

### Resizable account naming

There is no `UpdateResizableAccount` type in the current Pina tree. If the v0.2 integration introduces it, its public funding and refund field must be named `rent_account`, not `payer`. Apply the same semantic name in examples, tests, instruction account schemas, and generated Codama clients.

Do not rename the existing `payer` fields on `AllocateAccount`, `CreateProgramAccount`, `ReallocAccount`, `ReallocAccountZeroed`, or `ReallocCompactAccount`. Those APIs specifically model an account paying for allocation or reallocation and are outside this rename.

Cheapest proof: search the source tree for `UpdateResizableAccount`, then assert generated Rust, TypeScript, and Dart instruction builders expose `rent_account`, `rentAccount`, and `rentAccount` respectively after regeneration.

### Compact capacity metadata

Compact capacity is currently hidden in prose:

- `crates/pina_cli/src/codegen/mod.rs` appends `Pina compact capacity: N.` to field docs.
- `crates/pina_codama_renderer/src/render/types.rs` parses that sentence back into a number.
- The generated TypeScript and Dart compact codecs use the length prefix but do not enforce the declared Pina capacity.

Codama nodes 0.13.2 cannot represent field-level extension metadata: `StructFieldTypeNode` has docs, type, default, and display fields, while `PluginNode` is only attachable to instructions. Do not introduce a second prose convention or an unknown JSON property that a deserialization and serialization round trip will discard.

Before removing the docs parser, adopt a Codama node version or extension that can attach typed field metadata. The target representation must contain an unsigned `capacity` on each compact tail field. The CLI must reject missing, duplicate, non-integer, or prefix-incompatible values, and the Rust renderer must consume only that structured value. Human-readable docs may remain, but they must not control generated types or validation.

Generated TypeScript and Dart clients must validate capacity at both boundaries:

- Encoders reject a collection whose logical length exceeds the declared capacity.
- Decoders reject a prefix above capacity before allocating or iterating over elements.
- Multi-tail accounts validate each tail against its own capacity.
- Decoders keep the discriminator checks and consume the exact computed account length.

Add the contract cases in `packages/nodes-from-pina/test/generatedClientsContract.test.ts` and `codama/clients/dart/test/generated_contract_test.dart`, then regenerate clients through the Pina Codama workflow. Never patch files under `codama/clients/*/generated` by hand.

Cheapest proof: encode and decode lengths `0`, `capacity`, and `capacity + 1` for both `Journal` tails in TypeScript and Dart. Then run the IDL and client drift checks.

### Solana account lifecycle

The compact example in `examples/compact_accounts/src/lib.rs` currently coordinates tail updates, `commit`, rent movement, and `ReallocCompactAccount` in different orders for growth and shrink. When PinaPod v0.2 exposes an atomic checked update, migrate this example and the corresponding tests with the framework wrapper. Do not retain the staged `set_*` plus public `commit` path as a second lifecycle.

The account-data borrow must end before a runtime resize. Growth must validate the complete update before moving rent, and shrink must write a self-consistent shorter representation before reducing the account data length. A failed capacity or arithmetic check must leave both lamports and bytes unchanged.

Cheapest proof: extend the Surfpool compact-account test with failed over-capacity growth and failed shrink cases, asserting byte-for-byte data and lamport balances are unchanged.

## Required verification before merge

Run all commands inside `devenv shell`:

1. `cargo test -p pina_root --test ui --locked`
2. `cargo test -p pina --all-features --locked`
3. `cargo test -p pina_cli --all-features --locked`
4. `cargo test -p pina_codama_renderer --locked`
5. `build:pina:no-default`
6. IDL and generated-client drift checks
7. TypeScript and Dart generated-client contract suites
8. The tracked SBF compact-account build and its Surfpool lifecycle tests
