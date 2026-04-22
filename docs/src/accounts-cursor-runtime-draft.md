# Accounts cursor runtime design draft

- Status: Draft
- Related issue: [#143](https://github.com/pina-rs/pina/issues/143)
- Related review: [Anchor `lang-v2` review](./anchor-next-review.md)

This document sketches a concrete path for reworking Pina's `#[derive(Accounts)]` runtime around a cursor-based loader, informed by `solana-foundation/anchor` `anchor-next/lang-v2` but adapted to Pina's explicit-validation and `no_std` constraints.

## Why change the current model?

Today `#[derive(Accounts)]` mainly expands to direct slice destructuring over `&mut [AccountView]`.

That is simple and easy to audit, but it starts to strain when Pina needs richer account-loading behavior such as:

- nested account groups
- explicit remaining-account cursors
- duplicate mutable alias diagnostics
- future optional/composite account ergonomics
- shared loader logic between entrypoint dispatch and future CPI/codegen paths

Anchor `lang-v2` demonstrates that a reusable cursor/loader runtime can support these needs cleanly.

## Constraints Pina must preserve

Any redesign must keep the following Pina invariants intact:

- no heap allocation in core on-chain account parsing
- `no_std` compatibility
- explicit validation order in user code
- discriminator-first fixed layouts
- no weakening of duplicate mutable alias guarantees
- proc-macro output should remain understandable and reviewable

## Proposed runtime model

### 1. Introduce an `AccountsCursor<'a>`

A lightweight cursor owns:

- the original `&'a mut [AccountView]`
- the current index
- duplicate-account bookkeeping state

Core operations:

- `peek()`
- `next()`
- `remaining()`
- `finish_exact()` or equivalent exactness check

The cursor should be allocator-free and work entirely with indices and borrowed references.

### 2. Split parsing from validation

Derive-generated code should stop directly destructuring account slices.

Instead it should:

1. parse structural account positions through the cursor
2. produce a typed accounts struct of borrowed `&AccountView` / `&mut AccountView`
3. leave semantic validation in user-authored `process(...)` methods

This keeps Pina's existing style intact:

- parsing answers "which account is where?"
- user code answers "is this account valid for my program logic?"

### 3. Make duplicate mutable checks first-class in the runtime

The cursor should explicitly track whether a parsed writable account aliases a prior writable account reference.

That allows:

- earlier and clearer failures
- shared duplicate-account logic across all derived structs
- future nested-account support without re-implementing alias checks in generated code paths

### 4. Model remaining accounts as a cursor view, not only a trailing slice

Current `#[pina(remaining)]` gives raw trailing access.

The next step should preserve that capability but add a more structured runtime concept:

- either a `RemainingAccounts<'a>` wrapper
- or a borrow of the cursor with restricted operations

That would make future nested parsing and optional account loading safer and more composable.

## Suggested traits

A possible shape is:

```rust,ignore
pub struct AccountsCursor<'a> {
	// borrowed account slice, index, duplicate tracking
}

pub trait ParseAccounts<'a>: Sized {
	fn parse(cursor: &mut AccountsCursor<'a>) -> Result<Self, ProgramError>;
}
```

Then `#[derive(Accounts)]` would generate `ParseAccounts` and keep `TryFromAccountInfos` as a compatibility layer that delegates to the cursor runtime.

That gives Pina an incremental migration path:

- existing public trait stays usable
- internal runtime becomes richer
- follow-up derive features have a stable foundation

## Migration plan

### Phase 1 — Internal cursor introduction

- add `AccountsCursor<'a>` behind the current public API
- reimplement `TryFromAccountInfos` derive output in terms of the cursor
- preserve current exact/remaining behavior byte-for-byte where possible

### Phase 2 — Explicit duplicate-account runtime checks

- move duplicate mutable alias detection into cursor state
- add adversarial regression tests for nested and repeated-account cases

### Phase 3 — Structured remaining accounts

- add a typed remaining-accounts wrapper
- update derive code and docs
- preserve a simple slice-based escape hatch when needed

### Phase 4 — Nested/composite account loaders

- allow derive-generated account structs to contain parsed sub-groups
- keep final semantic validation explicit in user code

## Testing requirements

This redesign should land with dedicated tests for:

- exact account count handling
- remaining-account passthrough
- duplicate mutable alias rejection
- nested loader ordering
- compatibility with current `ProcessAccountInfos` flows
- compile-fail coverage for unsupported account-struct shapes

## Non-goals

This draft does **not** propose:

- hiding validation logic behind Anchor-style constraint attributes
- adding allocator-backed parsing helpers to on-chain paths
- making `#[derive(Accounts)]` opaque or difficult to inspect
- changing the guard-backed typed account loader model

## Relationship to the CPI-handle prototype

The cursor-runtime work complements the typed CPI-handle prototype from [#142](https://github.com/pina-rs/pina/issues/142).

Together they point toward a more coherent runtime story:

- cursor-based typed account parsing on entry
- guard-backed typed data access in handlers
- typed CPI-handle composition for outbound invocations

That combination is the main architectural lesson Pina can take from Anchor `lang-v2` without copying its heap-backed or asm-oriented design choices.
