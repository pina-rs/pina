---
pina_cli: feat
pina_skill: feat
---

# Add safe, inspectable program deployment

`pina deploy` now resolves conventional Cargo deployment artifacts, verifies the declared program address against its keypair, and delegates deployment to the Agave Solana CLI only after showing the exact operation. Every invocation requires an explicit cluster or RPC URL, every remote write requires interactive confirmation or `--yes`, and named mainnet or an identity-unknown custom remote requires a second explicit acknowledgement.

Agents and CI can use `--dry-run --json` to inspect the canonical artifact, program ID, program keypair, upgrade authority, fee payer, operator-supplied endpoint, acknowledgement policy, and structured command plan without building, contacting a cluster, or invoking the Solana CLI. Custom URL user information, queries, and fragments fail closed. Accepted hosts and paths remain visible in plans and process listings, so operators must never place secrets anywhere in a custom URL and should prefer named clusters. Bounded, owner-private keypair reads and streamed artifact fingerprints let Pina reject unsafe files and changes detected during final pre-execution validation. The fully parameterized Solana child receives closed standard input because Pina owns operator confirmation.
