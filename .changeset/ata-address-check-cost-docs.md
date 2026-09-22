---
pina: none
---

# Document the cost of the ATA address checks

`assert_associated_token_address` and `as_associated_token_account` each perform a canonical associated-token-address search, which costs on the order of a thousand compute units and grows with how far the address's canonical bump sits below 255. Both doc comments now state that cost and warn against calling the assert and the loader with the same wallet, mint, and token program in one handler, because the second call repeats a search the first already paid.

The guidance consolidates what two example handlers (`vesting_program`'s `Claim` and `Cancel`) had drifted into — each paid the search twice per instruction until the companion example change removed the redundant calls and recorded the measured savings. The same audit confirmed `escrow_program`'s `Take` derives its `taker_ata_b` exactly once, so its search is not redundant and stays. Docs-only change; no behavior, wire, or measurement impact.
