---
pina: fix
---

Harden the token account loaders: SPL token accounts and mints are now required to be exactly `LEN` bytes (matching the token program's own validation), while token-2022 accounts keep the minimum-length check so extension data is accepted. The length requirement is now expressed as an explicit `LengthRequirement::Exact`/`AtLeast` enum instead of a bare `minimum_len` parameter, and the unsafe `from_bytes` call's SAFETY comment documents the validated invariant precisely. Adds tests for overlong/short SPL token accounts, token-2022 extension data, and overlong associated token accounts.
