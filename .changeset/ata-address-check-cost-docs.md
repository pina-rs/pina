---
pina: none
---

# Document the cost of the ATA address checks

`assert_associated_token_address` and `as_associated_token_account` each perform a canonical associated-token-address search, which costs on the order of a thousand compute units and grows with how far the address's canonical bump sits below 255. Both doc comments now state that cost and warn against calling the assert and the loader with the same wallet, mint, and token program in one handler, because the second call repeats the search for an address the first call already proved.

The guidance consolidates what three example handlers had drifted into (`vesting_program`'s `Claim` and `Cancel` and `escrow_program`'s `Take` each paid the search twice per instruction until the companion example change removed the redundant calls). Docs-only change; no behavior, wire, or measurement impact.
