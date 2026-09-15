---
pina: feat
pina_cli: feat
pina_lints: feat
---

# Add incident-driven security lessons and the full-balance-drain lint

The security guide grows two lessons grounded in exploits from the last three years: `11-admin-key-compromise` (Raydium 2022, DEXX 2024, Drift 2026) shows a vault where one leaked key sweeps every lamport and redirects the program in a single signature, and rebuilds it with the containment real protocols shipped — a guardian-gated pause that cannot move funds, a per-window withdrawal cap on the Mango v4 `net_borrow_limit` shape, dual-control unpause, and two-phase authority rotation; `12-oracle-integrity` (Loopscale 2025, Makina 2026) shows a market whose price feed passes ownership checks yet still prices loans, because the configured oracle address is ignored and staleness is unchecked, and secures it with feed pinning plus a Clock-driven staleness bound. The research mapping every cited incident to its vulnerability class and mitigation lives in `security/incidents-2023-2026.md`.

The lint catalog gains `require_guarded_full_balance_drain` (warn), which flags instruction handlers that can sweep an account's entire balance to a recipient with no pause, circuit-breaker, or close intent in sight — the exact shape the cited drains used — and `require_checked_asset_arithmetic` now also denies `<<`/`>>` and `<<=`/`>>=` on asset-named values with `checked_shl`/`checked_shr` suggestions, closing the shift-overflow family that broke Cetus. The new lint runs clean across every example and secure lesson, and fires on the new admin-key insecure fixture.
