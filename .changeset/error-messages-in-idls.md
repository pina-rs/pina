---
pina_cli: feat
pina: feat
---

# Carry error messages into IDLs and generated clients

Every example program's `#[error]` variants now carry doc comments, so the Codama IDL for each carries a real message instead of an empty string. 32 of the 42 error nodes across the checked-in IDLs had `"message": ""`, which generated clients render as a bare code — an explorer or logging stack could say "custom program error 0x1770" but not "offer key mismatch". A new `codama_idls` test fails when an `#[error]` variant has no doc comment, listing every undocumented variant by program and name. `custom_errors_program`'s `HelloNoMsg` stays deliberately undocumented because it is the Anchor-parity fixture for exactly this case, and the test exempts it by name.
