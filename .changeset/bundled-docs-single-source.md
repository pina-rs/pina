---
pina_cli: fix
---

# Sync the bundled `pina docs` topics with their templates

`pina docs pina-idl` and `pina docs pina-overview` printed hand-maintained copies of the repository templates, and the copies had fallen behind. Both topics are now mdt consumers of `templates/*.t.md`, as `pina-validation` already was, so `docs:check` fails when a bundled topic and its template disagree and `fix:format` rewrites it.

`pina docs pina-idl` now:

- lists generated dispatch from an `#[discriminator(entrypoint)]` enum and no longer advertises the `Accounts::try_from(accounts)` arm, which `#[derive(Accounts)]` stopped generating
- documents `#[pina(validate(...))]` as a source of signer, writable, and default-account metadata, alongside the direct `assert_*()` chains
- names an `#[discriminator(entrypoint)]` enum in the ambiguous-input rule
- describes the current `test:idl` contract, including the CPI, Rust CLI, and Dart clients
- includes the on-chain IDL lifecycle (`pina idl fetch`, `diff`, and `publish`)
- states that the migration version envelope never accepts `u64`

`pina docs pina-overview` now:

- lists the `verbose-logs` and `validation` features, and describes `logs` as static logging
- describes `PodF32` and `PodF64`
- replaces the guidance to keep `assert_writable()` on `&mut AccountView`, to call `assert_empty()` before initialization, and to use `assert_type::<T>()` against type cosplay with the current writable-slot, typed creation builder, and typed loader guidance
- adds the stored-bump and checked PDA loader guidance and the `CreateProgramAccountWithUncheckedBump` caveat

Both topics gain a title and section headings.

The `pina profile compare` summary existed only in the bundled copy. It moves into the shared `pinaProfileDescription` template, so the repository readme and the mdBook `crates-and-features` chapter now carry it too.
