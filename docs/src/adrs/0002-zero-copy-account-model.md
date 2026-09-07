# ADR 0002: Keep zero-copy behind explicit validation

- Status: Accepted, amended for PinaPod v0.2
- Date: 2026-04-18
- Last amended: 2026-09-07
- Deciders: Pina maintainers
- Related: [Security model](../security-model.md), [PinaPod v0.2 migration](../migrations/pinapod-v0.2.md), `security/loaders-audit.md`

## Context

Zero-copy account access reduces compute and memory use, but only when the representation contract is closed. A reference formed from unchecked account bytes can cause undefined behavior before later semantic validation runs.

The first version of this decision limited application schemas to scalar fields and fixed byte arrays. PinaPod v0.2 now fully initializes bounded containers, recursively validates active values, and limits compact mutation to generated patches. Those guarantees allow Pina to accept bounded strings, vectors, and options without weakening the boundary.

## Decision

Pina keeps zero-copy account and instruction handling as a core design choice. PinaPod owns representation and byte-to-view conversion. Pina's macros enforce a closed field grammar before invoking the derive.

The fixed schema grammar accepts audited scalars, addresses, byte arrays, `String<N>`, `Vec<T, N>`, and `Option<T>` where every nested `T` has a fixed PinaPod representation. `PodString<N, PFX>` and `PodVec<T, N, PFX>` select an explicit `1`, `2`, `4`, or `8` byte prefix.

Compact accounts use a narrower grammar because each dynamic field needs generated offset and patch logic. They accept:

- `String<N>`
- `Vec<T, N>` for fixed `T`
- `Option<T>` for fixed `T`
- `Option<String<N>>`
- `Option<Vec<T, N>>` for fixed `T`
- `Vec<String<M>, N>`

Dynamic fields form the final suffix, and one account can have several tails. Unsupported nesting fails at macro expansion with an error that lists the accepted forms.

The boundary also requires these rules:

- Typed loads validate the discriminator, size, content, and relevant account identity before use.
- PinaPod initializes inactive fixed-container capacity and zeroes payload bytes removed by safe mutations.
- Generated compact patches validate the complete update before changing account data or lamports.
- Pina does not duplicate PinaPod pointer casts or expose a schema or storage-view object representation as bytes.
- A manual `PinaPodFixed` implementation is an `unsafe` escape hatch whose author owns every documented invariant.

## Consequences

Fixed accounts can store bounded text, lists, and options without custom byte-array helpers. They pay rent for the declared capacity because the full representation is inline.

Compact accounts pay only for active tail data. Their API is more constrained: reads use generated views, and writes use a generated patch plus `UpdateResizableAccount`. The framework owns grow-before-write and write-before-shrink ordering.

Pina remains narrower than Rust's type system and the complete Codama schema language. Custom mappings and unsupported dynamic nesting fail at compile time rather than falling back to unchecked behavior.

## Historical comparison

The original ADR compared Pina with Quasar and kept collections outside Pina's schema macros. That restriction was correct for the earlier container implementation, which could leave inactive backing bytes uninitialized and exposed staged compact mutation. PinaPod v0.2 removes those two blockers while preserving the existing wire format.

## Alternatives considered

### Copy-based deserialization into owned structs

Rejected because it adds compute, adds stack or heap pressure, and discards the in-place access model.

### Arbitrary dynamic nesting

Deferred because recursive offsets, partial updates, and generated clients need one unambiguous layout. The first compact release supports the forms listed above. Future releases can add a form after native, Miri, code-generation, and SBF tests prove the full lifecycle.

### Unsafe dynamic zero-copy for arbitrary layouts

Rejected because it moves layout, initialization, and aliasing invariants into caller discipline.
