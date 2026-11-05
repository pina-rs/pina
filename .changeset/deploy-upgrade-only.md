---
pina_cli: breaking
---

# Upgrade a deployed program without its program keypair

`pina deploy --upgrade-only` upgrades the program already deployed at the source's declared program ID without reading a program keypair. Pina passes the bare address to `solana program deploy --program-id`, which can upgrade with it but not create a program, so a first deployment still needs the program keypair. A continuous-deployment job can now upgrade devnet with a dedicated upgrade authority and never hold the key that controls the program's address on mainnet.

The plan's `program_keypair` is `null` for such an upgrade, the human plan shows `Program keypair: none (upgrade by declared program ID)`, and a `--remote-command` receives no `PINA_DEPLOY_PROGRAM_KEYPAIR`. Every other safeguard is unchanged: target policy, confirmation, snapshot fingerprints, and publication receipts.

`solana program deploy` now also receives the fee payer as `--keypair`. The Solana CLI loads that default signer even when every role is explicit, so a deployment on a machine without Solana CLI configuration, such as a CI runner, failed with `No default signer found`; it no longer reads that configuration at all.

**Breaking:** `DeploymentRequest::program_keypair` is replaced by `program_address: ProgramAddress` (`ConventionalKeypair`, `Keypair(path)`, or `DeclaredId`), and `DeploymentPlan::program_keypair()` now returns `Option<&str>`. The `--dry-run --json` plan's `program_keypair` key is nullable.
