# Production Readiness

Pina supplies program-building primitives. It does not make an application safe merely because the application compiles, passes the framework tests, or follows an example. Before deploying a program that controls valuable assets, define its economic invariants, test them at the transaction boundary, and obtain an independent security review.

## What the examples prove

Examples have deliberately narrow scopes. They demonstrate framework APIs, account layouts, validation order, CPI construction, IDL extraction, or compatibility with an upstream test case. They are not audited applications.

In particular:

- `staking_rewards_program` proves pool and position account creation, authority checks, ATA validation, and checked bookkeeping. Deposit, withdraw, and claim do not transfer stake or reward tokens.
- `vesting_program` proves schedule-state creation, PDA and ATA validation, and claim/cancel bookkeeping. It does not read the clock, calculate vested entitlement, transfer claimed tokens, or return tokens after cancellation.
- A passing native test does not prove that an SBF-backed test ran. Some example E2E tests report a skip when the required program binary has not been built.

Use these programs as focused implementation samples. Do not deploy them unchanged or treat their instruction names as evidence that the corresponding economic operation occurred.

## Application invariants

Write the invariants for each instruction before implementation. For an asset-bearing program, cover at least:

- the authority allowed to initiate the transition;
- the exact accounts, owners, mints, programs, and canonical PDAs accepted;
- the assets that must move and the balances that must be conserved;
- the state values that may change, including their monotonicity and bounds;
- the allowed time window and the trusted time source;
- rounding, precision, overflow, and zero-value behavior;
- replay, duplicate-account, cancellation, pause, close, and migration behavior;
- Token and Token-2022 compatibility, including extensions that the program accepts or rejects.

Treat the on-chain account layout and instruction data as a protocol ABI. Plan migrations before changing either one.

## Verification gate

Before a public deployment:

1. Test every instruction against the built SBF artifact. Make missing artifacts fail the production CI job instead of skipping tests.
2. Add negative tests for every authority, owner, signer, writable, program-ID, mint, PDA, and account-aliasing constraint.
3. Assert post-transaction token and lamport balances, not only program-owned bookkeeping fields.
4. Test transaction atomicity: every rejected CPI or late validation failure must leave balances and state unchanged.
5. Add boundary and property tests for arithmetic, time, capacity, and serialization rules.
6. Pin the toolchain and dependencies used for the audited build, then reproduce the final SBF hash from a clean environment.
7. Profile compute use on the SBF artifact with realistic account sizes and worst-case instruction inputs.
8. Review upgrade authority, emergency controls, monitoring, and incident response before mainnet deployment.
9. Commission an independent review of the application logic. Framework checks and repository CI are supporting evidence, not a substitute.

## Staking completion checklist

Before `staking_rewards_program` can represent a staking product, it needs application-specific decisions and tests for:

- custody transfers between user accounts and PDA-controlled stake/reward vaults;
- vault ownership, mint consistency, funding, solvency, and withdrawal authority;
- a time-based reward-emission and precision model;
- reward-debt settlement before every stake change;
- the difference between accrued, claimable, and paid rewards;
- pause, unpause, administration, position closure, and pool shutdown;
- adversarial deposits, withdrawals, claims, duplicate accounts, and depleted reward vaults.

## Vesting completion checklist

Before `vesting_program` can represent a vesting product, it needs application-specific decisions and tests for:

- clock-sysvar validation and a precise cliff/linear/unlock formula;
- escrow funding during initialization and proof that the vault is sufficiently funded;
- PDA-signed transfers from the vault to the beneficiary;
- cancellation policy, including who receives unvested tokens and whether vested tokens remain claimable;
- rounding at schedule boundaries and protection against claiming the same entitlement twice;
- schedule amendment, closure, and recovery behavior;
- adversarial claims before the cliff, after cancellation, at exact boundaries, and against substituted vaults or mints.

The [Security Model](./security-model.md) documents framework-level invariants. The [CI and Releases](./ci-and-releases.md) page describes the repository's verification layers; an application should adopt equivalent gates for its own program logic.
