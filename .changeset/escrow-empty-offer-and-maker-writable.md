---
pina: none
---

# Reject empty escrow offers and prove the maker is writable

`escrow_program`'s `Make` handler now rejects an offer whose `amount_a` or `amount_b` is zero with a new `EscrowError::EmptyOffer` (code 2), before any account is created or any token moves. A zero `amount_b` let a maker escrow token A for nothing, and a zero `amount_a` let a taker receive the vault for nothing; both are fat-finger shapes rather than real offers. The variant is new rather than a reuse of `OfferKeyMismatch` or `TokenAccountMismatch`, because wire values are part of the program ABI and remapping an existing code would change what deployed clients decode. The doc comment on the variant becomes the message in the regenerated Codama IDL and the Rust, JavaScript, and Dart clients.

`Take` now asserts the maker is writable next to the existing address check, so a read-only maker fails with Pina's own `InvalidAccountData` diagnostic instead of a generic runtime failure from the vault or escrow close CPIs. The `&mut AccountView` field already enforced this at parse time, so the assert is defense in depth: the test passes with it removed. It costs 16 CU on `Take` (32,241 to 32,257), which is the price of keeping the requirement visible in the handler, and the maker is credited twice there (vault rent and escrow rent).

Two Surfpool cases cover the behavior: one asserts `EmptyOffer` and that no balance moves and no account is created, and one presents a read-only maker through a non-payer keypair and asserts the failure plus unchanged escrow, vault, and taker balances.
