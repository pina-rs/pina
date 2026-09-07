# Native PinaPod integration tracking

This historical record began with `pina-rs/pina#193` and `pina-rs/pina#195`. The first integration used the upstream zeropod names. PinaPod v0.2 replaces those names and extends the audited schema grammar.

## Design decisions

1. Pina's macros keep native schemas separate from generated storage views.
2. Bytes enter the model through checked loaders. Pina does not expose a schema or storage-view object representation as bytes.
3. PinaPod initializes inactive fixed-container capacity and zeroes removed payload bytes.
4. Fixed initialization starts with an exact caller-owned buffer and validates the completed value.
5. PinaPod owns zero-copy casts. Pina owns discriminator, account identity, borrow-guard, and lifecycle checks.
6. Generated clients own their wire buffers and enforce the same canonical values and capacities.
7. Compact mutation uses a generated patch. Pina owns rent adjustment and runtime resize ordering through `UpdateResizableAccount`.

## Completed migration

- [x] Return generated `TypeZc` views from fixed loaders.
- [x] Rename the derive and traits to `PinaPod`, `PinaPodFixed`, and `PinaPodCompact`.
- [x] Accept bounded strings, vectors, and recursively fixed options in Pina schemas.
- [x] Add multiple compact tails and the documented compact nesting grammar.
- [x] Replace public staged compact mutation with generated patches.
- [x] Generate PinaPod schemas and checked storage views in Rust clients.
- [x] Enforce compact capacity in TypeScript and Dart clients.
- [x] Keep native, Miri, generated-client, and SBF regression coverage.

## Safety boundary

The native schema value is never treated as its wire representation. PinaPod validates initialized runtime buffers before Pina returns a view. Mutable fixed views and compact updates remain tied to the source buffer or account borrow. Manual unsafe trait implementations remain outside Pina's audited macro-generated contract.
