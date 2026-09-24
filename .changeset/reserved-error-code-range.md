---
pina: feat
pina_macros: breaking
---

# Reject reserved-range `#[error]` discriminants

Pina reserves the custom error codes `0xFFFF_0000..=0xFFFF_FFFF` for `PinaProgramError`, but nothing enforced it: a user `#[error]` enum could declare `SomethingFailed = 0xFFFF_FFF5` and compile, and clients then decoded that error as the framework's `MigrationBudgetExceeded`. The `#[error]` macro now emits a compile-time assertion for every variant, so a variant whose explicit, constant-expression, or implicit auto-incremented discriminant falls in the reserved range fails to build with an error that points at the variant:

```text
error[E0080]: evaluation panicked: error discriminant for `MyError::Boundary` is in the range 0xFFFF_0000..=0xFFFF_FFFF reserved for Pina's framework errors; use a value below 0xFFFF_0000
```

The boundary is exported as `pina::RESERVED_ERROR_CODE_START`, which the assertion reads. The check is a `const _` item, so it emits no code: compute units and program size are unchanged. An enum that already used a reserved code no longer compiles; move those variants below `0xFFFF_0000`.
