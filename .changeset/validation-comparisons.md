---
pina_macros: feat
pina: feat
pina_cli: feat
pina_skill: docs
---

# Spell value validation rules as comparisons

A value rule is now a comparison over the field's `value` or its `len`, so the annotation reads as the check it generates:

```rust
#[account(discriminator = Kind::State)]
struct Counter {
	#[pina(validate(value >= 1 && value <= 10, len <= 64, value != 0))]
	pub amount: u64,
}
```

This replaces the `min`/`max`/`min_len`/`max_len`/`exact_len` parameter spellings, which made the annotation read like an assignment to a name nothing ever assigns and left inclusive-versus-exclusive to a naming convention. Comparisons carry the direction in the operator, chain bounds into one rule (`100 < value <= u64::MAX`), and can express what the named bounds could not — `value != 0` for a declarative non-zero check.

## The named bounds are deprecated, not removed

`min`, `max`, `min_len`, `max_len`, and `exact_len` still parse and generate the identical checks, so nothing breaks. Each one warns at the parameter the author wrote and names its replacement:

```text
warning: use of deprecated constant `_::PINA_DEPRECATED_VALIDATION_BOUND`: `min` is deprecated;
write the bound as a comparison, as in `value >= 1`
 --> src/lib.rs:8:22
  |
8 |     #[pina(validate(min = 1, max = 10))]
  |                      ^^^
```

`error = ERROR` keeps its spelling: it names the failure to raise, not a comparison. The warning is the ordinary `deprecated` lint, so `#[allow(deprecated)]` silences it and `-D warnings` fails on it. The check compiles to an empty const block and costs no compute units.

## No compatibility effect

The IDL and migration manifests do not record value rules, and the checks the deprecated spellings generate are the same ones comparisons generate, so migrating an annotation changes nothing on the wire.
