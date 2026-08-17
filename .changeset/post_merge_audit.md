---
codama-nodes-from-pina: fix
core: none
---

Repair regressions discovered while auditing the recently merged pull requests. Ensure the npm package is built and verified before publication, publish its real CommonJS, ESM, and declaration entry points, build CPI-heavy examples with Solana's supported SBF toolchain, include the associated-token program in escrow instructions, and harden the release, development-shell, fuzz, and end-to-end workflows that guard the workspace.
