# ADR 0003: Typed account loaders must be guard-backed

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: `security/loaders-audit.md`, #120, #121, #122

## Context

The loader audit identified a high-severity soundness problem: returning plain `&T` or `&mut T` from temporary runtime borrow guards lets the guard drop before the typed reference stops being used.

That escaped-borrow pattern affected both generic account loaders and token helper loaders.

## Decision

Typed account loader APIs must keep the runtime borrow guard alive for the full lifetime of typed access.

The reference shape for this decision is a guard-backed wrapper such as:

- `LoadedAccount<'a, T>` for immutable typed access
- `LoadedAccountMut<'a, T>` for mutable typed access

These wrappers retain the underlying `pinocchio::account::Ref` / `RefMut` and expose the typed account through `Deref` / `DerefMut` instead of returning bare references.

The same rule applies to token and ATA helper loaders. Safe owner or address validation is necessary, but it is not a substitute for keeping the borrow guard alive.

## Consequences

Benefits:

- overlapping mutable and immutable borrows fail through the runtime borrow model instead of becoming aliasing bugs
- zero-copy access is preserved without severing lifetime coupling
- token helper APIs follow the same soundness model as generic account loaders

Costs:

- this is a breaking public API change for typed loader return values
- helper traits and examples must use wrapper types instead of assuming raw `&T` / `&mut T`
- regression coverage needs Miri and borrow-specific tests, not only ordinary functional tests

## Alternatives considered

### Keep returning bare references and document the caveat

Rejected because soundness bugs are not acceptable as documentation-only footguns.

### Copy account data into owned values

Rejected because it throws away the zero-copy design goal and still does not solve the core runtime borrowing contract for mutation.
