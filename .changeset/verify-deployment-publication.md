---
pina_cli: breaking
---

# Verify deployment publication on chain

Require finalized RPC readback of the approved program bytes, loader accounts, upgrade authority, and cluster before finalizing a migration publication receipt. Keep unverifiable deployments pending even when the deployment command exits successfully.

Add `DeployError::PublicationUnverified` for failed publication verification. Rust callers that exhaustively match `DeployError` must handle the new variant.
