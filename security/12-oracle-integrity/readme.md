# 12: Oracle Integrity

<br>

## The Vulnerability

<br>

Lending, liquidation, and AMM programs all price positions from off-chain or market data. The vulnerability class is not one missing check but a chain of them: a program that reads a price account without pinning **which** feed it trusts, verifying **who owns** it, and rejecting **stale** observations is pricing loans with whatever an attacker hands it.

Real failures line up along that chain:

- **Unpinned feed.** Loopscale (April 2025, ~$5.8M) was drained sixteen days after launch when an attacker deployed their own malicious price feed and called the program's own loan creation against it. Ownership checks would not have stopped this: the attacker's feed lived under the same oracle program, so only pinning the exact feed address recorded in market configuration does.
- **Unconstrained acceptance.** Drift (2026, ~$285M) collapsed because the program accepted a freshly minted, wash-traded token as collateral at an oracle price the attacker manufactured. Value acceptance must be constrained to program-derived feeds with verified mints and liquidity bounds.
- **Stale observation.** The Sherlock-documented `oku` findings and the Solana Security Standard's oracle-integration rules both flag feeds consumed without comparing `publish_time`/`valid_slot` against the Clock: a frozen or abandoned feed keeps pricing loans at old extremes during volatility — exactly when liquidations matter most.
- **Manipulated price source.** Mango Markets (2022, ~$117M) remains the canonical case: the MNGO perp oracle tracked a low-liquidity market the attacker could move. Freshness and pinning do not help when the feed's _source_ is manipulable; sane-value bounds and liquidity-aware feeds are the mitigations.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The market loads the passed price account and — one step ahead of lesson 02 — even verifies its owner is the oracle program. But the market's configured oracle address (`Market::oracle`) is loaded and then **ignored**, and the feed's `updated_at` timestamp is never checked. Any feed created under the oracle program prices every loan, at any age.

## Why This Is Dangerous

<br>

An attacker can:

- Deploy a legitimate-looking feed under the real oracle program pricing a token they control, then borrow against inflated collateral
- Replay an old extreme price if the live feed froze during volatility
- Repeat the borrow across every market that shares the same unpinned feed handling

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The price path enforces all three properties before a price is trusted:

1. **Ownership** — `as_account::<PriceFeed>(&ORACLE_PROGRAM_ID)` proves the oracle program owns the feed (lesson 02's mitigation; necessary, not sufficient).
2. **Pinning** — `assert_address(&market.oracle)` proves the feed is _the_ one recorded in the market's configuration, so an attacker's freshly deployed feed cannot be substituted (the Loopscale fix).
3. **Freshness** — the observation age is compared against `MAX_STALENESS_SECONDS` using the Clock sysvar, so frozen feeds cannot keep pricing loans (the staleness fix).

Note what pinning alone does not buy: if the pinned feed's _source market_ is thin, the price itself is still manipulable (Mango). Production protocols add sane-value bounds, confidence intervals, and multi-source aggregation on top of the three checks shown here.

## Pina API Reference

<br>

- `AccountView::as_account::<T>(&Address)` — typed load that asserts the account's owner
- `AccountView::assert_address(&Address)` — pins the feed to the address stored in market state
- `AccountView::assert_sysvar(&Address)` — verifies the Clock account is the real sysvar
- `sysvars::clock::Clock::from_account_view(&AccountView)` — reads `unix_timestamp` for the staleness check
