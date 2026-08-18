---
pina: fix
---

Harden the token account loaders without local casts. SPL Token accounts retain Pinocchio's legacy state type, while Token-2022 accounts retain its complete `StateWithExtensions` type and validation. The new `TokenMintRef` and `TokenAccountRef` enums provide common field access for code that supports either program, while still exposing the concrete Token-2022 state when extensions are needed. Associated-token loaders validate against the explicitly selected program, so valid extensions are accepted without accepting overlong legacy accounts, malformed extensions, or mixed-program account sets.
