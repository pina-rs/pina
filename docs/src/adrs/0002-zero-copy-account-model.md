# ADR 0002: Keep zero-copy behind explicit validation

- Status: Accepted
- Date: 2026-04-18
- Deciders: Pina maintainers
- Related: [Security model](../security-model.md), `security/loaders-audit.md`

## Context

Low compute usage is a stated project goal, and zero-copy account access is one of the biggest reasons to use Pina instead of heavier Solana framework stacks.

But zero-copy is only defensible when the layout contract is tight. Unsafe or dynamically shaped reinterpretation can erase the very safety properties the framework is supposed to enforce.

## Decision

Pina keeps zero-copy account and instruction handling as a core design choice. Zeropod owns representation and byte-to-view conversion, while Pina's macros enforce a closed supported field grammar before invoking its derive.

In practice that means:

- macro-generated application schemas accept only audited scalar, address, byte-array, and scalar-option fields; loaders return the generated `TypeZc` storage view
- typed loads must validate discriminator, size, content (`ZcValidate`), and relevant account identity constraints before use
- dynamic, variable-length, or schema-driven reinterpretation is out of scope for the core loader model
- custom/nested `ZcField` mappings, enum-typed payload fields, generic schemas, and `PodString`/`PodVec` or `String`/`Vec` fields are outside the macro-generated contract
- Pina does not manually implement zeropod's unsafe traits, duplicate its pointer casts, or expose a schema/storage-view object representation as bytes
- bounded text and lists use fully initialized fixed byte arrays with checked semantic helpers
- manual `PinaAccount` / `ZeroPodFixed` implementations are advanced escape hatches whose authors own all zeropod safety invariants

## Consequences

Benefits:

- no heap copies are required for common account access paths
- account parsing stays predictable in both runtime cost and memory behavior
- the framework can keep `no_std` and low-dependency goals without abandoning typed APIs

Costs:

- some data models must use explicit versioning or companion accounts instead of variable-length in-place layouts
- loader APIs need stronger lifetime coupling than a simple `&T` return type can provide
- future extensions must prove they preserve layout and aliasing safety, not just correctness in happy-path tests

### Comparison with Quasar

Quasar does not avoid collection fields. At commit [`b0de7db`](https://github.com/blueshift-gg/quasar/tree/b0de7db4cd271654a2dcf78807dd865e98e0b339), its account derive classifies `String` and `Vec` as dynamic fields, maps them to `PodString` and `PodVec`, and places the generated compact schema in a hidden child module ([layout generation](https://github.com/blueshift-gg/quasar/blob/b0de7db4cd271654a2dcf78807dd865e98e0b339/derive/src/account/layout.rs)). Quasar then encapsulates dynamic access behind compact read views, load-mutate-save guards, and explicit writer commits. A commit resizes the account to the active compact tail before saving it ([dynamic access](https://github.com/blueshift-gg/quasar/blob/b0de7db4cd271654a2dcf78807dd865e98e0b339/derive/src/account/dynamic.rs)). Its compile-pass coverage explicitly accepts bounded `String` and `Vec` account fields ([collection example](https://github.com/blueshift-gg/quasar/blob/b0de7db4cd271654a2dcf78807dd865e98e0b339/lang/tests/compile_pass/account_string_vec_alias.rs)).

Pina deliberately does not claim parity with that compact representation in this decision. Pina preserves its existing fixed wire layouts, so its macros reject fixed-capacity collection fields until Pina has an equally closed design that prevents inactive backing capacity from becoming observable.

## Alternatives considered

### Copy-based deserialization into owned structs

Rejected because it adds compute overhead, increases stack or heap pressure, and gives up one of Pina's primary performance advantages.

### Unsafe dynamic zero-copy for arbitrary layouts

Rejected because it makes soundness depend on ad-hoc caller discipline and scattered invariants instead of framework-level rules.
