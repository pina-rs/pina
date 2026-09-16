---
pina_test: feat
---

# Assert the error a program returned, and decode through its own reader

`pina_test::assert_custom_error` asserts that a program rejected an instruction with a specific custom code. It reads the structured `TransactionError` the harness already retains, so a test no longer matches rendered runtime text — the pattern one real consumer wrote by hand because the framework had no equivalent — and it cannot confuse the expected error with a different one that happens to share digits. The failure message names both the expected and the actual code, or reports the non-custom error that was returned instead. The expected code accepts anything convertible to `u32`, so a generated `#[error]` enum works directly.

The earlier assertion style — `assert!(!error.message().is_empty())` — proved only that _something_ failed, which passes even when the program fails for the wrong reason.

`pina_test::with_account_bytes` decodes a fetched account with the program's own generated reader instead of byte slicing. Tests otherwise assert `account.data[0]` for a discriminator and `account.data[2..]` for a field, which keeps passing against the wrong layout and cannot separate a decode failure from a value assertion. The helper is generic over the decoder's error type, so it adds no dependency from the test harness onto the framework, and the program's own error stays available to assert on.
