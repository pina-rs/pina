# Anchor `lang-v2` review and follow-up backlog

This page records the focused review of `solana-foundation/anchor` on branch `anchor-next`, with emphasis on `lang-v2` and the `bench/programs/prop-amm` benchmark.

## Reviewed upstream paths

Primary framework/runtime files:

- `lang-v2/README.md`
- `lang-v2/src/context.rs`
- `lang-v2/src/context_cpi.rs`
- `lang-v2/src/cursor.rs`
- `lang-v2/src/dispatch.rs`
- `lang-v2/src/lib.rs`
- `lang-v2/src/loader.rs`
- `lang-v2/src/pod.rs`
- `lang-v2/src/traits.rs`

Benchmark/example files:

- `bench/programs/prop-amm/anchor-v1/src/lib.rs`
- `bench/programs/prop-amm/anchor-v2/src/lib.rs`
- `bench/programs/prop-amm/anchor-v2/src/instructions.rs`
- `bench/programs/prop-amm/anchor-v2/src/instructions/initialize.rs`
- `bench/programs/prop-amm/anchor-v2/src/instructions/rotate_authority.rs`
- `bench/programs/prop-amm/anchor-v2/src/error.rs`
- `bench/programs/prop-amm/anchor-v2/src/state.rs`
- `bench/programs/prop-amm/anchor-v2/src/asm/entrypoint.s`

## What Pina should adopt, adapt, and avoid

### Adopt or adapt

- A more trait-first runtime surface instead of pushing more semantics into proc macros.
- Typed CPI handles and typed CPI account structs.
- Cursor-based account parsing that can support nested account groups, duplicate-account tracking, and explicit remaining-account handling.
- Better generated-client account resolution for default programs and derived addresses.
- Carefully bounded zero-copy container ideas where they do not violate Pina's fixed-layout and no-allocation goals.

### Explicitly avoid

- Handwritten asm fast paths like `bench/programs/prop-amm/anchor-v2/src/asm/entrypoint.s`.
- Any design that compiles away important safety checks in production configurations.
- Heap-backed runtime APIs in core on-chain paths.
- Dynamic container abstractions that undermine discriminator-first fixed layouts or make borrow provenance harder to reason about.

## Prioritized issue backlog

### P0 — Add typed CPI handles and a const-generic CPI context ([#142](https://github.com/pina-rs/pina/issues/142))

Why this matters:

- `lang-v2/src/traits.rs` and `lang-v2/src/context_cpi.rs` show the clearest ergonomic win relative to current Pina APIs.
- Typed CPI handles help encode writable/read-only intent and reduce accidental misuse when building CPI account lists.
- This fits naturally with Pina's recent move to guard-backed account loaders.

Scope for Pina:

- Keep the API allocator-free.
- Prefer const-generic account counts over `Vec` in on-chain paths.
- Start from checked `pinocchio::cpi::invoke_signed` and only consider unchecked variants after the account-runtime story is stronger.
- Eventually teach generated CPI account structs and client code to use the same model.

Do not copy directly from Anchor:

- The heap-backed `Vec` design in `lang-v2/src/context_cpi.rs`.
- Any panic-based writable checks.

### P1 — Rework `#[derive(Accounts)]` around a cursor-based loader/runtime ([#143](https://github.com/pina-rs/pina/issues/143))

Why this matters:

- `lang-v2/src/cursor.rs`, `lang-v2/src/loader.rs`, and `lang-v2/src/dispatch.rs` expose a stronger runtime model than today's simple slice destructuring.
- Pina currently gets good clarity from explicit validation code, but it has limited structure for nested account groups, richer duplicate-account analysis, and future optional-account ergonomics.

Scope for Pina:

- Preserve explicit validation chains as the user-facing model.
- Move parsing/runtime logic out of ad-hoc generated slice destructuring into a reusable cursor abstraction.
- Make duplicate mutable alias checks and remaining-account handling first-class.
- Keep the final API `no_std` and allocator-free.

Do not copy directly from Anchor:

- Any runtime path that assumes heap allocation is acceptable.
- Any abstraction that hides validation order or weakens Pina's explicitness.

### P1 — Improve generated clients with resolved default accounts and PDA inference ([#144](https://github.com/pina-rs/pina/issues/144))

Why this matters:

- Anchor `lang-v2` pushes more account-resolution intelligence into generated surfaces.
- Pina's generated Codama clients are already useful, but callers still supply more boilerplate than necessary for common program/system account defaults and canonical PDA derivations.

Scope for Pina:

- Auto-fill well-known program accounts when the IDL marks them as defaults.
- Support deterministic PDA derivation helpers in generated clients.
- Make signer/writable expectations clearer in generated builders.
- Preserve exact IDL semantics so generation stays reproducible and reviewable.

### P2 — Add compile-time PDA bump precomputation where seeds are fully static

Why this matters:

- Anchor `lang-v2/README.md` hints at compile-time bump optimization opportunities.
- Pina already favors explicit seeds and deterministic PDA behavior, so compile-time precomputation could shave host-side and on-chain setup overhead in narrow cases.

Scope for Pina:

- Restrict this to obviously static seed sets.
- Preserve canonical bump semantics.
- Keep it as an optimization, not a semantic fork in PDA validation.

### P2 — Explore bounded dynamic Pod containers only if they fit Pina's invariants

Why this matters:

- `lang-v2/src/pod.rs` is an interesting demonstration of alignment-safe integer wrappers and zero-copy-friendly support code.
- More expressive bounded containers could unlock richer examples without abandoning fixed-layout account models.

Scope for Pina:

- Keep discriminator-first layouts.
- Avoid allocator requirements.
- Prefer small, fixed-capacity, bytemuck-auditable designs.
- Land only after stronger tests, docs, and invariants are written down.

## `prop_amm` port outcome

The Pina port lives at:

- `examples/prop_amm_program/Cargo.toml`
- `examples/prop_amm_program/src/lib.rs`
- `examples/prop_amm_program/tests/e2e.rs`

This is intentionally a semantic port of the benchmark logic, not a port of the handwritten assembly benchmark harness.
