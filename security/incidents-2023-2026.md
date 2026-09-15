# Solana incident research, 2023–2026

Date: 2026-09-14

Scope: major exploits and near-misses touching the Solana ecosystem from September 2023 through September 2026, plus cross-chain incidents included only when the vulnerability class directly instructs Solana program design. This is the research behind security lessons [11](11-admin-key-compromise/) and [12](12-oracle-integrity/) and the `require_guarded_full_balance_drain` lint and `require_checked_asset_arithmetic` shift coverage added alongside it.

## Conclusions

- The dominant loss driver on Solana in the window was **privileged-key compromise with no on-chain containment** — not missing per-instruction checks. Drift (~$285M, April 2026), DEXX (~$21M), and the custodial bot drainers all ended the same way: one leaked or fooled key moved everything, and nothing in the program could slow, cap, or halt the drain.
- The second class is **unvalidated price input**. Loopscale (~$5.8M) fell to a malicious feed deployed under the real oracle program, which owner checks alone cannot catch; only address pinning plus freshness bounds can.
- The third class is **arithmetic edge cases that survive audits until a degenerate configuration makes them profitable** (Cetus on Sui, Balancer cross-chain). Checked arithmetic — including shift operators — on every asset path is the program-level defense.
- Frequently repeated "Solana hack" attributions are wrong in both directions: PlayDapp (February 2024) was an Ethereum ERC-20 mint-authority compromise, not a Solana program; and the Raydium loss was ~$4.4M–$5.5M, not the ~$440k often quoted.
- On-chain mitigations are well established in Solana code (Mango v4 instruction gates and per-window borrow limits, Drift's per-market pause bitmask, MarginFi's expiring panic pause, BPF-loader `SetAuthorityChecked`). Lesson 11 distills them into pina; the new lint flags the ungated full-balance drain shape that recurs across incidents.

## Incident catalog

### Solana, in window

| Incident              | Date            | Loss                 | Root cause class                                 |
| --------------------- | --------------- | -------------------- | ------------------------------------------------ |
| Drift Protocol        | 2026-04-01      | ~$285M               | Governance/key compromise + unconstrained oracle |
| Aquifer               | 2026 (reported) | ~$2.5M               | Compromised admin credentials (single-source)    |
| Loopscale             | 2025-04-26      | ~$5.8M (returned)    | Unpinned malicious price feed                    |
| ZK ElGamal proof bugs | 2025-04/06      | none (patched)       | Proof soundness (client-level)                   |
| DEXX                  | 2024-11-16      | ~$21M (est. to $30M) | Custodial key compromise                         |
| @solana/web3.js npm   | 2024-12-02      | ~$160k               | Supply-chain key exfiltration                    |
| Pump.fun              | 2024-05-16      | ~$1.9M               | Privileged bypass, no program-enforced invariant |
| Solareum              | 2024-03-26      | ~$1.4M               | Insider (DPRK IT worker)                         |
| Slerf                 | 2024-03-18      | ~$10M burned         | Irreversible privileged operation, no guardrail  |

