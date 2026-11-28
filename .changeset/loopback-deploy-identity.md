---
pina_cli: breaking
pina_skill: docs
---

# Identify the cluster behind a loopback deploy target

`pina deploy` used to treat every loopback target, `localnet` included, as local. A local target skips confirmation, `--allow-mainnet`, and the publication receipt. A loopback port can be an SSH tunnel or a proxy to a live cluster, so a deploy through a forwarded `127.0.0.1:8899` reached that cluster with none of those safeguards (SEC-13).

Before planning, Pina now asks a loopback endpoint for its genesis hash and version. A devnet, testnet, or mainnet-beta genesis hash gets that cluster's safeguards. Surfpool reports the genesis hash of the cluster it forks, but its `getVersion` answer names Surfpool and every transaction it accepts stays local, so it remains local. Any other genesis hash is a local validator. When the initial probe obtains an identity, the plan pins it: Pina asks again just before the deploy starts and stops if the genesis hash or Surfpool answer changed (an unidentified endpoint already requires confirmation and `--allow-mainnet`, without a stored identity to recheck), and publication verification requires the same genesis hash and Surfpool answer after the deploy.

## User impact

Deploys to `solana-test-validator` or Surfpool work as before. Start the validator before you plan a deploy to it: an endpoint that does not answer is now treated as an unknown cluster, so the plan reports `requires_mainnet_acknowledgement: true` with a warning, and execution needs `--allow-mainnet`. A loopback port that forwards to devnet or testnet now asks for confirmation, and one that forwards to mainnet-beta also needs `--allow-mainnet`.

Add `DeployError::TargetChanged` for a loopback endpoint that answers differently at launch than at planning, and `DeploymentPlan::verify_target_unchanged` to check it. Rust callers that exhaustively match `DeployError` must handle the new variant.

The agent skill's CLI reference describes the new loopback rule.
