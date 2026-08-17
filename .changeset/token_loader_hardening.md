---
pina: fix
---

Harden the token account loaders: SPL Token accounts and mints must match their exact base lengths, while Token-2022 data must be either the exact base layout or a valid extension layout with the correct account-type marker. Multi-owner and associated-token loaders select the policy from the account's actual owner, accepting valid Token-2022 extensions without accepting overlong legacy accounts or malformed extension data. The unsafe base-state projection now follows complete layout validation and remains guarded by the runtime borrow.