**Drift Protocol (2026-04-01, ~$285M).** Not a code bug. The Security Council was a 2-of-5 multisig with zero timelock; two signers were socially engineered into pre-signing durable-nonce transactions — signed payloads that stay valid until the nonce advances — which carried hidden admin authorizations. Weeks later the attacker executed them, called `UpdateAdmin`, listed a worthless wash-traded token as collateral (the program's oracle accepted it), raised withdrawal limits, and drained in roughly twelve minutes. Three separate on-chain guards would each have contained it: a timelock between approval and execution (invalidating indefinitely-lived pre-signed payloads), withdrawal limits that a single admin signature cannot raise, and oracle feeds constrained to program-derived, liquidity-verified accounts.

**Loopscale (2025-04-26, ~$5.8M, returned 2025-04-29).** Sixteen days after launch, an attacker deployed a malicious price feed and invoked Loopscale's own `create_loan`, taking undercollateralized loans in the new SOL/USDC Genesis vaults. The OShield audit had flagged oracle validation and reported it fixed — the fix covered ownership, not address pinning. All funds were returned after negotiation. Lesson 12 is this attack in miniature.

**DEXX (2024-11-16, ~$21M).** Custodial memecoin terminal; attacker obtained users' entrusted private keys and laundered through 8,600+ wallets. Operational, but the program-level lesson stands: delegated authorities with per-token, per-window spend caps enforced on chain bound a leaked server key to a rounding error.

**Pump.fun (2024-05-16, ~$1.9M).** A former employee used privileged access to the platform's internal borrowing mechanism to trade bonding curves without capital. Solvency checks lived in operational logic, not in the instruction: every instruction must enforce its own invariants regardless of who signs.

**Solareum (2024-03, ~$1.4M) and Slerf (2024-03-18, ~$10M destroyed).** Solareum's wallets were drained with insider help (DOJ filing: a DPRK IT worker infiltrated the team). Slerf's developer burned presale, airdrop, and LP allocations by mistake. Both are the "irreversible privileged action" failure mode: burns, closes, and migrations need program-scoped allowlists, staged execution, and dry-run discipline.

**ZK ElGamal proof program (2025, patched, no loss).** Two soundness bugs in the Token-2022 confidential-transfer proof verifier ("phantom challenge" transcript binding) could have enabled unauthorized minting. Included as a missed-verification class bug in core infrastructure; not lintable at the program level.

### Cross-chain, included for the class

- **Radiant Capital** (2024-10-16, ~$53M): malware spoofed the signing UI for three of eleven multisig signers; the team had removed its timelock after a July 2024 incident, so the drain was immediate. Lesson 11's two-phase rotation and pause guardian are the on-chain answers.
- **Bybit** (2025-02-21, ~$1.5B): compromised Safe{Wallet} front end; signers blind-approved an implementation upgrade embedding a sweep. On-chain timelocks and cap state — not UI trust — bound this.
- **Cetus Protocol** (Sui, 2025-05-22, ~$223M, $162M frozen by validators): a flash-loaned 200-tick position drove a liquidity denominator toward zero and a `checked_shlw` shift lacked overflow checking, minting ~10^34 liquidity units. Motivates shift coverage in `require_checked_asset_arithmetic`; the same unfixed pattern was later found in Kriya, FlowX, and Turbos.
- **Balancer** (2025-11-03, ~$116–128M): a rounding-direction error dormant since 2021 (flagged by Trail of Bits in 2021 as undetermined severity) became profitable only in low-liquidity pools. Program-level takeaway: rounding must favor the protocol and be fuzzed across multi-operation flows; rate limits and pause guardians cap the blast radius.
- **Makina Finance** (2026-01-20, ~$4.2M): flash-loan-inflated Curve pool fed the program's oracle — Loopscale's class, again.

### Pre-window anchors (context for the taxonomy)

- **Raydium** (2022-12-16, ~$4.4M–$5.5M): trojan-infected operator machine leaked the single AMM authority key; malicious withdrawals hid inside legitimate transactions. The original "leaked sweep key" incident.
- **Mango Markets** (2022-10, ~$110–117M): oracle manipulated through a thin market; the team's emergency upgrade response worked only because upgrade authority existed — Mango v4 then added instruction gates, per-window net-borrow limits, and an asymmetric security admin.
- **OptiFi** (2022-08-29, ~$661k stranded): a routine upgrade ran `program close`, permanently bricking the program ID. The lost-key mirror image of the compromise cases.

## Vulnerability classes → repository coverage

| Class                                        | Representative incidents              | Coverage added here                                    | Existing coverage                                                                             |
| -------------------------------------------- | ------------------------------------- | ------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| Key compromise, ungated drains               | Raydium, Drift, DEXX, Radiant, Bybit  | Lesson 11; `require_guarded_full_balance_drain` (warn) | — (none of the prior 20 lints looked at drains)                                               |
| Unvalidated price input                      | Loopscale, Drift (CVT), Makina, Mango | Lesson 12                                              | Lessons 01/02 cover ownership generally; not pinning/freshness                                |
| Arithmetic edge cases                        | Cetus (shift), Balancer (rounding)    | Shift coverage in `require_checked_asset_arithmetic`   | Add/sub/mul/div and compound assignments were already denied                                  |
| Privileged bypass without program invariants | Pump.fun                              | — (documented)                                         | `require_checked_asset_arithmetic` catches the asset-math symptoms, not the missing invariant |
| Supply chain / signing infrastructure        | web3.js npm, BigONE, Bybit            | Out of program scope                                   | Repo CI hardening; signing hygiene is operational                                             |

## Lint changes

**New: `require_guarded_full_balance_drain` (warn).** Fires on instruction handlers that pass an account's own full balance (`account.lamports()`, inline or through one binding) to a lamport send, when the function shows no close intent (`zeroed`/`close*` on the same account) and no guard-shaped call (pause, cap, circuit, halt, guard, limit, throttle). This is the exact shape Raydium's and Drift's drains used. It is warn, not deny: legitimate sweeps exist, and verifying a guard semantically is beyond a lexical lint — the help text states what a real guard looks like (pause flag plus per-window cap, Mango v4's `net_borrow_limit_per_window_quote` shape). Lesson 11's insecure `sweep` triggers it; the secure program does not.

