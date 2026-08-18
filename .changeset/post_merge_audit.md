---
codama-nodes-from-pina: fix
core: none
---

Repair regressions discovered while auditing the recently merged pull requests. Ensure the npm package is built and verified before publication, publish its real CommonJS, ESM, and declaration entry points, build CPI-heavy examples with Solana's supported SBF toolchain, include the associated-token program in escrow instructions, and harden the release, development-shell, fuzz, mutation, and end-to-end workflows that guard the workspace. Pull-request mutation tests remain advisory, but surviving mutants now appear as a failed step and explicit summary instead of a false-green result.