**Extended: `require_checked_asset_arithmetic`.** Now also denies `<<`/`>>` and `<<=`/`>>=` on asset-named integral values, suggesting `checked_shl`/ `checked_shr`. Cetus was a shift-overflow; shifts were the one unchecked arithmetic family the lint still missed (the compound-assignment gap noted in earlier research has since been fixed upstream).

**Corpus verification.** `cargo test -p pina_lints` passes with 20 UI tests and catalog sync; `security:pina-lint` runs clean over every example and secure lesson; the new lint's UI fixture covers the guarded, close-path, partial-amount, and non-handler exemptions.

## Proposed future lints (not implemented here)

Ordered by expected value; both need the semantic hooks the earlier lint infrastructure research called for before a low-false-positive rule exists.

1. **Oracle-validation presence.** A handler reads a feed-shaped account without any Clock comparison in the same function (the syntactic form of the staleness absence Sherlock's oku findings and the Solana Security Standard flag). Blocked on a pina oracle/feed API whose resolved calls a lint can anchor to; a name-only version would be noise.
2. **Two-phase authority rotation.** A handler assigns a caller-supplied address into an authority/admin/owner field with no pending-authority marker — the single-signature takeover shape behind Radiant and Bybit. Deferred because it warns on the `role_registry_program` scaffold's one-step `RotateAdmin`; that example should be upgraded to the two-phase pattern (with its surfpool tests) before the lint lands.
3. **Unused signed accounts.** Accounts proven to be signers never referenced again are re-privilege fodder for mid-handler CPIs (Asymmetric Research's invocation-security work); needs resolved provenance, not spellings.

## Sources

- Drift: [TRM Labs](https://www.trmlabs.com/resources/blog/north-korean-hackers-attack-drift-protocol-in-285-million-heist), [Chainalysis](https://www.chainalysis.com/blog/lessons-from-the-drift-hack/), [BlockSec](https://blocksec.com/blog/drift-protocol-incident-multisig-governance-compromise-via-durable-nonce-exploitation), [CoinDesk](https://www.coindesk.com/tech/2026/04/02/how-a-solana-feature-designed-for-convenience-let-an-attacker-drain-usd270-million-from-drift)
- Loopscale: [rekt.news](https://rekt.news/loopscale-rekt/), [Halborn](https://www.halborn.com/blog/post/explained-the-loopscale-hack-april-2025)
- Cetus: [rekt.news](https://rekt.news/cetus-rekt/)
- Balancer: [Trail of Bits](https://blog.trailofbits.com/2025/11/07/balancer-hack-analysis-and-guidance-for-the-defi-ecosystem/), [Certora](https://www.certora.com/blog/breaking-down-the-balancer-hack), [Check Point](https://research.checkpoint.com/2025/how-an-attacker-drained-128m-from-balancer-through-rounding-error-exploitation/)
- Bybit: [rekt.news](https://rekt.news/bybit-rekt/), [FBI IC3 PSA](https://www.ic3.gov/psa/2025/psa250226), [DarkNavy](https://www.darknavy.org/darknavy_insight/reconstructing_the_1.5_billion_bybit_hack_by_north_korean_actors/)
- Radiant: [post-mortem](https://medium.com/@RadiantCapital/radiant-post-mortem-fecd6cd38081), [Halborn](https://www.halborn.com/blog/post/explained-the-radiant-capital-hack-october-2024)
- BigONE: [rekt.news](https://rekt.news/bigone-rekt/)
- Makina: [QuillAudits](https://www.quillaudits.com/blog/hack-analysis/makina-4m-hack-explained), [rekt.news](https://rekt.news/makina-rekt)
- DEXX: [Cointelegraph](https://www.tradingview.com/news/cointelegraph:287d4fd9c094b:0-over-8-6k-solana-wallets-linked-to-21m-dexx-hacker/), [SlowMist](https://history.harrydenley.com/event/e0e807b5-f0af-4339-aefb-7b14e8b7acd5)
- Pump.fun: [Cointelegraph](https://www.tradingview.com/news/cointelegraph:5200dae5d094b:0-memecoin-launcher-pump-fun-claims-ex-employee-behind-1-9m-exploit/), [Bankless](https://www.bankless.com/read/pump-fun-hit-for-2m-in-flash-loan-exploit)
- Solareum: [DL News](https://www.dlnews.com/articles/regulation/how-a-dprk-developer-tricked-solareum-and-stole-14m/)
- Slerf: [rekt.news](https://rekt.news/slerf-rekt), [The Block](https://www.theblock.co/news/ecosystems/2024-03-18-solana-memecoin-slerf-burn-283025)
- PlayDapp (Ethereum, misattributed): [Nefture](https://medium.com/nefture/playdapp-exploit-post-mortem-of-a-290m-heist-f6803349cde8), [Halborn](https://www.halborn.com/blog/post/explained-the-playdapp-hack-february-2024)
- web3.js npm: [Socket](https://socket.dev/blog/supply-chain-attack-solana-web3-js-library), [ReversingLabs](https://www.reversinglabs.com/blog/malware-found-in-solana-npm-library-with-50m-downloads)
- ZK ElGamal: [Solana post-mortem](https://solana.com/news/post-mortem-may-2-2025), [zkSecurity](https://blog.zksecurity.xyz/posts/solana-phantom-challenge-bug/)
- Raydium: [CertiK](https://www.certik.com/blog/raydium-protocol-exploit-incident-analysis), [Raydium post-mortem](https://raydium.medium.com/detailed-post-mortem-and-next-steps-d6d6dd461c3e)
- OptiFi: [incident report](https://medium.com/@OptiFi/optifi-program-incident-report-08-29-22-d8fe6d229bad)
- Mitigation implementations: [Mango v4](https://github.com/blockworks-foundation/mango-v4) (ix gates, `Bank` borrow windows), [Drift protocol-v2](https://github.com/drift-labs/protocol-v2) (`paused_operations`), [MarginFi v2](https://github.com/mrgnlabs/marginfi-v2) (expiring panic pause), [Squads v4](https://github.com/Squads-Protocol/v4)
- Guidance: [Solana docs — program deployment/upgrade authority](https://solana.com/docs/core/programs/program-deployment), [Neodyme — upgrade authority](https://neodyme.io/en/blog/solana_upgrade_authority), [Solana Operational Security Standard](https://publish.obsidian.md/sos/standard/wiki/realms), [Asymmetric Research — invocation security](https://www.asymmetric.re/blog-archived/invocation-security-navigating-vulnerabilities-in-solana-cpis), [Sherlock oku #604](https://github.com/sherlock-audit/2024-11-oku-judging/issues/604)
